//! What the core checks instead of asking a round to remember.
//!
//! The principle (plan §4.D): **anything the core can check with code does not
//! get written as a "不得".** Half the protocol's prohibitions were enforced by
//! the model's goodwill, and a rule enforced by goodwill is a rule that holds
//! until the round is busy.
//!
//! These run at one point — after a session's design document has parsed and
//! before the transition table sees it — so there is exactly one place where a
//! round's output is admitted.
//!
//! Two of them fail the task; the rest are warnings that land on the task card
//! and in the events. The split is not about severity of intent but about
//! whether continuing would produce a *wrong* record:
//!
//! - An implementation round marking a milestone `已完成` has claimed the one
//!   thing only an independent audit may claim. Letting it through means a
//!   closed milestone nobody verified.
//! - A loop round with no evidence file has left nothing for the next round to
//!   read, and the audit has nothing to check against.
//!
//! **The core does not rewrite a session's document except for evidence pointers.** It would be easy to
//! flip the cell back and carry on, and it would leave the commit history and
//! the session log saying different things about what happened. The task stops
//! and a person looks. Closed evidence pointers have a narrow exception: the
//! scheduler restores only those lines in a recorded kernel commit.

use autome_domain::role::Role;
use autome_domain::status_block::{MilestoneState, StatusBlock};

/// Warning thresholds, plan §4.D. Deliberately generous: these fire on a trend
/// worth seeing, not on every round that runs long.
pub const DESIGN_DOC_MAX_BYTES: u64 = 80 * 1024;
pub const DESIGN_DOC_MAX_GROWTH_BYTES: u64 = 10 * 1024;
pub const RETRO_LINE_MAX_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The task stops and waits for a person.
    Error,
    /// Recorded and shown; the Loop keeps going.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    /// A stable identifier, for the event payload and for tests.
    pub code: &'static str,
    pub detail: String,
}

impl Finding {
    fn error(code: &'static str, detail: impl Into<String>) -> Self {
        Finding {
            level: Level::Error,
            code,
            detail: detail.into(),
        }
    }
    fn warn(code: &'static str, detail: impl Into<String>) -> Self {
        Finding {
            level: Level::Warning,
            code,
            detail: detail.into(),
        }
    }
}

/// What the previous session left behind, so a round can be compared with the
/// state it started from.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub milestones: Vec<(String, MilestoneState)>,
    #[serde(default)]
    pub retro_lines: usize,
    #[serde(default)]
    pub design_bytes: u64,
}

/// The selected integration audit and whether it wrote its assigned evidence.
#[derive(Debug)]
pub struct IntegrationAudit {
    pub legacy: bool,
    pub impl_round: u32,
    pub evidence: String,
    pub summary: String,
    pub evidence_touched: bool,
}

/// Everything a check needs, gathered by the caller so this module does no I/O
/// and stays testable without a repository.
pub struct Round<'a> {
    pub protocol_version: Option<u32>,
    pub integration: Option<&'a IntegrationAudit>,
    pub design_before: Option<&'a str>,
    pub design_after: Option<&'a str>,
    pub role: Role,
    /// The status block the session just wrote.
    pub status: &'a StatusBlock,
    /// What the previous session left. `None` for the first round.
    pub before: Option<&'a Snapshot>,
    pub retro_lines: usize,
    /// The one line the retro file gained, if it gained exactly one.
    pub retro_added: Vec<String>,
    pub design_bytes: u64,
    /// Filenames in `docs/<slug>/evidence/` after the session.
    pub evidence_files: Vec<String>,
    /// Whether the session's edit to the design document touched anything
    /// beyond the milestone table. `None` when it could not be determined.
    pub design_changed_outside_milestones: Option<bool>,
    /// The round was sent back to repair what the core's own gate found on
    /// the commit it left, not to advance a milestone. Its milestone is
    /// already `待审` and stays there until the gate passes and the audit
    /// looks at it, so "nothing was open" is the expected start.
    pub repairing: bool,
}

/// Runs every check that applies to this round.
pub fn check(round: &Round<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    match round.role {
        Role::Impl => {
            out.extend(evidence_file(round, "impl"));
            out.extend(no_closing_a_milestone(round));
            out.extend(no_retargeting_closed_evidence(round));
            out.extend(impl_had_a_target(round));
            out.extend(retro_one_line(round));
        }
        Role::Audit => {
            if let Some(integration) = round.integration {
                out.extend(integration_audit_concluded(round, integration));
            } else {
                // Conflict repair can leave an already completed table unchanged.
                // Use the starting snapshot so closing the last pending milestone
                // in this audit still requires current evidence.
                let unchanged_completed_table = round.before.is_some_and(|before| {
                    !before.milestones.is_empty()
                        && before
                            .milestones
                            .iter()
                            .all(|(_, state)| *state == MilestoneState::Done)
                        && crate::task_metrics::snapshot(round.status) == before.milestones
                });
                if !unchanged_completed_table {
                    out.extend(evidence_file(round, "audit"));
                }
                out.extend(audit_moved_the_table(round));
            }
            out.extend(retro_one_line(round));
            out.extend(audit_stayed_in_its_column(round));
        }
        // The design loop does not write evidence and does not touch the retro
        // file; the retro round writes `lessons.md`, which has its own schema
        // check at parse time.
        _ => {}
    }
    out.extend(design_doc_size(round));
    out
}

