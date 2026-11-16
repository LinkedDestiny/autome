//! The four paragraphs a round is handed before it opens anything.
//!
//! Every session used to start by reading the whole design document. Across
//! three real runs those documents reached 230–335KB, and the document is not
//! read once: it is in the context for every turn that follows. The round
//! needed four things out of it, and paid for all of it.
//!
//! So the core assembles those four things into `docs/<slug>/brief/<node>-<k>.md`
//! and the prompt says to read that first. The design document is still there
//! and still authoritative — the brief is an index, not a replacement, and it
//! says so in its own first line. What changes is that reading the whole thing
//! becomes a choice the round makes when it needs to.
//!
//! What is deliberately *not* in it: "the design section for this milestone".
//! Finding that reliably needs a column in the milestone table saying where
//! each milestone is specified, and that is a change to the format the core
//! parses — the one part of the protocol that cannot be changed cheaply. A
//! brief that guessed the section would be worse than one that does not offer
//! it, because a round that trusts a wrong pointer stops looking.

use autome_domain::config::{GateRun, ResolvedConfig};
use serde_json::Value;
use std::collections::BTreeMap;

/// A skipped round must not inherit a previous commit's gate verdict.
pub const GATE_ONLY_BEFORE_MERGE: &str = "本项目门禁只在合并前运行，本轮没有宿主门禁结果。\n";

pub fn repair_gate_evidence(result: &Value) -> String {
    format!(
        "本轮是修复轮：宿主{}在 `{}` 未通过；失败清单与输出末尾在本轮提示词里，完整日志 `{}`。\n",
        if result["kind"] == "quick" {
            "快检"
        } else {
            "合并前门禁"
        },
        result["sha"].as_str().unwrap_or_default(),
        result["log"].as_str().unwrap_or_default()
    )
}

pub fn gate_evidence(
    config: &ResolvedConfig,
    last: Option<&Value>,
    head: Option<&str>,
) -> Option<String> {
    if config.gate.is_none() || config.gate_run == GateRun::EachRound {
        return last.and_then(|result| {
            gate_line_for_mode(result, config.gate_run == GateRun::BeforeMerge)
        });
    }
    if let (Some(result), Some(head)) = (last, head)
        && result.get("sha").and_then(Value::as_str) == Some(head)
    {
        return gate_line_of_result(result);
    }
    Some(
        if config.quick_gate().is_some() {
            "本项目全量门禁只在合并前运行；本轮的提交上还没有宿主快检结果。\n"
        } else {
            GATE_ONLY_BEFORE_MERGE
        }
        .to_owned(),
    )
}

use autome_domain::role::Role;
use autome_domain::status_block::{Milestone, MilestoneState, StatusBlock};

/// One line for the audit round's brief: what the core's own run of the gate
/// found, on which commit, and where the full output is.
pub fn gate_line_of_result(result: &Value) -> Option<String> {
    gate_line_for_mode(result, true)
}

fn gate_line_for_mode(result: &Value, before_merge: bool) -> Option<String> {
    let sha = result.get("sha")?.as_str()?;
    let command = result.get("command")?.as_str()?;
    let ok = result.get("ok")?.as_bool()?;
    let code = result
        .get("exit_code")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let log = result.get("log").and_then(Value::as_str).unwrap_or("");
    let short = &sha[..sha.len().min(10)];
    if result["kind"].as_str() == Some("quick") {
        let policy = if before_merge {
            "全量门禁只在合并前运行。"
        } else {
            "当前配置已改为每轮运行完整门禁，这条结果是切换前的快检。"
        };
        let verdict =
            autome_domain::gate_failure::kernel_timeout(result["tail"].as_str().unwrap_or(""))
                .filter(|_| !ok)
                .map_or_else(
                    || {
                        if ok {
                            "通过".to_owned()
                        } else {
                            "未通过".to_owned()
                        }
                    },
                    |n| format!("快检超时（运行超过 {n} 分钟，被内核终止）"),
                );
        return Some(format!(
            "本轮宿主只跑了快检：`{command}`，在提交 `{short}` 上{verdict}，退出码 {code}。完整输出：`{log}`。\n快检不是全量门禁：它只说明这条命令里的检查是否通过，不能当作全量回归的证据；{policy}复验时可以引用它，但它不替代你自己的负向对照。\n"
        ));
    }
    let verdict =
        autome_domain::gate_failure::kernel_timeout(result["tail"].as_str().unwrap_or(""))
            .filter(|_| !ok)
            .map_or_else(
                || format!("{}（退出码 {code}）", if ok { "通过" } else { "未通过" }),
                |n| format!("门禁超时（运行超过 {n} 分钟，被内核终止）"),
            );

    Some(format!(
        "Autome 在提交 `{short}` 上运行了 `{command}`：{verdict}。完整输出：`{log}`。\n\
         这是实现轮之外的一次独立运行。复验时可以引用它，但它不替代你自己的负向对照。\n"
    ))
}

/// Which protocol sections each role's brief carries, read from the protocol
/// version's own `brief-map.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BriefMap {
    roles: BTreeMap<Role, RoleSections>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoleSections {
    pub loop_protocol: Vec<String>,
    pub session_protocol: Vec<String>,
}

impl BriefMap {
    pub fn for_role(&self, role: Role) -> RoleSections {
        self.roles.get(&role).cloned().unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }

