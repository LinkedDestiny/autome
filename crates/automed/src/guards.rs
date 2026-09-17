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
//! **The core does not rewrite a session's document.** It would be easy to
//! flip the cell back and carry on, and it would leave the commit history and
//! the session log saying different things about what happened. The task stops
//! and a person looks.

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

/// Everything a check needs, gathered by the caller so this module does no I/O
/// and stays testable without a repository.
pub struct Round<'a> {
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
    /// Paths the session changed, repository-relative.
    pub changed_paths: Vec<String>,
    /// Whether the session's edit to the design document touched anything
    /// beyond the milestone table. `None` when it could not be determined.
    pub design_changed_outside_milestones: Option<bool>,
}

/// Runs every check that applies to this round.
pub fn check(round: &Round<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    match round.role {
        Role::Impl => {
            out.extend(evidence_file(round, "impl"));
            out.extend(no_closing_a_milestone(round));
            out.extend(retro_one_line(round));
        }
        Role::Audit => {
            out.extend(evidence_file(round, "audit"));
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
            milestones: milestones
                .iter()
                .map(|(id, s)| milestone(id, *s))
                .collect(),
            backlog: vec![],
            disputes: vec![],
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
            role,
            status,
            before: Some(before),
            retro_lines: before.retro_lines + 1,
            retro_added: vec!["实现 #3 | M-01 | 待审 | docs/x/evidence/M-01-r3-impl.md | 无".into()],
            design_bytes: before.design_bytes,
            evidence_files: evidence.iter().map(|s| s.to_string()).collect(),
            changed_paths: vec![],
            design_changed_outside_milestones: Some(false),
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
        let s = status(3, &[("M-01", MilestoneState::Done)]);
        let before = snapshot(&[("M-01", MilestoneState::Pending)], 4, 20_000);
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
            &[("M-01", MilestoneState::Done), ("M-02", MilestoneState::Pending)],
        );
        let before = snapshot(
            &[("M-01", MilestoneState::Done), ("M-02", MilestoneState::Open)],
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
        let r = round(Role::Impl, &s, &before, &["M-01-r3-impl.md", "M-01-r5-impl.md"]);
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
            role: Role::Impl,
            status: &s,
            before: None,
            retro_lines: 1,
            retro_added: vec!["实现 #1 | M-01 | 待审 | e | 无".into()],
            design_bytes: 20_000,
            evidence_files: vec!["M-01-r1-impl.md".into()],
            changed_paths: vec![],
            design_changed_outside_milestones: None,
        };
        assert_eq!(check(&r), vec![]);
    }
}
