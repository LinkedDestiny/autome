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

use std::collections::BTreeMap;

use autome_domain::role::Role;
use autome_domain::status_block::{Milestone, MilestoneState, StatusBlock};

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
    pub role: Role,
    pub slug: &'a str,
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
         这份简报是索引，不是设计文档的替代品。设计文档 `docs/{}/{}.md` 仍然是\
         权威，本轮需要什么就去读什么——但先读这里，多数轮次不必整份拉进来。\n",
        input.role.round_name(),
        input.round,
        input.slug,
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
            }
        }
        None => s.push_str("还没有里程碑表，或者所有里程碑都已完成。\n"),
    }

    s.push_str("\n## 上一轮对手轮的结论\n\n");
    s.push_str(&counterpart(input));

    let sections = input.map.for_role(input.role);
    if !sections.loop_protocol.is_empty() || !sections.session_protocol.is_empty() {
        s.push_str("\n## 本轮相关的协议小节\n\n");
        s.push_str(&format!(
            "全文在 `docs/{}/protocol/`。下面是与本轮直接相关的几节。\n",
            input.slug
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
                    "审计文件里没有提到 {id}。全文在 `docs/{}/{}-audit.md`。\n",
                    input.slug, input.slug
                ),
            }
        }
        Role::Audit => {
            let Some(id) = id else {
                return "没有待审的里程碑。\n".to_string();
            };
            match latest_evidence(input.evidence, &id, "impl") {
                Some(file) => format!(
                    "本轮要复验的是 {id}，实现轮的证据在 `docs/{}/evidence/{file}`。\n\n\
                     **先不要读它。** 独立复验的意思是自己跑验收命令、自己构造\
                     判别检查；读完实现轮的推理再去验证，验的就是那套推理而不是产品。\n",
                    input.slug
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

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::status_block::{ConvergenceMode, DocStatus};

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
            role,
            slug: "checkout",
            round: 3,
            status: Some(status),
            audit_doc: None,
            evidence,
            loop_protocol: PROTOCOL,
            session_protocol: "# 会话协议\n\n## 状态块\n\n字段表。\n",
            map,
            budget_line: Some("本轮是实现轮第 3 轮，N = 35。\n".into()),
            decisions: None,
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
}