    /// Headings the map names that the protocol does not have. Layer 1 of the
    /// eval gate reports these: a mapping that points at a heading which was
    /// renamed produces a brief with a section silently missing, and a round
    /// that was never shown a rule cannot follow it.
    pub fn dangling(&self, loop_protocol: &str, session_protocol: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (role, sections) in &self.roles {
            for (text, wanted, file) in [
                (loop_protocol, &sections.loop_protocol, "loop-protocol.md"),
                (
                    session_protocol,
                    &sections.session_protocol,
                    "session-protocol.md",
                ),
            ] {
                for heading in wanted {
                    if section(text, heading).is_none() {
                        out.push(format!("{}: {file} 里没有「{heading}」", role.as_str()));
                    }
                }
            }
        }
        out
    }
}

/// Parses `brief-map.toml`.
///
/// Hand-written rather than via `toml::from_str` into a typed struct for one
/// reason: an unknown role key has to be an error naming the key, not a
/// silently dropped table. A typo'd role in the map means that role's brief
/// quietly loses its protocol sections, which is exactly the kind of failure
/// that shows up months later as "the audit round stopped doing X".
pub fn parse_map(text: &str) -> Result<BriefMap, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut map = BriefMap::default();
    let Some(roles) = doc.get("roles").and_then(toml::Value::as_table) else {
        return Ok(map);
    };
    for (name, table) in roles {
        let Some(role) = Role::parse(name) else {
            return Err(format!(
                "brief-map.toml 里有未知角色 `{name}`，可选：{}",
                Role::ALL
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
        };
        let Some(table) = table.as_table() else {
            return Err(format!("[roles.{name}] 不是一个表"));
        };
        let list = |key: &str| -> Result<Vec<String>, String> {
            match table.get(key) {
                None => Ok(vec![]),
                Some(toml::Value::Array(items)) => items
                    .iter()
                    .map(|i| {
                        i.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| format!("[roles.{name}].{key} 里有不是字符串的项"))
                    })
                    .collect(),
                Some(_) => Err(format!("[roles.{name}].{key} 不是一个数组")),
            }
        };
        map.roles.insert(
            role,
            RoleSections {
                loop_protocol: list("loop-protocol")?,
                session_protocol: list("session-protocol")?,
            },
        );
    }
    Ok(map)
}

/// The text under a Markdown heading, heading line included, up to the next
/// heading of the same or a higher level.
///
/// Matched on a prefix so that a heading may gain a clarifying tail without
/// orphaning every brief that named it.
pub fn section(text: &str, heading: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| {
        l.starts_with('#') && l.trim_start_matches('#').trim().starts_with(heading)
    })?;
    let level = lines[start].chars().take_while(|c| *c == '#').count();
    let mut out = vec![lines[start]];
    for line in &lines[start + 1..] {
        if line.starts_with('#') {
            let this = line.chars().take_while(|c| *c == '#').count();
            if this <= level {
                break;
            }
        }
        out.push(line);
    }
    // Trailing blank lines belong to the gap, not to the section.
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    Some(out.join("\n"))
}

/// Everything the brief is assembled from. Read by the caller so this module
/// does no I/O.
pub struct Inputs<'a> {
    pub gate_history: Option<String>,
    pub integration: Option<String>,
    pub role: Role,
    pub slug: &'a str,
    /// The task's document directory as the session sees it — the same value
    /// the prompt's `{doc_dir}` gets. `docs/<slug>` for a single repository
    /// on the default root, but not for a workspace or a configured root.
    pub doc_dir: &'a str,
    pub round: u32,
    pub status: Option<&'a StatusBlock>,
    /// The audit document, for an implementation round.
    pub audit_doc: Option<&'a str>,
    /// Evidence filenames, newest last.
    pub evidence: &'a [String],
    pub loop_protocol: &'a str,
    pub session_protocol: &'a str,
    pub map: &'a BriefMap,
    /// The budget line the prompt also carries, repeated here so the brief is
    /// self-contained.
    pub budget_line: Option<String>,
    /// What the user decided at the last stopping point, already rendered.
    pub decisions: Option<String>,
    /// What the core's own run of the project gate found on the commit this
    /// round is looking at, already rendered. For the audit round: the full
    /// suite has been run once by someone other than the round being audited.
    pub gate: Option<String>,
    pub scope_patches: Option<String>,
}

/// The milestone this round is about: the current one if the status block
/// names a live one, else the lowest-numbered open or pending one.
pub fn focus(status: &StatusBlock) -> Option<&Milestone> {
    if let Some(id) = &status.current_milestone
        && let Some(m) = status.milestones.iter().find(|m| &m.id == id)
        && m.state != MilestoneState::Done
    {
        return Some(m);
    }
    status
        .milestones
        .iter()
        .find(|m| m.state == MilestoneState::Open)
        .or_else(|| {
            status
                .milestones
                .iter()
                .find(|m| m.state == MilestoneState::Pending)
        })
}