fn integration_audit_concluded(
    round: &Round<'_>,
    integration: &IntegrationAudit,
) -> Option<Finding> {
    if integration.evidence_touched {
        return None;
    }
    let unchanged = round
        .before
        .is_some_and(|before| crate::task_metrics::snapshot(round.status) == before.milestones);
    Some((if integration.legacy {
        Finding::warn
    } else {
        Finding::error
    })(
        if unchanged {
            "audit_made_no_progress"
        } else {
            "evidence_missing"
        },
        format!(
            "本轮是集成复验轮（实现 #{}；{}），没有写入本轮证据 evidence/{}。请逐项复验后写证据、表不动；或退回受影响里程碑 `开放`、`reopen` 加 1 并写证据。",
            integration.impl_round, integration.summary, integration.evidence
        ),
    ))
}

fn pointer_lines(text: &str) -> Vec<(usize, String, String)> {
    let mut pointers = Vec::new();
    let mut owner = None;
    let mut fence: Option<(char, usize)> = None;
    for (index, line) in text.lines().map(str::trim).enumerate() {
        let marker = line.chars().next().unwrap_or(' ');
        let count = line.chars().take_while(|c| *c == marker).count();
        if let Some((open, len)) = fence {
            if marker == open && count >= len && line[count..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if matches!(marker, '`' | '~') && count >= 3 {
            fence = Some((marker, count));
            continue;
        }
        if marker == '#' && (2..=6).contains(&count) && line[count..].starts_with(' ') {
            owner = line.match_indices("M-").find_map(|(at, _)| {
                let digits = line[at + 2..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                (digits > 0).then(|| line[at..at + 2 + digits].to_string())
            });
        }
        let pointer = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line);
        if pointer.starts_with("最新证据")
            && let Some(id) = &owner
        {
            pointers.push((index, id.clone(), pointer.to_string()));
        }
    }
    pointers
}

fn evidence_pointers(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut pointers = std::collections::BTreeMap::<String, Vec<String>>::new();
    for (_, id, pointer) in pointer_lines(text) {
        pointers.entry(id).or_default().push(pointer);
    }
    pointers
}

pub(crate) fn retargeted_closed_milestones(
    before: &str,
    after: &str,
    milestones: &[(String, MilestoneState)],
) -> Vec<String> {
    let old = evidence_pointers(before);
    let new = evidence_pointers(after);
    milestones
        .iter()
        .filter(|(id, state)| {
            *state == MilestoneState::Done
                && new.get(id).into_iter().flatten().any(|line| {
                    line.contains("-impl.md")
                        && !old.get(id).is_some_and(|lines| lines.contains(line))
                })
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// Replace only offending milestones' pointer lines, retaining every other byte.
/// Return None when there is no identified repair; never guess missing sections.
pub(crate) fn restore_closed_pointers(
    before: &str,
    after: &str,
    milestones: &[(String, MilestoneState)],
) -> Option<String> {
    let changed = retargeted_closed_milestones(before, after, milestones);
    if changed.is_empty() {
        return None;
    }
    let old_lines: Vec<_> = before.split_inclusive('\n').collect();
    let old = pointer_lines(before);
    let new = pointer_lines(after);
    let mut inserted = std::collections::BTreeSet::new();
    let mut result = String::new();
    for (index, line) in after.split_inclusive('\n').enumerate() {
        if let Some((_, id, _)) = new
            .iter()
            .find(|(at, id, _)| *at == index && changed.contains(id))
        {
            if inserted.insert(id) {
                for (at, _, _) in old.iter().filter(|(_, owner, _)| owner == id) {
                    result.push_str(old_lines[*at]);
                    if !old_lines[*at].ends_with('\n') && line.ends_with('\n') {
                        result.push('\n');
                    }
                }
            }
        } else {
            result.push_str(line);
        }
    }
    Some(result)
}

fn no_retargeting_closed_evidence(round: &Round<'_>) -> Option<Finding> {
    let before = round.before?;
    let old = evidence_pointers(round.design_before?);
    let new = evidence_pointers(round.design_after?);
    let mut changed = Vec::new();
    for (id, state) in &before.milestones {
        if *state != MilestoneState::Done {
            continue;
        }
        for line in new.get(id).into_iter().flatten() {
            if line.contains("-impl.md") && !old.get(id).is_some_and(|lines| lines.contains(line)) {
                changed.push(format!("{id}：{line}"));
            }
        }
    }
    if changed.is_empty() {
        return None;
    }
    Some(Finding::error(
        "impl_closed_milestone",
        format!(
            "实现轮把已完成里程碑验收契约的「最新证据」改指向了实现证据：{}。已完成里程碑的最新证据是审计复验留下的结论，实现证据替代不了它。本轮证据写进自己的 M-xx-r{}-impl.md 即可；集成改动由紧随其后的集成复验审计。自动还原未完成，请检查文档、工作区与提交错误。",
            changed.join("；"),
            round.status.impl_round
        ),
    ))
}

/// A loop round must leave a file the next one can read.
///
/// The role suffix is not decoration: an audit round and the implementation
/// round of the same `k` used to write the same path, and whichever ran second
/// overwrote the first. The audit does not increment `k`, so the collision was
/// not occasional.
fn evidence_file(round: &Round<'_>, suffix: &str) -> Option<Finding> {
    let k = round.status.impl_round;
    let wanted = format!("-r{k}-{suffix}.md");
    if round
        .evidence_files
        .iter()
        .any(|f| f.starts_with("M-") && f.ends_with(&wanted))
    {
        return None;
    }
    Some(Finding::error(
        "evidence_missing",
        format!(
            "本轮没有留下证据文件。实现轮与审计轮每轮都要写 \
             `docs/<slug>/evidence/M-xx-r{k}-{suffix}.md`——\
             文件名里的角色后缀不能省，否则同一个 k 的实现轮和审计轮会撞名，后跑的把先跑的覆盖掉。\
             现有的证据文件：{}",
            if round.evidence_files.is_empty() {
                "（一个都没有）".to_string()
            } else {
                round.evidence_files.join("、")
            }
        ),
    ))
}

/// The one rule the whole generation/evaluation split rests on.
fn no_closing_a_milestone(round: &Round<'_>) -> Option<Finding> {
    let before = round.before?;
    let closed: Vec<&str> = round
        .status
        .milestones
        .iter()
        .filter(|m| m.state == MilestoneState::Done)
        .filter(|m| {
            before
                .milestones
                .iter()
                .find(|(id, _)| id == &m.id)
                .is_some_and(|(_, s)| *s != MilestoneState::Done)
        })
        .map(|m| m.id.as_str())
        .collect();
    if closed.is_empty() {
        return None;
    }
    Some(Finding::error(
        "impl_closed_milestone",
        format!(
            "实现轮把 {} 标成了 `已完成`。只有审计轮独立复验通过才能关闭里程碑——\
             这是生成与评测分离在任务层面的体现，和 SAME-MODEL 是同一件事的两面。\
             Autome 不会替你把这一格改回去：改回去会让提交记录和会话日志说两套话。",
            closed.join("、")
        ),
    ))
}

/// An audit round has to move the milestone table.
///
/// Each of the three verdicts moves it: 通过 closes the milestone, 实现缺陷
/// reopens it, 验证缺口 closes it too (the audit strengthens the checks itself
/// and re-verifies). A table that came out of an audit unchanged means the
/// round reached a fourth verdict the protocol does not have.
///
/// A real run reached it eleven times. `M-07 端到端链路真机验收` needed a real
/// mouse, a real microphone and a person looking at a banner, so no session
/// could ever produce the evidence; the audit had no defect to point at either,
/// so it could not reopen the milestone. It invented "stays `待审`" instead.
/// The next implementation round then found no `开放` milestone, advanced
/// nothing, and the pair repeated — thirteen rounds and about two thirds of the
/// budget, all of it spent adding checks to a milestone nobody could close.
///
/// The way out is in the protocol already: acceptance a session cannot run does
/// not belong to a milestone, it belongs in `## 人工验收清单`, which the user
/// confirms before the merge. So this stops the task where it can still be
/// fixed cheaply, and the detail names both exits.
fn audit_moved_the_table(round: &Round<'_>) -> Option<Finding> {
    let before = round.before?;
    // A `待审` milestone is what an audit round is *for*. With none in the
    // table the round had nothing to reach a verdict on, and an unchanged
    // table is the normal way out: `Node::Implement` always schedules an
    // audit, so the round that follows the last milestone's closure sees a
    // table that is already all `已完成` and hands straight to the merge gate.
    // Cheap, and it short-circuits the comparison below on the common path.
    let stuck: Vec<&str> = round
        .status
        .milestones
        .iter()
        .filter(|m| m.state == MilestoneState::Pending)
        .map(|m| m.id.as_str())
        .collect();
    if stuck.is_empty() {
        return None;
    }
    // Compared against the same projection that *wrote* the snapshot
    // (`task_metrics::snapshot`, via `scheduler::apply_guards`), so the two
    // cannot drift into disagreeing about what "the table changed" means.
    if crate::task_metrics::snapshot(round.status) != before.milestones {
        return None;
    }
    Some(Finding::error(
        "audit_made_no_progress",
        format!(
            "审计轮结束时里程碑表和本轮开始时一模一样（{} 仍是 `待审`）。\
             审计结论只有三种，每一种都会动这张表：\
             通过就标 `已完成`，实现缺陷就退回 `开放`，验证缺口是审计当场补强检查再复验、\
             通过后照常关闭。表没动，说明本轮落在了协议里不存在的第四种结论上，\
             而下一个实现轮会发现没有 `开放` 的里程碑、无事可做——这一对会一直空转到预算耗尽。\
             两条出路：要么拿出产品行为错了的证据，退回 `开放`；\
             要么这个里程碑剩下的验收项本来就要真人（真实鼠标、麦克风、系统弹窗、肉眼看横幅），\
             那它就不该是里程碑的验收条件——把它们逐条移进设计文档的 `## 人工验收清单`，\
             由用户在合并前确认，然后关闭这个里程碑。",
            stuck.join("、")
        ),
    ))
}

/// An implementation round needs something to advance.
///
/// The question is about the table the round *started* from, not the one it
/// left: a round that did its job turns the one `开放` milestone into `待审`,
/// so by the end there is legitimately nothing open. Only `before` can answer
/// it.
///
/// With `audit_made_no_progress` in place the loop stops one round earlier and
/// this never fires. It is the second line: a task resumed by hand, or a design
/// round that left every milestone `待审`, arrives here too.
fn impl_had_a_target(round: &Round<'_>) -> Option<Finding> {
    if round.repairing {
        return None;
    }
    let before = round.before?;
    // `all` is true on an empty list, so the "no milestones yet" case falls
    // out of the third clause without a test of its own.
    if before
        .milestones
        .iter()
        .any(|(_, s)| *s == MilestoneState::Open)
        || before
            .milestones
            .iter()
            .all(|(_, s)| *s == MilestoneState::Done)
    {
        return None;
    }
    Some(Finding::error(
        "impl_had_no_target",
        "本轮开始时里程碑表里没有 `开放` 的里程碑，实现轮没有推进对象。\
         实现轮推进的是编号最小的 `开放` 里程碑；剩下的都卡在 `待审` 时，\
         该动的是审计轮，不是再开一个实现轮。",
    ))
}

/// The retro file takes one line a round.
fn retro_one_line(round: &Round<'_>) -> Vec<Finding> {
    let Some(before) = round.before else {
        return vec![];
    };
    let mut out = Vec::new();
    let added = round.retro_lines.saturating_sub(before.retro_lines);
    if added != 1 {
        out.push(Finding::warn(
            "retro_not_one_line",
            format!(
                "本轮给 retro.md 加了 {added} 行，协议要求每轮一行。\
                 叙述、理由、命令输出写进证据文件。"
            ),
        ));
    }
    for line in &round.retro_added {
        let chars = line.chars().count();
        if chars > RETRO_LINE_MAX_CHARS {
            out.push(Finding::warn(
                "retro_line_too_long",
                format!("retro.md 新增的那行有 {chars} 字，上限 {RETRO_LINE_MAX_CHARS} 字。"),
            ));
        }
    }
    out
}

/// The audit writes its conclusions to its own file, not into the design.
fn audit_stayed_in_its_column(round: &Round<'_>) -> Option<Finding> {
    if round.design_changed_outside_milestones != Some(true) {
        return None;
    }
    Some(Finding::warn(
        "audit_edited_the_design",
        "审计轮改了设计文档里程碑表以外的内容。审计结论写进 `<slug>-audit.md` 与证据文件，\
         设计文档只改里程碑那一格。",
    ))
}

/// The design document is read by every session at every turn.
fn design_doc_size(round: &Round<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    if round.design_bytes > DESIGN_DOC_MAX_BYTES {
        out.push(Finding::warn(
            "design_doc_large",
            format!(
                "设计文档 {} KB，超过 {} KB。三次真实运行分别长到 335 / 323 / 230 KB，\
                 其中约七成是逐轮追加的证据——每个会话开场都要读它，之后每次往返还带着它。",
                round.design_bytes / 1024,
                DESIGN_DOC_MAX_BYTES / 1024
            ),
        ));
    }
    if let Some(before) = round.before {
        let growth = round.design_bytes.saturating_sub(before.design_bytes);
        if growth > DESIGN_DOC_MAX_GROWTH_BYTES {
            out.push(Finding::warn(
                "design_doc_grew",
                format!(
                    "设计文档本轮长了 {} KB。设计文档写的是当前状态，不是日志；\
                     证据正文写进 `evidence/`，里程碑那里只留一行指针。",
                    growth / 1024
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::status_block::{ConvergenceMode, DocStatus, Milestone};

    fn milestone(id: &str, state: MilestoneState) -> Milestone {
        Milestone {
            id: id.into(),
            state,
            title: id.into(),
            reopen_count: 0,
            reopen_domains: vec![],
        }
    }

    fn status(k: u32, milestones: &[(&str, MilestoneState)]) -> StatusBlock {
        StatusBlock {
            status: DocStatus::Implementing,
            design_round: 2,
            design_round_limit: 15,
            impl_round: k,
            impl_round_limit: 35,
            current_milestone: None,
            current_milestone_reopens: 0,
            convergence_mode: ConvergenceMode::Normal,
            next_action: String::new(),
            repos: vec![],
            milestones: milestones.iter().map(|(id, s)| milestone(id, *s)).collect(),
            backlog: vec![],
            disputes: vec![],
            manual_items: vec![],
        }
    }

    fn snapshot(milestones: &[(&str, MilestoneState)], retro: usize, bytes: u64) -> Snapshot {
        Snapshot {
            milestones: milestones
                .iter()
                .map(|(id, s)| (id.to_string(), *s))
                .collect(),
            retro_lines: retro,
            design_bytes: bytes,
        }
    }

    fn round<'a>(
        role: Role,
        status: &'a StatusBlock,
        before: &'a Snapshot,
        evidence: &[&str],
    ) -> Round<'a> {
        Round {
            protocol_version: None,
            role,
            status,
            before: Some(before),
            retro_lines: before.retro_lines + 1,
            retro_added: vec![
                "实现 #3 | M-01 | 待审 | docs/x/evidence/M-01-r3-impl.md | 无".into(),
            ],
            design_bytes: before.design_bytes,
            evidence_files: evidence.iter().map(|s| s.to_string()).collect(),
            design_changed_outside_milestones: Some(false),
            repairing: false,
            integration: None,
            design_before: None,
            design_after: None,
        }
    }

    fn integration_findings(touched: bool, reopened: bool, files: &[&str]) -> Vec<Finding> {
        let before = snapshot(&[("M-01", MilestoneState::Done)], 4, 20_000);
        let s = status(
            99,
            &[(
                "M-01",
                if reopened {
                    MilestoneState::Open
                } else {
                    MilestoneState::Done
                },
            )],
        );
        let integration = IntegrationAudit {
            legacy: false,
            impl_round: 3,
            evidence: "M-01-r3-audit.md".into(),
            summary: "rebase 冲突 abc1234..def5678".into(),
            evidence_touched: touched,
        };
        let mut r = round(Role::Audit, &s, &before, files);
        r.integration = Some(&integration);
        check(&r)
    }

    #[test]
    fn integration_audit_with_a_prewritten_evidence_file_and_no_new_write_made_no_progress() {
        let f = integration_findings(false, false, &["M-01-r3-audit.md"]);
        assert_eq!(codes(&f), vec!["audit_made_no_progress"]);
    }
    #[test]
    fn integration_audit_with_a_still_table_and_no_evidence_made_no_progress() {
        let f = integration_findings(false, false, &[]);
        assert_eq!(codes(&f), vec!["audit_made_no_progress"]);
        for text in [
            "集成复验",
            "abc1234..def5678",
            "M-01-r3-audit.md",
            "开放",
            "reopen",
        ] {
            assert!(f[0].detail.contains(text));
        }
    }
    #[test]
    fn integration_audit_with_evidence_and_a_still_table_passes() {
        assert!(integration_findings(true, false, &["M-01-r3-audit.md"]).is_empty());
    }
    #[test]
    fn integration_audit_that_reopens_without_evidence_is_missing_evidence() {
        let f = integration_findings(false, true, &[]);
        assert_eq!(codes(&f), vec!["evidence_missing"]);
        assert!(f[0].detail.contains("M-01-r3-audit.md"));
    }
    #[test]
    fn integration_audit_that_reopens_with_evidence_passes() {
        assert!(integration_findings(true, true, &["M-01-r3-audit.md"]).is_empty());
    }
    #[test]
    fn integration_audit_evidence_of_an_earlier_round_does_not_count() {
        assert_eq!(
            codes(&integration_findings(false, false, &["M-01-r2-audit.md"])),
            vec!["audit_made_no_progress"]
        );
    }

    fn pointer_findings(old: Option<&str>, new: &str) -> Vec<Finding> {
        let before = snapshot(
            &[
                ("M-01", MilestoneState::Done),
                ("M-02", MilestoneState::Open),
            ],
            4,
            20_000,
        );
        let s = status(
            3,
            &[
                ("M-01", MilestoneState::Done),
                ("M-02", MilestoneState::Pending),
            ],
        );
        let mut r = round(Role::Impl, &s, &before, &["M-02-r3-impl.md"]);
        r.design_before = old;
        r.design_after = Some(new);
        check(&r)
    }
    #[test]
    fn closed_pointer_retargeted_to_impl_evidence_fails() {
        let old = "### M-01 验收契约\n最新证据：a-audit.md";
        let new = "### M-01 验收契约\n最新证据：a-impl.md";
        let f = pointer_findings(Some(old), new);
        assert_eq!(codes(&f), vec!["impl_closed_milestone"]);
        assert!(f[0].detail.contains("M-01"));
        assert!(pointer_findings(Some(old), "### M-01 验收契约\n最新证据：b-audit.md").is_empty());
        assert!(pointer_findings(Some(new), new).is_empty());
    }
    #[test]
    fn closed_pointer_added_under_a_closed_milestone_fails() {
        for prefix in ["", "- ", "* "] {
            assert_eq!(
                codes(&pointer_findings(
                    Some(""),
                    &format!("## M-01\n{prefix}最新证据：a-impl.md")
                )),
                vec!["impl_closed_milestone"]
            );
        }
    }
    #[test]
    fn closed_pointer_rules_leave_open_milestones_and_fences_alone() {
        let new = "最新证据：unowned-impl.md\n### M-01\n````markdown\n```\n最新证据：fenced-impl.md\n````\n~~~\n### M-01\n最新证据：fenced-impl.md\n~~~\n### unrelated\n最新证据：unowned-impl.md\n### M-02\n最新证据：open-impl.md";
        assert!(pointer_findings(Some(""), new).is_empty());
    }
    #[test]
    fn closed_pointer_without_a_baseline_is_not_guessed() {
        assert!(pointer_findings(None, "### M-01\n最新证据：a-impl.md").is_empty());
    }

    #[test]
    fn closed_pointer_restore_rewrites_only_the_pointer_lines() {
        let milestones = vec![
            ("M-01".into(), MilestoneState::Done),
            ("M-02".into(), MilestoneState::Open),
        ];
        let old = "# title\nstatus: old\n### M-01\n  - 最新证据：old-audit.md\ntext old\n### M-02\n最新证据：open-audit.md\n";
        let new = "# title\nstatus: new\n### M-01\n* 最新证据：new-impl.md\ntext new\n### M-02\n最新证据：open-impl.md\n";
        assert_eq!(
            restore_closed_pointers(old, new, &milestones).unwrap(),
            new.replace("* 最新证据：new-impl.md", "  - 最新证据：old-audit.md")
        );
        assert!(restore_closed_pointers(old, old, &milestones).is_none());
        assert!(restore_closed_pointers(old, "unrecognizable", &milestones).is_none());
        assert!(restore_closed_pointers("", "", &milestones).is_none());
    }

    #[test]
    fn closed_pointer_restore_handles_an_added_line_and_fences() {
        let milestones = vec![("M-01".into(), MilestoneState::Done)];
        for newline in ["\n", "\r\n"] {
            let old = "### M-01\n````md\n```\n最新证据：fenced-impl.md\n````\n~~~\n最新证据：tilde-impl.md\n~~~\nbody\n".replace('\n', newline);
            let new = format!(
                "{old}- 最新证据：added-impl.md{newline}* 最新证据：another-impl.md{newline}"
            );
            assert_eq!(
                restore_closed_pointers(&old, &new, &milestones).unwrap(),
                old
            );
        }
        assert_eq!(
            restore_closed_pointers(
                "### M-01\n最新证据：old-audit.md",
                "### M-01\n最新证据：new-impl.md",
                &milestones
            )
            .unwrap(),
            "### M-01\n最新证据：old-audit.md"
        );
    }

    #[test]
    fn legacy_pointer_integration_guard_warns_for_both_missing_evidence_branches() {
        for reopened in [false, true] {
            for touched in [false, true] {
                let before = snapshot(&[("M-01", MilestoneState::Done)], 4, 20_000);
                let s = status(
                    3,
                    &[(
                        "M-01",
                        if reopened {
                            MilestoneState::Open
                        } else {
                            MilestoneState::Done
                        },
                    )],
                );
                let integration = IntegrationAudit {
                    legacy: true,
                    impl_round: 3,
                    evidence: "M-01-r3-audit.md".into(),
                    summary: "legacy range".into(),
                    evidence_touched: touched,
                };
                let mut r = round(Role::Audit, &s, &before, &[]);
                r.integration = Some(&integration);
                let f = integration_audit_concluded(&r, &integration);
                if touched {
                    assert!(f.is_none());
                } else {
                    let f = f.unwrap();
                    assert_eq!(f.level, Level::Warning);
                    assert_eq!(
                        f.code,
                        if reopened {
                            "evidence_missing"
                        } else {
                            "audit_made_no_progress"
                        }
                    );
                }
            }
        }
    }

    fn codes(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.code).collect()
    }

    #[test]
    fn a_well_behaved_implementation_round_passes_everything() {
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn an_implementation_round_closing_a_milestone_stops_the_task() {
        // `开放` is what the round actually starts from: it is the state an
        // implementation round is given something to advance in.
        let s = status(3, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md"]);
        let f = check(&r);
        assert_eq!(codes(&f), vec!["impl_closed_milestone"]);
        assert_eq!(f[0].level, Level::Error);
        assert!(f[0].detail.contains("M-01"), "{}", f[0].detail);
    }

    #[test]
    fn a_milestone_the_audit_closed_earlier_is_not_blamed_on_the_next_round() {
        // M-01 was already `已完成` when this round started. Comparing against
        // the final table rather than against the previous one would fail
        // every implementation round after the first milestone closes.
        let s = status(
            4,
            &[
                ("M-01", MilestoneState::Done),
                ("M-02", MilestoneState::Pending),
            ],
        );
        let before = snapshot(
            &[
                ("M-01", MilestoneState::Done),
                ("M-02", MilestoneState::Open),
            ],
            4,
            20_000,
        );
        let r = round(Role::Impl, &s, &before, &["M-02-r4-impl.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn the_audit_round_may_close_a_milestone() {
        let s = status(3, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Pending)], 4, 20_000);
        let r = round(Role::Audit, &s, &before, &["M-01-r3-audit.md"]);
        assert_eq!(check(&r), vec![]);
    }

    /// The shape a real run got stuck in: six closed milestones and a seventh
    /// whose acceptance needed a person. Eleven audits in a row left this table
    /// exactly as they found it.
    fn stuck_at_m07() -> [(&'static str, MilestoneState); 7] {
        [
            ("M-01", MilestoneState::Done),
            ("M-02", MilestoneState::Done),
            ("M-03", MilestoneState::Done),
            ("M-04", MilestoneState::Done),
            ("M-05", MilestoneState::Done),
            ("M-06", MilestoneState::Done),
            ("M-07", MilestoneState::Pending),
        ]
    }

    #[test]
    fn an_audit_that_leaves_the_milestone_table_untouched_stops_the_task() {
        let s = status(31, &stuck_at_m07());
        let before = snapshot(&stuck_at_m07(), 56, 60_000);
        let r = round(Role::Audit, &s, &before, &["M-07-r31-audit.md"]);
        let f = check(&r);
        assert_eq!(codes(&f), vec!["audit_made_no_progress"]);
        assert_eq!(f[0].level, Level::Error);
        // The detail has to name the milestone and both ways out, or the
        // person who gets the stopped task has to go read the protocol.
        assert!(f[0].detail.contains("M-07"), "{}", f[0].detail);
        assert!(f[0].detail.contains("人工验收清单"), "{}", f[0].detail);
        assert!(f[0].detail.contains("开放"), "{}", f[0].detail);
    }

    #[test]
    fn an_audit_that_reopens_a_milestone_is_progress() {
        let mut after = stuck_at_m07();
        after[6].1 = MilestoneState::Open;
        let s = status(31, &after);
        let before = snapshot(&stuck_at_m07(), 56, 60_000);
        let r = round(Role::Audit, &s, &before, &["M-07-r31-audit.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn an_audit_that_closes_the_last_milestone_is_progress() {
        let mut after = stuck_at_m07();
        after[6].1 = MilestoneState::Done;
        let s = status(31, &after);
        let before = snapshot(&stuck_at_m07(), 56, 60_000);
        let r = round(Role::Audit, &s, &before, &["M-07-r31-audit.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn the_audit_after_the_last_milestone_closes_is_not_blamed_for_a_still_table() {
        // `Node::Implement` always schedules an audit, so one runs after the
        // table is already all `已完成`. It has no `待审` milestone to reach a
        // verdict on and leaves the table alone — which is the normal way out
        // of the loop, straight to the merge gate, not a stalled round.
        let done = [
            ("M-01", MilestoneState::Done),
            ("M-02", MilestoneState::Done),
        ];
        let s = status(9, &done);
        let before = snapshot(&done, 12, 20_000);
        let r = round(Role::Audit, &s, &before, &["M-02-r9-audit.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn rebase_repair_with_all_milestones_done_does_not_require_empty_audit_evidence() {
        use autome_domain::config::{GateConfig, GlobalConfig, ProjectConfig, resolve};
        use autome_domain::task::{
            Context, CoreStepResult, GateOutcome, Node, PendingDecisions, SessionOutcome,
            TaskState, Trigger, apply,
        };

        let config = resolve(
            &GlobalConfig::default(),
            &ProjectConfig {
                gate: Some(GateConfig {
                    command: "true".into(),
                    timeout_minutes: 5,
                }),
                ..Default::default()
            },
        );
        let ctx = Context {
            bug_test_defect: false,
            handoff_pending: false,
            config: &config,
            budget_n: Some(35),
            decisions: PendingDecisions::default(),
        };
        let done = [
            ("M-01", MilestoneState::Done),
            ("M-02", MilestoneState::Done),
        ];
        let s = status(8, &done);
        let before = snapshot(&done, 12, 20_000);
        let repaired = apply(
            &TaskState::Active { node: Node::Rebase },
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Conflict {
                    files: vec!["src/example.rs".into()],
                    detail: "rebase conflict".into(),
                },
            },
            &ctx,
        )
        .unwrap();
        assert_eq!(
            repaired.next,
            TaskState::Active {
                node: Node::Implement
            }
        );

        // The repair writes implementation evidence but leaves the closed table alone.
        let evidence = ["M-02-r7-audit.md", "M-02-r8-impl.md"];
        let impl_round = round(Role::Impl, &s, &before, &evidence);
        assert_eq!(check(&impl_round), vec![]);
        let gated = apply(
            &repaired.next,
            &Trigger::SessionEnded {
                outcome: SessionOutcome::Ok {
                    status: Box::new(s.clone()),
                },
            },
            &ctx,
        )
        .unwrap();
        assert_eq!(gated.next, TaskState::Active { node: Node::Gate });
        let mut next = apply(
            &gated.next,
            &Trigger::GateDone {
                node: Node::Gate,
                outcome: GateOutcome::Passed,
                status: Some(Box::new(s.clone())),
            },
            &ctx,
        )
        .unwrap()
        .next;

        // Either skip this empty audit or admit it without inventing milestone evidence.
        if next == (TaskState::Active { node: Node::Audit }) {
            let mut audit = round(Role::Audit, &s, &before, &evidence);
            audit.retro_added = vec!["审计 #8 | 无 | 无事可审 | 无 | 无".into()];
            let findings = check(&audit);
            let outcome = match findings.iter().find(|f| f.level == Level::Error) {
                Some(f) => SessionOutcome::GuardFailed {
                    detail: f.detail.clone(),
                },
                None => SessionOutcome::Ok {
                    status: Box::new(s.clone()),
                },
            };
            next = apply(&next, &Trigger::SessionEnded { outcome }, &ctx)
                .unwrap()
                .next;
        }
        assert!(
            matches!(
                next,
                TaskState::Active {
                    node: Node::Retro | Node::Rebase
                }
            ),
            "a completed task must continue after conflict repair, got {next:?}"
        );
    }

    #[test]
    fn an_audit_that_closes_a_pending_milestone_still_requires_current_evidence() {
        let s = status(8, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Pending)], 12, 20_000);
        let r = round(Role::Audit, &s, &before, &["M-01-r7-audit.md"]);
        let findings = check(&r);
        assert_eq!(codes(&findings), vec!["evidence_missing"]);
        assert_eq!(findings[0].level, Level::Error);
    }

    #[test]
    fn an_implementation_round_with_nothing_open_stops_the_task() {
        let s = status(31, &stuck_at_m07());
        let before = snapshot(&stuck_at_m07(), 56, 60_000);
        let r = round(Role::Impl, &s, &before, &["M-07-r31-impl.md"]);
        let f = check(&r);
        assert_eq!(codes(&f), vec!["impl_had_no_target"]);
        assert_eq!(f[0].level, Level::Error);
    }

    #[test]
    fn a_finished_table_is_not_a_missing_target() {
        // Every milestone closed is how the implementation loop ends, not a
        // round that had nothing to do.
        let done = [
            ("M-01", MilestoneState::Done),
            ("M-02", MilestoneState::Done),
        ];
        let s = status(9, &done);
        let before = snapshot(&done, 12, 20_000);
        let r = round(Role::Impl, &s, &before, &["M-02-r9-impl.md"]);
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn a_loop_round_with_no_evidence_file_stops_the_task() {
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let r = round(Role::Impl, &s, &before, &[]);
        let f = check(&r);
        assert_eq!(codes(&f), vec!["evidence_missing"]);
        assert_eq!(f[0].level, Level::Error);
    }

    #[test]
    fn an_evidence_file_without_the_role_suffix_does_not_count() {
        // The collision this suffix exists to prevent: the audit does not
        // increment `k`, so `M-01-r3.md` is written by both rounds of round 3
        // and the second one wins.
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let r = round(Role::Impl, &s, &before, &["M-01-r3.md"]);
        assert_eq!(codes(&check(&r)), vec!["evidence_missing"]);
    }

    #[test]
    fn an_evidence_file_from_an_earlier_round_does_not_count_for_this_one() {
        let s = status(7, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let r = round(
            Role::Impl,
            &s,
            &before,
            &["M-01-r3-impl.md", "M-01-r5-impl.md"],
        );
        assert_eq!(codes(&check(&r)), vec!["evidence_missing"]);
    }

    #[test]
    fn the_design_loop_is_not_asked_for_evidence_files() {
        let s = status(0, &[]);
        let before = snapshot(&[], 0, 4_000);
        let mut r = round(Role::Plan, &s, &before, &[]);
        r.retro_lines = 0;
        r.retro_added = vec![];
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn a_retro_entry_of_more_than_one_line_is_a_warning_not_a_failure() {
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let mut r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md"]);
        r.retro_lines = before.retro_lines + 6;
        let f = check(&r);
        assert_eq!(codes(&f), vec!["retro_not_one_line"]);
        assert_eq!(f[0].level, Level::Warning);
    }

    #[test]
    fn a_two_hundred_character_retro_line_is_fine_and_a_longer_one_is_not() {
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let mut r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md"]);
        r.retro_added = vec!["实".repeat(RETRO_LINE_MAX_CHARS)];
        assert_eq!(check(&r), vec![]);
        r.retro_added = vec!["实".repeat(RETRO_LINE_MAX_CHARS + 1)];
        assert_eq!(codes(&check(&r)), vec!["retro_line_too_long"]);
    }

    #[test]
    fn an_audit_that_edited_the_design_beyond_the_table_is_warned_about() {
        let s = status(3, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Pending)], 4, 20_000);
        let mut r = round(Role::Audit, &s, &before, &["M-01-r3-audit.md"]);
        r.design_changed_outside_milestones = Some(true);
        let f = check(&r);
        assert_eq!(codes(&f), vec!["audit_edited_the_design"]);
        assert_eq!(f[0].level, Level::Warning);
    }

    #[test]
    fn an_undeterminable_diff_produces_no_finding_rather_than_a_guess() {
        let s = status(3, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Pending)], 4, 20_000);
        let mut r = round(Role::Audit, &s, &before, &["M-01-r3-audit.md"]);
        r.design_changed_outside_milestones = None;
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn a_design_document_over_the_size_budget_is_warned_about_on_every_role() {
        for role in [Role::Plan, Role::Impl, Role::Audit] {
            let s = status(3, &[("M-01", MilestoneState::Pending)]);
            let before = snapshot(
                &[("M-01", MilestoneState::Open)],
                4,
                DESIGN_DOC_MAX_BYTES + 1,
            );
            let mut r = round(role, &s, &before, &["M-01-r3-impl.md", "M-01-r3-audit.md"]);
            r.design_bytes = DESIGN_DOC_MAX_BYTES + 1;
            if role == Role::Plan {
                r.retro_lines = before.retro_lines;
                r.retro_added = vec![];
            }
            assert!(
                codes(&check(&r)).contains(&"design_doc_large"),
                "{role} was not warned"
            );
        }
    }

    #[test]
    fn a_sudden_ten_kilobyte_jump_is_warned_about_even_under_the_cap() {
        let s = status(3, &[("M-01", MilestoneState::Pending)]);
        let before = snapshot(&[("M-01", MilestoneState::Open)], 4, 20_000);
        let mut r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md"]);
        r.design_bytes = 20_000 + DESIGN_DOC_MAX_GROWTH_BYTES + 1;
        assert_eq!(codes(&check(&r)), vec!["design_doc_grew"]);
    }

    #[test]
    fn the_first_round_has_nothing_to_compare_against_and_is_not_penalised() {
        let s = status(1, &[("M-01", MilestoneState::Pending)]);
        let r = Round {
            protocol_version: None,
            role: Role::Impl,
            status: &s,
            before: None,
            retro_lines: 1,
            retro_added: vec!["实现 #1 | M-01 | 待审 | e | 无".into()],
            design_bytes: 20_000,
            evidence_files: vec!["M-01-r1-impl.md".into()],
            design_changed_outside_milestones: None,
            repairing: false,
            integration: None,
            design_before: None,
            design_after: None,
        };
        assert_eq!(check(&r), vec![]);
    }

    #[test]
    fn a_round_repairing_a_red_gate_needs_no_open_milestone() {
        let s = status(31, &stuck_at_m07());
        let before = snapshot(&stuck_at_m07(), 56, 60_000);
        let mut r = round(Role::Impl, &s, &before, &["M-07-r31-impl.md"]);
        assert_eq!(codes(&check(&r)), vec!["impl_had_no_target"]);
        r.repairing = true;
        assert_eq!(
            check(&r),
            vec![],
            "sent back by the gate, it fixes rather than advances"
        );
    }
}