pub fn build(input: &Inputs<'_>) -> String {
    let mut s = format!(
        "# {} 简报 · 第 {} 轮\n\n\
         这份简报是索引，不是设计文档的替代品。设计文档 `{}/{}.md` 仍然是\
         权威，本轮需要什么就去读什么——但先读这里，多数轮次不必整份拉进来。\n",
        input.role.round_name(),
        input.round,
        input.doc_dir,
        input.slug
    );

    s.push_str("\n## 本里程碑\n\n");
    match input.status.and_then(focus) {
        Some(m) => {
            s.push_str("| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n");
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                m.id,
                m.state.as_str(),
                m.title,
                m.reopen_count,
                m.reopen_domains.join(" ")
            ));
            if let Some(status) = input.status {
                s.push_str(&format!(
                    "\n里程碑共 {} 个，已完成 {} 个。收敛模式 `{}`。\n",
                    status.milestones.len(),
                    status.milestones_done(),
                    status.convergence_mode.as_str()
                ));
                let evidence_round = match input.role {
                    Role::Impl => Some((input.round, "impl")),
                    Role::Audit => Some((status.impl_round, "audit")),
                    _ => None,
                };
                if let Some((round, suffix)) = evidence_round {
                    s.push_str(&format!(
                        "\n本轮证据必须写入 `{}/evidence/{}-r{round}-{suffix}.md`。\
                         文件名中的轮号是实现轮号，审计会话序号不用于证据命名。\n",
                        input.doc_dir, m.id
                    ));
                }
            }
        }
        None => s.push_str("还没有里程碑表，或者所有里程碑都已完成。\n"),
    }

    if let Some(history) = &input.gate_history {
        s.push_str(history);
    }
    if let Some(section) = &input.integration {
        s.push_str(section);
    }
    s.push_str("\n## 上一轮对手轮的结论\n\n");
    s.push_str(&counterpart(input));

    let sections = input.map.for_role(input.role);
    if !sections.loop_protocol.is_empty() || !sections.session_protocol.is_empty() {
        s.push_str("\n## 本轮相关的协议小节\n\n");
        s.push_str(&format!(
            "全文在 `{}/protocol/`。下面是与本轮直接相关的几节。\n",
            input.doc_dir
        ));
        for (text, headings) in [
            (input.loop_protocol, &sections.loop_protocol),
            (input.session_protocol, &sections.session_protocol),
        ] {
            for heading in headings {
                if let Some(body) = section(text, heading) {
                    s.push('\n');
                    s.push_str(&body);
                    s.push('\n');
                }
            }
        }
    }

    s.push_str("\n## 预算与用户表态\n\n");
    match &input.budget_line {
        Some(line) => s.push_str(line),
        None => s.push_str("本轮没有预算行。\n"),
    }
    if let Some(d) = &input.decisions {
        s.push_str(d);
    }
    if let Some(g) = &input.gate {
        s.push_str("\n## 宿主门禁\n\n");
        s.push_str(g);
    }
    if let Some(patches) = &input.scope_patches {
        s.push_str(patches);
    }
    s
}

/// What the other side of the pair concluded last round, about this milestone.
///
/// The implementation round gets the audit's verdict; the audit round gets a
/// pointer to the evidence it is meant to be checking rather than the evidence
/// itself, because reading the implementation round's reasoning is exactly
/// what an independent re-verification must not start from.
fn counterpart(input: &Inputs<'_>) -> String {
    let id = input.status.and_then(focus).map(|m| m.id.clone());
    match input.role {
        Role::Impl => {
            let Some(doc) = input.audit_doc else {
                return "还没有审计轮跑过。\n".to_string();
            };
            let Some(id) = id else {
                return "没有对应的里程碑。\n".to_string();
            };
            match milestone_paragraphs(doc, &id) {
                Some(body) => format!("审计文件里关于 {id} 的部分：\n\n{body}\n"),
                None => format!(
                    "审计文件里没有提到 {id}。全文在 `{}/{}-audit.md`。\n",
                    input.doc_dir, input.slug
                ),
            }
        }
        Role::Audit => {
            let Some(id) = id else {
                return "没有待审的里程碑。\n".to_string();
            };
            match latest_evidence(input.evidence, &id, "impl") {
                Some(file) => format!(
                    "本轮要复验的是 {id}，实现轮的证据在 `{}/evidence/{file}`。\n\n\
                     **先不要读它。** 独立复验的意思是自己跑验收命令、自己构造\
                     判别检查；读完实现轮的推理再去验证，验的就是那套推理而不是产品。\n",
                    input.doc_dir
                ),
                None => format!("{id} 还没有实现轮的证据文件。\n"),
            }
        }
        Role::Retro => "本轮读的是整轮运行，不是某一个对手轮。\n".to_string(),
        _ => "设计循环没有对手轮文件。\n".to_string(),
    }
}

/// The paragraphs of a document that mention a milestone id.
fn milestone_paragraphs(doc: &str, id: &str) -> Option<String> {
    let kept: Vec<&str> = doc.split("\n\n").filter(|p| p.contains(id)).collect();
    (!kept.is_empty()).then(|| kept.join("\n\n"))
}

/// The newest evidence file for a milestone and role.
fn latest_evidence(files: &[String], id: &str, role: &str) -> Option<String> {
    let suffix = format!("-{role}.md");
    files
        .iter()
        .filter(|f| f.starts_with(&format!("{id}-r")) && f.ends_with(&suffix))
        .max_by_key(|f| round_of(f))
        .cloned()
}

fn round_of(name: &str) -> u32 {
    name.split("-r")
        .nth(1)
        .and_then(|rest| rest.split('-').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Where a round's brief lives, relative to the worktree.
pub fn path(doc_dir: &str, role: Role, round: u32) -> String {
    format!("{doc_dir}/brief/{}-{round}.md", role.as_str())
}

/// Render saved facts; launching a session never has to recompute patch stats.
#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::status_block::{ConvergenceMode, DocStatus};

    fn before_merge_config() -> ResolvedConfig {
        use autome_domain::config::{self, GateConfig, GlobalConfig, ProjectConfig};
        let mut config = config::resolve(&GlobalConfig::default(), &ProjectConfig::default());
        config.gate = Some(GateConfig {
            command: "make check".into(),
            timeout_minutes: 5,
        });
        config.gate_run = GateRun::BeforeMerge;
        config
    }

    #[test]
    fn before_merge_missing_stale_or_unreadable_head_has_no_result() {
        let config = before_merge_config();
        let stale = serde_json::json!({"sha":"old", "command":"make check", "ok":false});
        let missing_sha = serde_json::json!({"command":"make check", "ok":false});
        for (last, head) in [
            (None, Some("head")),
            (Some(&stale), Some("head")),
            (Some(&stale), None),
            (None, None),
            (Some(&missing_sha), None),
            (Some(&missing_sha), Some("head")),
        ] {
            assert_eq!(
                gate_evidence(&config, last, head).as_deref(),
                Some(GATE_ONLY_BEFORE_MERGE)
            );
        }
    }

    #[test]
    fn before_merge_same_head_preserves_success_and_failure() {
        let config = before_merge_config();
        for sha in ["a", "0123456789", "0123456789abcdef"] {
            for ok in [true, false] {
                let result = serde_json::json!({"sha":sha, "command":"make check", "ok":ok, "exit_code":if ok {0} else {1}, "log":"gate.log"});
                let line = gate_evidence(&config, Some(&result), Some(sha));
                assert_eq!(line, gate_line_of_result(&result));
                assert!(
                    line.unwrap()
                        .contains(if ok { "：通过" } else { "：未通过" })
                );
            }
        }
    }

    #[test]
    fn before_merge_each_round_and_unconfigured_gate_are_byte_identical() {
        let result = serde_json::json!({"sha":"old", "command":"make check", "ok":false});
        let mut each_round = before_merge_config();
        each_round.gate_run = GateRun::EachRound;
        let mut no_gate = before_merge_config();
        no_gate.gate = None;
        for config in [each_round, no_gate] {
            for last in [None, Some(&result)] {
                for head in [None, Some("old"), Some("new")] {
                    assert_eq!(
                        gate_evidence(&config, last, head),
                        last.and_then(gate_line_of_result)
                    );
                }
            }
        }
    }

    const MAP: &str = r#"
[roles.impl]
loop-protocol = ["实现循环"]
session-protocol = ["状态块"]

[roles.audit]
loop-protocol = ["实现循环"]
session-protocol = []
"#;

    const PROTOCOL: &str = "# 协议\n\n\
        ## 共同原则\n\n甲\n\n\
        ## 实现循环\n\n推进最小编号的开放里程碑。\n\n\
        ### 自审清单\n\n四项。\n\n\
        ## 里程碑\n\n表格在这里。\n";

    fn milestone(id: &str, state: MilestoneState, reopen: u32) -> Milestone {
        Milestone {
            id: id.into(),
            state,
            title: format!("{id} 的标题"),
            reopen_count: reopen,
            reopen_domains: vec![],
        }
    }

    fn status(current: Option<&str>, milestones: Vec<Milestone>) -> StatusBlock {
        StatusBlock {
            status: DocStatus::Implementing,
            design_round: 2,
            design_round_limit: 15,
            impl_round: 3,
            impl_round_limit: 35,
            current_milestone: current.map(str::to_string),
            current_milestone_reopens: 0,
            convergence_mode: ConvergenceMode::Normal,
            next_action: String::new(),
            repos: vec![],
            milestones,
            backlog: vec![],
            disputes: vec![],
            manual_items: vec![],
        }
    }

    fn inputs<'a>(
        role: Role,
        status: &'a StatusBlock,
        map: &'a BriefMap,
        evidence: &'a [String],
    ) -> Inputs<'a> {
        Inputs {
            gate_history: None,
            integration: None,
            role,
            slug: "checkout",
            doc_dir: "docs/checkout",
            round: 3,
            status: Some(status),
            audit_doc: None,
            evidence,
            loop_protocol: PROTOCOL,
            session_protocol: "# 会话协议\n\n## 状态块\n\n字段表。\n",
            map,
            budget_line: Some("本轮是实现轮第 3 轮，N = 35。\n".into()),
            decisions: None,
            gate: None,
            scope_patches: None,
        }
    }

    #[test]
    fn a_map_names_the_sections_each_role_gets() {
        let map = parse_map(MAP).unwrap();
        assert_eq!(map.for_role(Role::Impl).loop_protocol, vec!["实现循环"]);
        assert_eq!(map.for_role(Role::Impl).session_protocol, vec!["状态块"]);
        assert!(map.for_role(Role::Audit).session_protocol.is_empty());
        // A role the map does not mention gets no sections rather than a panic.
        assert!(map.for_role(Role::Plan).loop_protocol.is_empty());
    }

    #[test]
    fn an_unknown_role_in_the_map_is_an_error_naming_it() {
        // A typo here would silently drop that role's protocol sections, and
        // the symptom months later is "the audit round stopped doing X".
        let e = parse_map("[roles.auditor]\nloop-protocol = []\n").unwrap_err();
        assert!(e.contains("auditor"), "{e}");
        assert!(e.contains("audit"), "{e}");
    }

    #[test]
    fn a_heading_the_protocol_no_longer_has_is_reported() {
        let map = parse_map("[roles.impl]\nloop-protocol = [\"早就删了的一节\"]\n").unwrap();
        let dangling = map.dangling(PROTOCOL, "");
        assert_eq!(dangling.len(), 1, "{dangling:?}");
        assert!(dangling[0].contains("早就删了的一节"), "{dangling:?}");
    }

    #[test]
    fn a_section_runs_to_the_next_heading_of_its_level_and_keeps_its_children() {
        let s = section(PROTOCOL, "实现循环").unwrap();
        assert!(s.starts_with("## 实现循环"), "{s}");
        assert!(s.contains("### 自审清单"), "{s}");
        assert!(s.contains("四项。"), "{s}");
        assert!(!s.contains("## 里程碑"), "{s}");
        assert!(
            !s.ends_with('\n'),
            "trailing blank lines are the gap: {s:?}"
        );
    }

    #[test]
    fn a_missing_section_is_absent_rather_than_the_rest_of_the_file() {
        assert_eq!(section(PROTOCOL, "不存在的一节"), None);
    }

    #[test]
    fn the_focus_is_the_current_milestone_when_it_is_still_live() {
        let s = status(
            Some("M-02"),
            vec![
                milestone("M-01", MilestoneState::Open, 0),
                milestone("M-02", MilestoneState::Pending, 1),
            ],
        );
        assert_eq!(focus(&s).unwrap().id, "M-02");
    }

    #[test]
    fn a_current_milestone_that_has_closed_falls_back_to_the_next_open_one() {
        // The status block names the milestone the last round worked on, and
        // the audit may have closed it. Pointing the next round at a finished
        // milestone would be worse than pointing it nowhere.
        let s = status(
            Some("M-01"),
            vec![
                milestone("M-01", MilestoneState::Done, 0),
                milestone("M-02", MilestoneState::Open, 0),
            ],
        );
        assert_eq!(focus(&s).unwrap().id, "M-02");
    }

    #[test]
    fn an_open_milestone_is_preferred_over_a_pending_one() {
        let s = status(
            None,
            vec![
                milestone("M-01", MilestoneState::Pending, 0),
                milestone("M-02", MilestoneState::Open, 0),
            ],
        );
        assert_eq!(focus(&s).unwrap().id, "M-02");
    }

    #[test]
    fn a_brief_says_it_is_an_index_and_names_the_authority() {
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-02"),
            vec![milestone("M-02", MilestoneState::Open, 1)],
        );
        let brief = build(&inputs(Role::Impl, &s, &map, &[]));
        assert!(brief.contains("不是设计文档的替代品"), "{brief}");
        assert!(brief.contains("docs/checkout/checkout.md"), "{brief}");
    }

    #[test]
    fn a_brief_carries_the_milestone_row_and_the_mapped_protocol_sections() {
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-02"),
            vec![milestone("M-02", MilestoneState::Open, 1)],
        );
        let brief = build(&inputs(Role::Impl, &s, &map, &[]));
        assert!(
            brief.contains("| M-02 | 开放 | M-02 的标题 | 1 |"),
            "{brief}"
        );
        assert!(brief.contains("## 实现循环"), "{brief}");
        assert!(brief.contains("### 自审清单"), "{brief}");
        assert!(brief.contains("## 状态块"), "{brief}");
        // Not the sections it was not mapped to.
        assert!(!brief.contains("## 共同原则"), "{brief}");
    }

    #[test]
    fn an_implementation_round_is_given_the_audit_paragraphs_about_its_milestone() {
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-02"),
            vec![milestone("M-02", MilestoneState::Open, 1)],
        );
        let audit =
            "# 审计\n\n## 审计 #3\n\nM-01 通过。\n\nM-02 退回：转义写反了。\n\n无关的一段。\n";
        let mut i = inputs(Role::Impl, &s, &map, &[]);
        i.audit_doc = Some(audit);
        let brief = build(&i);
        assert!(brief.contains("M-02 退回：转义写反了。"), "{brief}");
        assert!(!brief.contains("无关的一段"), "{brief}");
    }

    #[test]
    fn an_audit_round_is_given_the_path_and_told_not_to_read_it_first() {
        // Independence is the whole point of the pair. A brief that pasted the
        // implementation round's reasoning in would verify the reasoning.
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-02"),
            vec![milestone("M-02", MilestoneState::Pending, 0)],
        );
        let evidence = vec![
            "M-02-r3-impl.md".to_string(),
            "M-02-r5-impl.md".to_string(),
            "M-01-r1-impl.md".to_string(),
        ];
        let brief = build(&inputs(Role::Audit, &s, &map, &evidence));
        assert!(brief.contains("M-02-r5-impl.md"), "{brief}");
        assert!(
            !brief.contains("M-02-r3-impl.md"),
            "the newest one: {brief}"
        );
        assert!(brief.contains("先不要读它"), "{brief}");
    }

    #[test]
    fn every_path_in_the_brief_is_the_one_the_session_can_reach() {
        // A workspace session starts beside the member checkouts, so its
        // documents are `<docs-member>/docs/<slug>`. The brief once said
        // `docs/<slug>` regardless, contradicting the prompt beside it.
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-02"),
            vec![milestone("M-02", MilestoneState::Pending, 0)],
        );
        let evidence = vec!["M-02-r5-impl.md".to_string()];
        let mut input = inputs(Role::Audit, &s, &map, &evidence);
        input.doc_dir = "docs-repo/docs/checkout";
        let brief = build(&input);
        assert!(
            brief.contains("`docs-repo/docs/checkout/checkout.md`"),
            "{brief}"
        );
        assert!(
            brief.contains("`docs-repo/docs/checkout/evidence/M-02-r5-impl.md`"),
            "{brief}"
        );
        assert!(!brief.contains("`docs/checkout"), "{brief}");
    }

    #[test]
    fn regression_evidence_destination_audit_uses_implementation_round() {
        let map = parse_map(MAP).unwrap();
        let mut s = status(
            Some("M-03"),
            vec![milestone("M-03", MilestoneState::Pending, 0)],
        );
        // Implementation #3 failed its gate; #4 passed, so this is audit #3.
        s.impl_round = 4;
        let evidence = vec!["M-03-r4-impl.md".to_string()];
        let input = inputs(Role::Audit, &s, &map, &evidence);
        assert_eq!(
            path(input.doc_dir, input.role, input.round),
            "docs/checkout/brief/audit-3.md"
        );

        // Exercise the actual admission guard: changing only the evidence
        // filename from the audit ordinal to the implementation round fixes it.
        let mut round = crate::guards::Round {
            protocol_version: None,
            role: Role::Audit,
            status: &s,
            before: None,
            retro_lines: 0,
            retro_added: vec![],
            design_bytes: 0,
            evidence_files: vec!["M-03-r3-audit.md".into()],
            design_changed_outside_milestones: None,
            repairing: false,
            integration: None,
            design_before: None,
            design_after: None,
        };
        assert!(
            crate::guards::check(&round)
                .iter()
                .any(|f| f.code == "evidence_missing")
        );
        round.evidence_files = vec!["M-03-r4-audit.md".into()];
        assert!(
            !crate::guards::check(&round)
                .iter()
                .any(|f| f.code == "evidence_missing")
        );

        let brief = build(&input);
        let wanted = "docs/checkout/evidence/M-03-r4-audit.md";
        assert!(
            brief.contains(wanted),
            "missing current evidence destination: {wanted}\n{brief}"
        );
        assert!(!brief.contains("docs/checkout/evidence/M-03-r3-audit.md"));
    }

    #[test]
    fn regression_evidence_destination_impl_uses_current_session_round() {
        let map = parse_map(MAP).unwrap();
        // The design document is read before implementation #4 updates it.
        let s = status(
            Some("M-03"),
            vec![milestone("M-03", MilestoneState::Open, 0)],
        );
        let mut input = inputs(Role::Impl, &s, &map, &[]);
        input.round = 4;
        let brief = build(&input);
        let wanted = "docs/checkout/evidence/M-03-r4-impl.md";
        assert!(
            brief.contains(wanted),
            "missing current evidence destination: {wanted}\n{brief}"
        );
        assert!(!brief.contains("docs/checkout/evidence/M-03-r3-impl.md"));
    }

    #[test]
    fn regression_evidence_destination_preserves_session_doc_directory() {
        let map = parse_map(MAP).unwrap();
        let mut s = status(
            Some("M-03"),
            vec![milestone("M-03", MilestoneState::Pending, 0)],
        );
        s.impl_round = 4;
        let mut input = inputs(Role::Audit, &s, &map, &[]);
        input.doc_dir = "docs-repo/custom-docs/checkout";
        let brief = build(&input);
        let wanted = "docs-repo/custom-docs/checkout/evidence/M-03-r4-audit.md";
        assert!(
            brief.contains(wanted),
            "missing current evidence destination: {wanted}\n{brief}"
        );
    }

    #[test]
    fn a_first_implementation_round_is_told_there_is_no_audit_yet() {
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-01"),
            vec![milestone("M-01", MilestoneState::Open, 0)],
        );
        let brief = build(&inputs(Role::Impl, &s, &map, &[]));
        assert!(brief.contains("还没有审计轮跑过"), "{brief}");
    }

    #[test]
    fn the_budget_line_is_repeated_so_the_brief_stands_alone() {
        let map = parse_map(MAP).unwrap();
        let s = status(
            Some("M-01"),
            vec![milestone("M-01", MilestoneState::Open, 0)],
        );
        let brief = build(&inputs(Role::Impl, &s, &map, &[]));
        assert!(brief.contains("N = 35"), "{brief}");
    }

    #[test]
    fn a_task_with_no_milestone_table_yet_still_gets_a_brief() {
        let map = parse_map(MAP).unwrap();
        let s = status(None, vec![]);
        let brief = build(&inputs(Role::Impl, &s, &map, &[]));
        assert!(brief.contains("还没有里程碑表"), "{brief}");
    }

    #[test]
    fn the_brief_path_is_per_role_and_per_round() {
        assert_eq!(
            path("docs/checkout", Role::Impl, 7),
            "docs/checkout/brief/impl-7.md"
        );
    }
    #[test]
    fn gate_quick_brief_actual_result_and_timeout_in_both_modes() {
        for (ok, code, tail) in [
            (true, 0, ""),
            (false, 1, "failure"),
            (false, 124, "=== Autome：门禁运行超过 5 分钟，已终止 ==="),
        ] {
            let result = serde_json::json!({"sha":"0123456789abcdef", "kind":"quick", "command":"echo {slug}","ok":ok,"exit_code":code,"tail":tail,"log":"gate.log"});
            for mode in [GateRun::BeforeMerge, GateRun::EachRound] {
                let mut config = before_merge_config();
                config.gate_run = mode;
                let line = gate_evidence(&config, Some(&result), Some("0123456789abcdef")).unwrap();
                assert!(line.contains("本轮宿主只跑了快检：`echo {slug}`"));
                assert!(line.contains(&format!("退出码 {code}")));
                assert!(line.contains("不能当作全量回归的证据"));
                assert!(!line.contains("这是实现轮之外的一次独立运行"));
                assert_eq!(
                    line.contains("当前配置已改为每轮运行完整门禁"),
                    mode == GateRun::EachRound
                );
                if code == 124 {
                    assert!(line.contains("快检超时（运行超过 5 分钟"));
                }
            }
        }
    }

    #[test]
    fn gate_quick_brief_stale_missing_and_legacy_results() {
        let mut config = before_merge_config();
        config.gate_quick = Some(autome_domain::config::QuickGate {
            session: None,
            command: "quick".into(),
            timeout_minutes: 5,
        });
        let old = serde_json::json!({"sha":"old","kind":"quick","command":"quick","ok":true,"exit_code":0});
        for last in [None, Some(&old)] {
            for head in [None, Some("new"), Some("")] {
                assert_eq!(
                    gate_evidence(&config, last, head).unwrap(),
                    "本项目全量门禁只在合并前运行；本轮的提交上还没有宿主快检结果。\n"
                );
            }
        }
        let original = serde_json::json!({"sha":"a","command":"full","ok":true,"exit_code":0});
        for kind in ["full", "repro"] {
            let mut result = original.clone();
            result["kind"] = kind.into();
            assert_eq!(
                gate_evidence(&config, Some(&result), Some("a")),
                gate_line_of_result(&original)
            );
        }
        assert!(gate_line_of_result(&serde_json::Value::Null).is_none());
    }
}

/// Render only kernel-recorded facts; never read implementation evidence.
pub fn integration_section(event: &serde_json::Value, doc_dir: &str) -> String {
    let string = |v: &serde_json::Value| v.as_str().unwrap_or("").to_owned();
    let short = |v: &serde_json::Value| string(v).chars().take(7).collect::<String>();
    let mut out = format!(
        "\n## 本轮要审的集成改动\n\n本轮是**集成复验**：实现 #{} 开始时里程碑表没有 `开放` / `待审`，却在 `{doc_dir}/` 之外留下了提交。规则见 `{doc_dir}/protocol/loop-protocol.md` 的「集成复验」一节。\n\n触发原因：",
        event["impl_round"]
    );
    if event["legacy"] == true {
        out = out.replace(&format!("规则见 `{doc_dir}/protocol/loop-protocol.md` 的「集成复验」一节。"),
            "冻结的旧协议没有集成复验规则；本节由内核补充。请核对下列目录外提交，独立复跑验收并记录结论；未写证据时内核只提醒。");
    }
    let trigger = &event["trigger"];
    match trigger["kind"].as_str() {
        Some("rebase_conflict") => {
            out.push_str("rebase 冲突。冲突文件：\n");
            if let Some(files) = trigger["files"].as_array() {
                for file in files {
                    out.push_str(&format!("- {}\n", string(file)));
                }
            }
        }
        Some("gate_failed") => out.push_str(&format!(
            "合并前门禁失败。命令：`{}`。输出末尾：\n```text\n{}\n```\n",
            string(&trigger["command"]),
            string(&trigger["tail"])
        )),
        Some("decisions") => out.push_str("用户表态。\n"),
        Some("scope_patch") => out.push_str("越界补丁交接。\n"),
        _ => out.push_str("没有注入：手动重跑或续跑。\n"),
    }
    if let Some(repos) = event["repos"].as_array() {
        for repo in repos {
            let name = string(&repo["repo"]);
            let prefix = if name.is_empty() {
                String::new()
            } else {
                format!("{name}/")
            };
            if !name.is_empty() {
                out.push_str(&format!("\n### 仓库 {name}\n"));
            }
            out.push_str(&format!("\n提交范围：`{prefix}{}..{}`（默认分支基线 `{}`），rebase 前后补丁不变的提交不列出。\n", short(&repo["start"]), short(&repo["end"]), short(&repo["base"])));
            for (key, label) in [
                ("commits", "列出的提交"),
                ("dropped", "rebase 前有、rebase 后找不到同一补丁的提交"),
            ] {
                let commits = repo[key].as_array().map(Vec::as_slice).unwrap_or(&[]);
                out.push_str(&format!("\n{label} {} 个：\n", commits.len()));
                for commit in commits {
                    out.push_str(&format!(
                        "- `{prefix}{}` {}\n",
                        short(&commit["sha"]),
                        string(&commit["subject"])
                    ));
                    if let Some(paths) = commit["paths"].as_array() {
                        for path in paths {
                            out.push_str(&format!("  - {prefix}{}\n", string(path)));
                        }
                    }
                    // The bounded stat's continuation hint contains the full SHA.
                    let stat = string(&commit["stat"])
                        .replace(&string(&commit["sha"]), &short(&commit["sha"]));
                    out.push_str(&format!(
                        "\n改动统计（`{doc_dir}/` 之外）：\n```text\n{stat}\n```\n"
                    ));
                }
                let omitted = repo[format!("{key}_omitted")].as_u64().unwrap_or(0);
                if omitted > 0 {
                    out.push_str(&format!(
                        "其余 {omitted} 个提交未展示，请按上述范围查看。\n"
                    ));
                }
            }
            out.push_str("\n改到的测试文件（逐个看 diff，留意测试前提有没有被改弱）：\n");
            if let Some(files) = repo["test_files"].as_array() {
                for file in files {
                    out.push_str(&format!("- {}\n", string(file)));
                }
            }
        }
    }
    out.push_str(&format!("\n本轮证据必须写入 `{doc_dir}/evidence/{}`。结论二选一：通过（里程碑表不动）；或把受影响的里程碑退回 `开放`、`reopen` 加 1。两种都要写这个证据文件。\n", string(&event["evidence"])));
    out
}

#[cfg(test)]
mod integration_section_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn integration_section_none_preserves_the_brief() {
        let map = BriefMap::default();
        let mut input = Inputs {
            scope_patches: None,
            gate_history: None,
            integration: None,
            role: Role::Audit,
            slug: "x",
            doc_dir: "docs/x",
            round: 1,
            status: None,
            audit_doc: None,
            evidence: &[],
            loop_protocol: "",
            session_protocol: "",
            map: &map,
            budget_line: None,
            decisions: None,
            gate: None,
        };
        let old = build(&input);
        assert!(!old.contains("## 本轮要审的集成改动"));
        let section = integration_section(
            &json!({"impl_round":9,"evidence":"M-08-r9-audit.md"}),
            "docs/x",
        );
        input.integration = Some(section.clone());
        let new = build(&input);
        assert_eq!(new.replacen(&section, "", 1), old);
        assert!(new.find("## 本轮要审").unwrap() < new.find("## 上一轮").unwrap());
        input.scope_patches = Some("补丁交接回归标记".to_string());
        let combined = build(&input);
        assert!(combined.contains(&section));
        assert!(combined.contains("补丁交接回归标记"));
    }
    #[test]
    fn integration_section_contains_facts_and_seven_character_shas() {
        let event = json!({"impl_round":9,"evidence":"M-08-r9-audit.md", "trigger":{"kind":"gate_failed","command":"make check","tail":"error: assertion failed","sha":"failure0123456789"}, "repos":[{"repo":"api","start":"123456789012345","end":"abcdef012345678","base":"987654321098765","commits":[{"sha":"fedcba987654321","subject":"repair","paths":["src/a.rs"],"stat":"src/a.rs | 2 +-\n其余 3 行见 git show --stat fedcba987654321"}],"dropped":[{"sha":"765432109876543","subject":"lost","paths":["old.rs"],"stat":"（读取失败：denied）"}],"commits_omitted":1,"dropped_omitted":2,"test_files":["api/tests/a.rs"]}]});
        let s = integration_section(&event, "docs/x");
        for term in [
            "make check",
            "error: assertion failed",
            "api/1234567..abcdef0",
            "9876543",
            "api/fedcba9",
            "api/7654321",
            "api/tests/a.rs",
            "api/old.rs",
            "读取失败",
            "其余 1 个",
            "其余 2 个",
            "git show --stat fedcba9",
        ] {
            assert!(s.contains(term), "{term}");
        }
        assert!(!s.contains("api/api/"));
        for term in ["failure012", "1234567890", "fedcba98765", "未通过"] {
            assert!(!s.contains(term), "{term}");
        }
        for (trigger, expected) in [
            (
                json!({"kind":"rebase_conflict","files":["api/a.rs"]}),
                "api/a.rs",
            ),
            (json!({"kind":"decisions"}), "用户表态"),
            (json!({"kind":"none"}), "没有注入"),
        ] {
            let mut event = event.clone();
            event["trigger"] = trigger;
            assert!(integration_section(&event, "docs/x").contains(expected));
        }
    }
    #[test]
    fn integration_section_exact_evidence_path_and_empty_range() {
        let s = integration_section(
            &json!({"impl_round":0,"evidence":"M-01-r0-audit.md","repos":[]}),
            "workspace/docs/x",
        );
        assert!(s.contains("`workspace/docs/x/evidence/M-01-r0-audit.md`"));
        assert!(s.contains("通过（里程碑表不动）"));
        assert!(s.contains("`reopen` 加 1"));
        assert!(!s.contains("### 仓库"));
        assert!(!s.contains("提交范围："));
    }
}
