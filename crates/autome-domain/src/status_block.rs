//! Parser for the design document's status block, milestone table, Backlog
//! and 争议项 sections. Technical design §5.4.
//!
//! This is the single most load-bearing parser in the system: after every
//! session ends, `automed` reads `docs/<slug>/<slug>.md` and the result
//! decides which node runs next (§5.3). The design therefore makes the format
//! strict and the failure mode loud — "字段缺失或格式错误一次即判
//! protocol_error" — rather than guessing at a half-written document and
//! advancing the task on a misreading.
//!
//! The format, which the 2.0 task-file template mandates verbatim:
//!
//! ```text
//! status: 实现中
//! design-round: 3/15
//! implementation-round: 4/25
//! current-milestone: M-03
//! current-milestone-reopens: 1
//! convergence-mode: normal
//! next-action: 补 promo.spec.ts 大小写用例
//!
//! ## 里程碑
//!
//! | ID | 状态 | 标题 | reopen | 领域 |
//! |---|---|---|---|---|
//! | M-01 | 已完成 | 购物车数据模型 | 0 | |
//! | M-02 | 待审 | 结算接口 | 1 | promo-case |
//!
//! ## Backlog
//!
//! - B-01 优惠码使用次数上限
//!
//! ## 争议项
//!
//! - D3-P02 优惠码是否大小写敏感
//! ```
//!
//! 1.x rendered the same information less regularly (free-form milestone
//! prose, a `round: 1/20` ledger). 2.0 does not try to parse those: the
//! template is regenerated per task, so every task this system runs produces
//! the strict shape above.

use serde::{Deserialize, Serialize};

/// `status:` — the design document's own view of where the task is. The
/// scheduler cross-checks it against the node it dispatched, and treats a
/// contradiction as a protocol error rather than trusting either side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocStatus {
    Designing,
    Implementing,
    Done,
    Infeasible,
    ProtocolFailure,
}

impl DocStatus {
    const NAMES: [(&'static str, DocStatus); 5] = [
        ("设计中", DocStatus::Designing),
        ("实现中", DocStatus::Implementing),
        ("已完成", DocStatus::Done),
        ("不可实现", DocStatus::Infeasible),
        ("协议失败", DocStatus::ProtocolFailure),
    ];

    pub fn parse(s: &str) -> Option<DocStatus> {
        Self::NAMES.iter().find(|(n, _)| *n == s).map(|(_, v)| *v)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            DocStatus::Designing => "设计中",
            DocStatus::Implementing => "实现中",
            DocStatus::Done => "已完成",
            DocStatus::Infeasible => "不可实现",
            DocStatus::ProtocolFailure => "协议失败",
        }
    }
}

/// `convergence-mode:` — escalating scrutiny after repeated reopens. The
/// domain only records it; the escalation rules themselves live in the task
/// file the agents read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceMode {
    Normal,
    DomainReview,
    MilestoneReview,
}

impl ConvergenceMode {
    pub fn parse(s: &str) -> Option<ConvergenceMode> {
        match s {
            "normal" => Some(ConvergenceMode::Normal),
            "domain-review" => Some(ConvergenceMode::DomainReview),
            "milestone-review" => Some(ConvergenceMode::MilestoneReview),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ConvergenceMode::Normal => "normal",
            ConvergenceMode::DomainReview => "domain-review",
            ConvergenceMode::MilestoneReview => "milestone-review",
        }
    }
}

/// The three milestone states from the 1.x protocol, unchanged: an
/// implementation round may move a milestone to `Pending` but never to
/// `Done` — only an audit round closes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneState {
    Open,
    Pending,
    Done,
}

impl MilestoneState {
    pub fn parse(s: &str) -> Option<MilestoneState> {
        match s {
            "开放" => Some(MilestoneState::Open),
            "待审" => Some(MilestoneState::Pending),
            "已完成" => Some(MilestoneState::Done),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            MilestoneState::Open => "开放",
            MilestoneState::Pending => "待审",
            MilestoneState::Done => "已完成",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub state: MilestoneState,
    pub title: String,
    pub reopen_count: u32,
    /// Stable behaviour-domain names, used by the convergence rules. Empty is
    /// normal and carries no meaning beyond "no reopen yet".
    pub reopen_domains: Vec<String>,
}

/// One `## Backlog` entry: a non-blocking improvement awaiting the user's
/// call (requirement T-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklogItem {
    pub id: String,
    pub text: String,
}

/// One `## 人工验收清单` entry: a check only a person can carry out — a real
/// mouse, a real microphone, a banner seen with eyes. It never blocks a
/// milestone; the user ticks it off before pressing merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualItem {
    pub id: String,
    pub text: String,
    /// `- [x] H-01 …`. Untouched items are what "still to confirm" counts.
    pub ticked: bool,
}

/// One `## 争议项` entry: a claim frozen after two re-raises, awaiting the
/// user's ruling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisputeItem {
    pub id: String,
    pub text: String,
}

/// Everything `automed` reads out of a design document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusBlock {
    pub status: DocStatus,
    pub design_round: u32,
    pub design_round_limit: u32,
    pub impl_round: u32,
    pub impl_round_limit: u32,
    /// `None` when the document writes `无`/`-`, which is legitimate before
    /// the first milestone exists.
    pub current_milestone: Option<String>,
    pub current_milestone_reopens: u32,
    pub convergence_mode: ConvergenceMode,
    pub next_action: String,
    /// The repositories this task works in, named by the intake round in a
    /// workspace project (`repos: 仓库A, 仓库B`).
    ///
    /// **Optional, and it has to stay optional.** A missing required field
    /// fails the whole document, so making this one required would turn every
    /// task document already written — and every fixture and eval case — into
    /// a protocol failure at once. Empty means "whatever the project already
    /// decided", which is the single-repository answer and therefore
    /// backwards-compatible by construction.
    pub repos: Vec<String>,
    pub milestones: Vec<Milestone>,
    pub backlog: Vec<BacklogItem>,
    pub disputes: Vec<DisputeItem>,
    pub manual_items: Vec<ManualItem>,
}

impl StatusBlock {
    pub fn milestones_done(&self) -> usize {
        self.milestones
            .iter()
            .filter(|m| m.state == MilestoneState::Done)
            .count()
    }

    pub fn has_open_milestone(&self) -> bool {
        self.milestones
            .iter()
            .any(|m| m.state == MilestoneState::Open)
    }

    /// True when every milestone is `Done` — the audit-side condition for
    /// leaving the implementation loop (§5.3).
    pub fn all_milestones_done(&self) -> bool {
        !self.milestones.is_empty()
            && self
                .milestones
                .iter()
                .all(|m| m.state == MilestoneState::Done)
    }

    /// True when no milestone is still `Open` — the condition for leaving the
    /// implementation loop when the audit role is disabled (requirement C-05).
    pub fn no_open_milestones(&self) -> bool {
        !self.milestones.is_empty() && !self.has_open_milestone()
    }
}

/// Why a document could not be read. Every variant names the offending line
/// or field, so the failure surfaced to the user says what to fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParseError {
    MissingField {
        field: String,
    },
    /// A recognised field whose value does not parse.
    BadValue {
        field: String,
        value: String,
    },
    DuplicateField {
        field: String,
    },
    /// The milestone table exists but a row is malformed.
    BadMilestoneRow {
        line: usize,
        reason: String,
    },
    DuplicateMilestoneId {
        id: String,
    },
    /// `current-milestone` names a milestone the table does not contain.
    UnknownCurrentMilestone {
        id: String,
    },
    /// The document says it has finished designing, but lists no milestones.
    /// The implementation loop would have nothing to advance.
    NoMilestonesAfterDesign,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::MissingField { field } => write!(f, "缺少状态字段 `{field}`"),
            ParseError::BadValue { field, value } => {
                write!(f, "状态字段 `{field}` 的值无法解析：{value}")
            }
            ParseError::DuplicateField { field } => write!(f, "状态字段 `{field}` 重复出现"),
            ParseError::BadMilestoneRow { line, reason } => {
                write!(f, "里程碑表第 {line} 行格式错误：{reason}")
            }
            ParseError::DuplicateMilestoneId { id } => write!(f, "里程碑 ID `{id}` 重复"),
            ParseError::UnknownCurrentMilestone { id } => {
                write!(f, "current-milestone 指向表中不存在的里程碑 `{id}`")
            }
            ParseError::NoMilestonesAfterDesign => {
                write!(f, "status 已离开「设计中」，但里程碑表是空的")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Where the scanner sits relative to a Markdown table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRegion {
    /// Not in a table. A pipe-bearing line here is prose.
    Outside,
    /// The previous line could have been a header row; `milestone` records
    /// whether it was *the milestone table's* header. Only a separator
    /// immediately below makes either answer real.
    AfterCandidateHeader { milestone: bool },
    /// Inside the milestone table's body. Every pipe line here is a row and
    /// must parse — a typo in a real row is still an error, which is the
    /// point of not simply skipping anything that fails to look like a
    /// milestone.
    Body,
    /// Inside some *other* table in the same section. The section holds prose
    /// and, as of the first real implementation round, an acceptance-evidence
    /// table per milestone. Its rows are skipped, not parsed: `| 验收项 | 结果
    /// | 证据 |` has a header and a separator like any table, and reading its
    /// `通过` as a milestone state failed the task.
    OtherTable,
}

/// Whether a header row belongs to the milestone table. The first column is
/// the discriminator: the milestone table's is `ID` (or `里程碑`), and every
/// other table in this section names something else.
///
/// A blank first cell used to qualify as well, as a tolerance for a milestone
/// table written without an `ID` header. That tolerance cost a real task.
/// Implementation round #16 of 2026-09-16 wrote a two-way comparison table
/// whose header was `| | 启动对齐 | 运行中的通道交接 |`, 1300 lines below the
/// milestone table and still inside the same 里程碑 section. The blank cell
/// opened a table body, the next row's `` `loadSchedules()` `` was read as a
/// milestone state, and the document — seven correctly parsed milestones
/// included — was rejected as a protocol error that stopped the task.
///
/// A blank header cell is ordinary in a comparison table and says nothing
/// about milestones, so it no longer qualifies. A milestone table that really
/// does omit the header now fails as `NoMilestonesAfterDesign`, which names
/// the actual problem instead of blaming an unrelated row.
///
/// The first column is not enough on its own. A design round wrote a
/// `## 里程碑依赖` section — a legitimate thing to write — whose table is
/// `| ID | 前置 | 说明 |`. Its first column is `ID`, so it opened a milestone
/// table body, and the first row's `—` under 前置 was read as a milestone
/// state. The task was failed for a protocol error it had not committed.
///
/// So the *second* column decides: a milestone row's second cell is its
/// state, and the parser reads it from there. A table that does not say
/// 状态 there is not the milestone table, whatever its first column says.
fn is_milestone_header(cells: &[&str]) -> bool {
    let first = cells
        .first()
        .is_some_and(|c| c.eq_ignore_ascii_case("id") || *c == "里程碑");
    let second = cells
        .get(1)
        .is_some_and(|c| c.starts_with("状态") || c.eq_ignore_ascii_case("state"));
    first && second
}

/// `d/N` — the shape both round fields use.
fn parse_fraction(field: &str, raw: &str) -> Result<(u32, u32), ParseError> {
    let bad = || ParseError::BadValue {
        field: field.to_string(),
        value: raw.to_string(),
    };
    let (num, den) = raw.split_once('/').ok_or_else(bad)?;
    let num = num.trim().parse::<u32>().map_err(|_| bad())?;
    let den = den.trim().parse::<u32>().map_err(|_| bad())?;
    Ok((num, den))
}

/// Splits a Markdown table row into trimmed cells, tolerating the optional
/// leading and trailing pipes GitHub-flavoured Markdown allows.
fn table_cells(line: &str) -> Vec<&str> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// True for a table's `|---|---|` separator row.
fn is_separator_row(cells: &[&str]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
}

/// Strips one leading `-` or `*` bullet. Returns `None` for a non-bullet line.
/// The text of the item a continuation line belongs to, if there is one.
///
/// Borrowed mutably so the caller can extend it in place: an item's text is
/// built up as the lines arrive, and copying it out and back would be the
/// same code with a place for the two copies to disagree.
fn last_text_mut<'a>(
    section: Section,
    backlog: &'a mut [BacklogItem],
    disputes: &'a mut [DisputeItem],
    manual_items: &'a mut [ManualItem],
) -> Option<&'a mut String> {
    match section {
        Section::Backlog => backlog.last_mut().map(|i| &mut i.text),
        Section::Disputes => disputes.last_mut().map(|i| &mut i.text),
        Section::ManualAcceptance => manual_items.last_mut().map(|i| &mut i.text),
        _ => None,
    }
}

fn bullet_text(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for marker in ["- ", "* "] {
        if let Some(rest) = t.strip_prefix(marker) {
            return Some(rest.trim());
        }
    }
    None
}

/// Splits `B-01 text` into `("B-01", "text")`. A bullet with no recognisable
/// leading id keeps the whole line as its text and gets a positional id, so a
/// slightly sloppy Backlog entry is still surfaced to the user rather than
/// failing the whole parse — unlike the status block, these sections are
/// advisory, not control flow.
fn split_id(text: &str, fallback_index: usize, prefix: &str) -> (String, String) {
    let mut parts = text.splitn(2, char::is_whitespace);
    let head = parts
        .next()
        .unwrap_or("")
        .trim_end_matches(['.', '：', ':']);
    let looks_like_id = head.len() >= 3
        && head.contains('-')
        && head.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if looks_like_id {
        let rest = parts.next().unwrap_or("").trim();
        (head.to_string(), rest.to_string())
    } else {
        (format!("{prefix}{}", fallback_index + 1), text.to_string())
    }
}

/// Which `##` section a line currently sits in. Only the three that carry
/// machine-read data are distinguished.
#[derive(PartialEq, Clone, Copy)]
enum Section {
    Preamble,
    Milestones,
    Backlog,
    Disputes,
    ManualAcceptance,
    Other,
}

fn classify_heading(heading: &str) -> Section {
    // Tolerates a trailing count or annotation, e.g. "## Backlog（3）".
    let h = heading.trim();
    if h.starts_with("里程碑") {
        Section::Milestones
    } else if h.eq_ignore_ascii_case("backlog") || h.to_lowercase().starts_with("backlog") {
        Section::Backlog
    } else if h.starts_with("争议项") {
        Section::Disputes
    } else if h.starts_with("人工验收清单") {
        Section::ManualAcceptance
    } else {
        Section::Other
    }
}

/// Parses a design document. Fenced code blocks are skipped wholesale, so a
/// document that *shows* a status block as an example (as the task template
/// itself does) cannot be mistaken for one that *has* one.
pub fn parse(doc: &str) -> Result<StatusBlock, ParseError> {
    let mut status: Option<DocStatus> = None;
    let mut design: Option<(u32, u32)> = None;
    let mut implementation: Option<(u32, u32)> = None;
    let mut current_milestone: Option<Option<String>> = None;
    let mut reopens: Option<u32> = None;
    let mut convergence: Option<ConvergenceMode> = None;
    let mut next_action: Option<String> = None;
    let mut repos: Option<Vec<String>> = None;

    let mut milestones: Vec<Milestone> = Vec::new();
    // A document has exactly one milestone table. Once it has been read, a
    // later table that happens to lead with `ID` is some other table — an
    // evidence or comparison table written by a round that had no idea it was
    // sitting in the 里程碑 section — and its rows are not milestone rows.
    let mut seen_milestone_table = false;
    let mut backlog: Vec<BacklogItem> = Vec::new();
    let mut disputes: Vec<DisputeItem> = Vec::new();
    let mut manual_items: Vec<ManualItem> = Vec::new();

    let mut section = Section::Preamble;
    let mut in_fence = false;
    // Blank lines seen since the last bullet, so a list item written with a
    // blank line inside it keeps its shape instead of being cut there.
    let mut blank_run = 0usize;
    // Where we are relative to the milestone table. A `##里程碑` section
    // holds prose as well as the table, and prose contains pipes: an inline
    // `grep "a\|b"`, a shell alternation, a sentence with a vertical bar.
    // Treating every pipe-bearing line as a row rejected a correct design
    // document on a line that was an acceptance bullet.
    let mut table = TableRegion::Outside;

    for (idx, raw_line) in doc.lines().enumerate() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        // A list item's continuation line, in any of the three bullet
        // sections. These items are prose a person reads and rules on, and a
        // design round writes them across several indented lines — the shape
        // Markdown defines for one item. Taking only the first line handed the
        // user half a sentence to rule on: a real dispute read "事实：仓库里
        // 没有" and stopped there, with the fact it was about on the next
        // line.
        //
        // Indentation is what distinguishes a continuation from a new
        // paragraph, so an unindented line still ends the item.
        if !trimmed.is_empty()
            && line.starts_with([' ', '\t'])
            && bullet_text(trimmed).is_none()
            && !trimmed.starts_with('#')
            && let Some(last) =
                last_text_mut(section, &mut backlog, &mut disputes, &mut manual_items)
        {
            if blank_run > 0 {
                last.push(' ');
            }
            last.push(' ');
            last.push_str(trimmed);
            blank_run = 0;
            continue;
        }
        if trimmed.is_empty() {
            blank_run += 1;
        } else {
            blank_run = 0;
        }

        if let Some(rest) = trimmed.strip_prefix("## ") {
            section = classify_heading(rest);
            continue;
        }
        // A deeper heading stays inside its parent section; a `# ` title
        // resets to the preamble.
        if trimmed.starts_with("# ") {
            section = Section::Preamble;
            continue;
        }

        match section {
            Section::Preamble => {
                if let Some((key, value)) = trimmed.split_once(':') {
                    // Only `key: value` where the key is one we know; the
                    // preamble is prose too, and "背景: 这里有冒号" must not
                    // be mistaken for a field.
                    let key = key.trim().trim_start_matches(['-', '*', ' ']).trim();
                    let value = value.trim();
                    let set_once = |slot_is_some: bool| -> Result<(), ParseError> {
                        if slot_is_some {
                            Err(ParseError::DuplicateField {
                                field: key.to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    };
                    match key {
                        "status" => {
                            set_once(status.is_some())?;
                            status = Some(DocStatus::parse(value).ok_or_else(|| {
                                ParseError::BadValue {
                                    field: "status".into(),
                                    value: value.to_string(),
                                }
                            })?);
                        }
                        "design-round" => {
                            set_once(design.is_some())?;
                            design = Some(parse_fraction("design-round", value)?);
                        }
                        "implementation-round" => {
                            set_once(implementation.is_some())?;
                            implementation = Some(parse_fraction("implementation-round", value)?);
                        }
                        "current-milestone" => {
                            set_once(current_milestone.is_some())?;
                            let v = if value.is_empty() || value == "无" || value == "-" {
                                None
                            } else {
                                Some(value.to_string())
                            };
                            current_milestone = Some(v);
                        }
                        "current-milestone-reopens" => {
                            set_once(reopens.is_some())?;
                            reopens =
                                Some(value.parse::<u32>().map_err(|_| ParseError::BadValue {
                                    field: "current-milestone-reopens".into(),
                                    value: value.to_string(),
                                })?);
                        }
                        "convergence-mode" => {
                            set_once(convergence.is_some())?;
                            convergence = Some(ConvergenceMode::parse(value).ok_or_else(|| {
                                ParseError::BadValue {
                                    field: "convergence-mode".into(),
                                    value: value.to_string(),
                                }
                            })?);
                        }
                        "next-action" => {
                            set_once(next_action.is_some())?;
                            next_action = Some(value.to_string());
                        }
                        "repos" => {
                            set_once(repos.is_some())?;
                            // Comma-separated names, the separator the rest of
                            // the block already uses for lists a human writes.
                            // A `无`/`-` is how every other optional field in
                            // this document says "nothing", so it says nothing
                            // here too rather than becoming a repository
                            // called 无.
                            repos = Some(
                                value
                                    .split([',', '，'])
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty() && *v != "无" && *v != "-")
                                    .map(str::to_string)
                                    .collect::<Vec<_>>(),
                            );
                        }
                        _ => {}
                    }
                }
            }
            Section::Milestones => {
                // A row is a row because of where it sits, not because it
                // contains pipes. GitHub-flavoured Markdown defines a table
                // as a header line, a `|---|` separator, then body lines
                // until the first line that is not one. Anchoring on the
                // separator is what keeps prose out: an acceptance bullet
                // containing `grep "import AppKit\|import Speech"` has three
                // pipes and no separator above it, and used to be read as a
                // malformed milestone row.
                if trimmed.matches('|').count() < 2 {
                    table = TableRegion::Outside;
                    continue;
                }
                let cells = table_cells(trimmed);
                if is_separator_row(&cells) {
                    // Only a separator under a header opens a body, and only
                    // the milestone table's header opens *the* body.
                    table = match table {
                        TableRegion::AfterCandidateHeader { milestone: true } => {
                            seen_milestone_table = true;
                            TableRegion::Body
                        }
                        TableRegion::AfterCandidateHeader { milestone: false } => {
                            TableRegion::OtherTable
                        }
                        _ => TableRegion::Outside,
                    };
                    continue;
                }
                if table == TableRegion::OtherTable {
                    continue;
                }
                if table != TableRegion::Body {
                    // The header line, or prose that happens to hold pipes.
                    // Which of the two it was is decided by the next line.
                    table = TableRegion::AfterCandidateHeader {
                        milestone: !seen_milestone_table && is_milestone_header(&cells),
                    };
                    continue;
                }
                if cells.len() < 3 {
                    return Err(ParseError::BadMilestoneRow {
                        line: idx + 1,
                        reason: format!("需要至少 3 列（ID / 状态 / 标题），实际 {}", cells.len()),
                    });
                }
                let id = cells[0].to_string();
                let state =
                    MilestoneState::parse(cells[1]).ok_or_else(|| ParseError::BadMilestoneRow {
                        line: idx + 1,
                        reason: format!("状态 `{}` 不是 开放 / 待审 / 已完成", cells[1]),
                    })?;
                let reopen_count = match cells.get(3).map(|c| c.trim()) {
                    None | Some("") => 0,
                    Some(v) => v.parse::<u32>().map_err(|_| ParseError::BadMilestoneRow {
                        line: idx + 1,
                        reason: format!("reopen 列 `{v}` 不是非负整数"),
                    })?,
                };
                let reopen_domains = cells
                    .get(4)
                    .map(|c| {
                        c.split([',', '，', ';', '；'])
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if milestones.iter().any(|m| m.id == id) {
                    return Err(ParseError::DuplicateMilestoneId { id });
                }
                milestones.push(Milestone {
                    id,
                    state,
                    title: cells[2].to_string(),
                    reopen_count,
                    reopen_domains,
                });
            }
            Section::Backlog => {
                if let Some(text) = bullet_text(trimmed)
                    && !text.is_empty()
                {
                    let (id, body) = split_id(text, backlog.len(), "B-");
                    backlog.push(BacklogItem { id, text: body });
                }
            }
            Section::ManualAcceptance => {
                if let Some(text) = bullet_text(trimmed)
                    && !text.is_empty()
                {
                    // `- [x] H-01 …` / `- [ ] H-01 …` / `- H-01 …` all occur;
                    // the protocol asks for one line per item and does not
                    // mandate a checkbox.
                    let (ticked, rest) = match text.split_at_checked(3) {
                        Some(("[x]", r)) | Some(("[X]", r)) => (true, r.trim_start()),
                        Some(("[ ]", r)) => (false, r.trim_start()),
                        _ => (false, text),
                    };
                    let (id, body) = split_id(rest, manual_items.len(), "H-");
                    manual_items.push(ManualItem {
                        id,
                        text: body,
                        ticked,
                    });
                }
            }
            Section::Disputes => {
                if let Some(text) = bullet_text(trimmed)
                    && !text.is_empty()
                {
                    let (id, body) = split_id(text, disputes.len(), "D-");
                    disputes.push(DisputeItem { id, text: body });
                }
            }
            Section::Other => {}
        }
    }

    let require = |name: &str| ParseError::MissingField {
        field: name.to_string(),
    };
    let status = status.ok_or_else(|| require("status"))?;
    let (design_round, design_round_limit) = design.ok_or_else(|| require("design-round"))?;
    let (impl_round, impl_round_limit) =
        implementation.ok_or_else(|| require("implementation-round"))?;
    let current_milestone = current_milestone.ok_or_else(|| require("current-milestone"))?;
    let current_milestone_reopens = reopens.ok_or_else(|| require("current-milestone-reopens"))?;
    let convergence_mode = convergence.ok_or_else(|| require("convergence-mode"))?;
    let next_action = next_action.ok_or_else(|| require("next-action"))?;

    // An empty milestone table is normal while the design is still being
    // written: the intake round creates the document with the table's header
    // and nothing under it, which is exactly right — there are no milestones
    // yet. What is *not* acceptable is claiming the design is finished with
    // nothing to implement.
    //
    // Tying the rule to `status` rather than to the heading's presence is the
    // difference between a parser that rejects a correct intake document and
    // one that catches a design round which forgot to decompose the work.
    if milestones.is_empty() && matches!(status, DocStatus::Implementing | DocStatus::Done) {
        return Err(ParseError::NoMilestonesAfterDesign);
    }
    if let Some(id) = &current_milestone
        && !milestones.is_empty()
        && !milestones.iter().any(|m| &m.id == id)
    {
        return Err(ParseError::UnknownCurrentMilestone { id: id.clone() });
    }

    Ok(StatusBlock {
        status,
        design_round,
        design_round_limit,
        impl_round,
        impl_round_limit,
        current_milestone,
        current_milestone_reopens,
        convergence_mode,
        next_action,
        repos: repos.unwrap_or_default(),
        milestones,
        backlog,
        disputes,
        manual_items,
    })
}

#[cfg(test)]
mod tests {

    /// A block with every required field and nothing optional — the shape of
    /// every task document written before workspaces existed.
    const WITHOUT_REPOS: &str = "\
<!-- autome:status
status: 设计中
design-round: 1/15
implementation-round: 0/25
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交评审
-->
";

    #[test]
    fn a_document_without_repos_still_parses() {
        // The load-bearing backwards-compatibility assertion. `repos:` is
        // optional, and the parser fails a document that is missing a
        // *required* field — so the day this becomes required is the day every
        // task already in flight, every fixture and every eval case turns into
        // a protocol failure at once.
        let parsed = parse(WITHOUT_REPOS).expect("a document from before workspaces existed");
        assert!(parsed.repos.is_empty(), "absent means 'nothing was said'");
    }

    #[test]
    fn repos_is_a_comma_separated_list_of_names() {
        let doc = WITHOUT_REPOS.replace(
            "next-action: 交评审",
            "next-action: 交评审\nrepos: offchat-backend-service, offchat-miniprogram",
        );
        assert_eq!(
            parse(&doc).unwrap().repos,
            vec!["offchat-backend-service", "offchat-miniprogram"]
        );
    }

    #[test]
    fn repos_tolerates_the_punctuation_a_person_actually_types() {
        // A round writing Chinese prose reaches for the full-width comma, and
        // the same round writes 无 for "none" in every other field of this
        // block. Both must mean what they look like they mean.
        let with_fullwidth = WITHOUT_REPOS.replace(
            "next-action: 交评审",
            "next-action: 交评审\nrepos: docs，backend ,  frontend",
        );
        assert_eq!(
            parse(&with_fullwidth).unwrap().repos,
            vec!["docs", "backend", "frontend"]
        );

        for none in ["无", "-", ""] {
            let doc = WITHOUT_REPOS.replace(
                "next-action: 交评审",
                &format!("next-action: 交评审\nrepos: {none}"),
            );
            assert!(
                parse(&doc).unwrap().repos.is_empty(),
                "`repos: {none}` is not a repository called {none}"
            );
        }
    }

    #[test]
    fn a_bullet_written_across_several_lines_keeps_all_of_it() {
        // From a real adjudication round. The user rules on these, and the
        // panel showed "事实：仓库里没有" — the fact itself was on the next
        // line, and the button next to it said 保存裁定.
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/15
implementation-round: 0/25
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
-->

## 争议项

- C-D3 R-7 的效果层在本任务内不可交付。事实：仓库里没有
  群内 AI 回复执行器，`qwen` 的既有用途只有帖子撰写辅助。

  若用户期望的是接通，这需要一个单独的任务。
- C-D4 第二条，独立的一条。

## Backlog

- B-01 一条建议，
  它的下半句在这里。
";
        let parsed = parse(doc).unwrap();
        assert_eq!(parsed.disputes.len(), 2, "两条，不是一条也不是三条");
        assert!(
            parsed.disputes[0].text.contains("群内 AI 回复执行器"),
            "续行丢了：{}",
            parsed.disputes[0].text
        );
        assert!(
            parsed.disputes[0].text.contains("单独的任务"),
            "空行之后的续行也要留住：{}",
            parsed.disputes[0].text
        );
        assert_eq!(parsed.disputes[1].id, "C-D4", "下一条 bullet 另起一条");
        assert!(
            !parsed.disputes[1].text.contains("单独的任务"),
            "两条不能黏在一起：{}",
            parsed.disputes[1].text
        );
        assert!(parsed.backlog[0].text.contains("下半句"), "Backlog 同理");
    }

    #[test]
    fn an_unindented_line_after_a_bullet_ends_it() {
        // Indentation is what Markdown uses to say "still the same item". An
        // ordinary paragraph after the list is not part of the last item.
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/15
implementation-round: 0/25
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
-->

## 争议项

- C-D1 一条争议。
这是后面的一段散文，不属于它。
";
        let parsed = parse(doc).unwrap();
        assert_eq!(parsed.disputes.len(), 1);
        assert!(
            !parsed.disputes[0].text.contains("散文"),
            "{}",
            parsed.disputes[0].text
        );
    }

    #[test]
    fn a_dependency_table_is_not_the_milestone_table() {
        // From a design round on 2026-09-22. `## 里程碑依赖` is a legitimate
        // section to write, and its table's first column is `ID` — which used
        // to be the whole test. The first row's `—` under 前置 was read as a
        // milestone state and the task was failed for a protocol error it had
        // not committed.
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/15
implementation-round: 0/25
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
-->

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 数据形状与迁移 | 0 | |
| M-02 | 开放 | 仓储与应用服务 | 0 | |

## 里程碑依赖

| ID | 前置 | 说明 |
|---|---|---|
| M-01 | — | 先立数据形状与迁移 |
| M-02 | M-01 | 仓储与应用服务 |
";
        let parsed = parse(doc).expect("the dependency table is not the milestone table");
        assert_eq!(parsed.milestones.len(), 2);
        assert_eq!(parsed.milestones[0].state, MilestoneState::Open);
    }

    #[test]
    fn a_table_whose_second_column_is_not_the_state_is_skipped_wherever_it_sits() {
        // The same rule, stated on its own: the milestone table is the one
        // that carries states, and the parser reads them from the second
        // column. Anything else in the section is prose with pipes in it.
        assert!(is_milestone_header(&["ID", "状态", "标题"]));
        assert!(is_milestone_header(&["里程碑", "状态"]));
        assert!(!is_milestone_header(&["ID", "前置", "说明"]));
        assert!(!is_milestone_header(&["ID", "结果", "证据"]));
        assert!(!is_milestone_header(&["ID"]));
    }

    /// From a design document a real session wrote on 2026-09-16. It was
    /// rejected with "里程碑表第 286 行格式错误：状态 `import AVFoundation\`
    /// 不是 开放 / 待审 / 已完成" — the line was an acceptance bullet, not a
    /// row. Prose in the milestone section is normal and must stay readable.
    #[test]
    fn prose_with_pipes_in_the_milestone_section_is_not_a_row() {
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/3
implementation-round: 0/6
current-milestone:
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交评审
-->

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 工程骨架 | 0 | |
| M-02 | 开放 | 权限与识别 | 0 | |

### 里程碑明细

**M-01 工程骨架** — 前置：无

验收：
- `grep -rn \"import AppKit\\|import AVFoundation\\|import Speech\" Sources/` 无输出。
- 管道也可能出现在散文里：a | b | c。
";
        let parsed = parse(doc).expect("the bullet is prose, not a malformed row");
        assert_eq!(parsed.milestones.len(), 2);
        assert_eq!(parsed.milestones[0].id, "M-01");
        assert_eq!(parsed.milestones[1].id, "M-02");
    }

    #[test]
    fn another_table_in_the_milestone_section_is_not_the_milestone_table() {
        // From the implementation round of 2026-09-16. It wrote an
        // acceptance-evidence table under `#### M-01 验收证据`, inside the
        // 里程碑 section. Anchoring only on the separator row read its `通过`
        // as a milestone state and failed a task whose evidence was complete.
        let doc = "\
<!-- autome:status
status: 实现中
design-round: 1/15
implementation-round: 3/14
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交审计
-->

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 待审 | 工程骨架 | 0 | |
| M-02 | 开放 | 权限与识别 | 0 | |

#### M-01 验收证据

| 验收项 | 结果 | 证据 |
|---|---|---|
| `bash scripts/verify.sh` 退出码 0 | 通过 | 末行 `VERIFY OK` |
| `bash scripts/test.sh` 退出码 0 | 通过 | `passed=11 failed=0` |
| 人工：Dock 无图标 | 通过 | `LSUIElement` 为 true |
";
        let parsed = parse(doc).expect("the evidence table is not the milestone table");
        assert_eq!(parsed.milestones.len(), 2);
        assert_eq!(parsed.milestones[0].state, MilestoneState::Pending);
        assert_eq!(parsed.milestones[1].id, "M-02");
    }

    #[test]
    fn the_milestone_table_is_still_read_when_it_comes_after_another_table() {
        let doc = "\
<!-- autome:status
status: 实现中
design-round: 1/15
implementation-round: 1/14
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 继续
-->

## 里程碑

| 验收项 | 结果 |
|---|---|
| 某项 | 通过 |

| ID | 状态 | 标题 |
|---|---|---|
| M-01 | 开放 | 骨架 |
";
        let parsed = parse(doc).unwrap();
        assert_eq!(parsed.milestones.len(), 1);
        assert_eq!(parsed.milestones[0].id, "M-01");
    }

    #[test]
    fn a_comparison_table_with_a_blank_header_is_not_the_milestone_table() {
        // The failure of 2026-09-16, reduced. Implementation round #16 wrote
        // this comparison table inside the 里程碑 section, far below the
        // milestone table. Its blank first header cell used to open a
        // milestone table body, and the row under it — whose second cell is a
        // code span, not a state — failed the whole document and stopped the
        // task with "里程碑表第 2124 行格式错误".
        let doc = "\
<!-- autome:status
status: 实现中
design-round: 1/15
implementation-round: 16/35
current-milestone: M-02
current-milestone-reopens: 1
convergence-mode: domain-review
next-action: 独立复验 A12-P01 的修复
-->

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 工程骨架 | 0 | |
| M-02 | 待审 | 端到端链路 | 1 | reminder-delivery |

#### 判据不同，所以新增的是一个独立的 Core 函数

| | 启动对齐 | 运行中的通道交接 |
|---|---|---|
| 入口 | `loadSchedules()` → `alignReminders(handingOver: false)` | `refreshMode()` 的通道变化分支 |
| 已经到点的那些 | **不补发** | **当场补送一次** |
| 判据 | 只看 `plan` | 看 `plan` **且** 看旧队列 |
";
        let parsed =
            parse(doc).expect("a blank-headed comparison table is not the milestone table");
        assert_eq!(parsed.milestones.len(), 2);
        assert_eq!(parsed.milestones[1].id, "M-02");
        assert_eq!(parsed.milestones[1].state, MilestoneState::Pending);
        assert_eq!(
            parsed.milestones[1].reopen_domains,
            vec!["reminder-delivery"]
        );
    }

    #[test]
    fn only_the_first_milestone_shaped_table_is_read() {
        // A document has one milestone table. A later table that also leads
        // with `ID` is something else — here a per-milestone evidence index —
        // and reading its rows as milestones used to fail the document on a
        // duplicate id.
        let doc = "\
<!-- autome:status
status: 实现中
design-round: 1/15
implementation-round: 2/10
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交审计
-->

## 里程碑

| ID | 状态 | 标题 |
|---|---|---|
| M-01 | 待审 | 工程骨架 |

#### 证据索引

| ID | 证据文件 |
|---|---|
| M-01 | .autome/output/m01-evidence.txt |
";
        let parsed = parse(doc).expect("the second ID-led table is not the milestone table");
        assert_eq!(parsed.milestones.len(), 1);
        assert_eq!(parsed.milestones[0].state, MilestoneState::Pending);
    }

    #[test]
    fn a_milestone_table_whose_header_is_blank_is_reported_as_a_missing_table() {
        // The cost of the rule above: a milestone table that omits its `ID`
        // header is no longer recognised. The document still fails — it must,
        // the milestones are unreadable — but it fails by naming the missing
        // table rather than by blaming a row in it.
        let doc = "\
<!-- autome:status
status: 实现中
design-round: 1/15
implementation-round: 1/10
current-milestone:
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交审计
-->

## 里程碑

| | 状态 | 标题 |
|---|---|---|
| M-01 | 待审 | 工程骨架 |
";
        assert_eq!(
            parse(doc).unwrap_err(),
            ParseError::NoMilestonesAfterDesign,
            "a blank header should read as 'no milestone table', not as a bad row"
        );
    }

    #[test]
    fn a_malformed_row_inside_the_table_is_still_an_error() {
        // The fix must not become "skip anything that does not parse": a typo
        // in a real row has to be reported, or a session could silently lose
        // a milestone.
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/3
implementation-round: 0/6
current-milestone:
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交评审
-->

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 工程骨架 | 0 | |
| M-02 | 进行中 | 权限与识别 | 0 | |
";
        let err = parse(doc).unwrap_err();
        assert!(
            matches!(err, ParseError::BadMilestoneRow { .. }),
            "expected a bad-row error, got {err}"
        );
    }

    #[test]
    fn a_table_without_a_separator_row_yields_no_milestones() {
        // GFM requires the separator. Without it there is no table, and the
        // scanner must not guess — guessing is what let prose in.
        let doc = "\
<!-- autome:status
status: 设计中
design-round: 1/3
implementation-round: 0/6
current-milestone:
current-milestone-reopens: 0
convergence-mode: normal
next-action: 交评审
-->

## 里程碑

| M-01 | 开放 | 工程骨架 | 0 | |
";
        assert!(parse(doc).unwrap().milestones.is_empty());
    }
    use super::*;

    const FULL: &str = r#"# 购物车结算流程

status: 实现中
design-round: 3/15
implementation-round: 4/25
current-milestone: M-03
current-milestone-reopens: 1
convergence-mode: normal
next-action: 补 promo.spec.ts 大小写用例，跑 vitest src/checkout/promo

## 目标

在现有结算链路上加入优惠码。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 购物车数据模型 | 0 | |
| M-02 | 已完成 | 结算接口 | 0 | |
| M-03 | 待审 | 优惠码校验 | 1 | promo-case |
| M-04 | 开放 | 库存不足阻止下单 | 0 | |
| M-05 | 开放 | 确认邮件 | 0 | |

## Backlog

- B-01 优惠码使用次数上限
- B-02 结算接口日志脱敏

## 争议项

- D3-P02 优惠码是否大小写敏感
"#;

    fn minimal(extra: &str) -> String {
        format!(
            "status: 设计中\ndesign-round: 1/15\nimplementation-round: 0/0\ncurrent-milestone: 无\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n{extra}"
        )
    }

    #[test]
    fn parses_a_complete_document() {
        let b = parse(FULL).unwrap();
        assert_eq!(b.status, DocStatus::Implementing);
        assert_eq!((b.design_round, b.design_round_limit), (3, 15));
        assert_eq!((b.impl_round, b.impl_round_limit), (4, 25));
        assert_eq!(b.current_milestone.as_deref(), Some("M-03"));
        assert_eq!(b.current_milestone_reopens, 1);
        assert_eq!(b.convergence_mode, ConvergenceMode::Normal);
        assert!(b.next_action.starts_with("补 promo.spec.ts"));
        assert_eq!(b.milestones.len(), 5);
        assert_eq!(b.backlog.len(), 2);
        assert_eq!(b.disputes.len(), 1);
    }

    #[test]
    fn reads_milestone_rows_including_reopen_and_domains() {
        let b = parse(FULL).unwrap();
        let m3 = &b.milestones[2];
        assert_eq!(m3.id, "M-03");
        assert_eq!(m3.state, MilestoneState::Pending);
        assert_eq!(m3.title, "优惠码校验");
        assert_eq!(m3.reopen_count, 1);
        assert_eq!(m3.reopen_domains, vec!["promo-case".to_string()]);
        assert!(b.milestones[0].reopen_domains.is_empty());
    }

    #[test]
    fn counts_milestone_states() {
        let b = parse(FULL).unwrap();
        assert_eq!(b.milestones_done(), 2);
        assert!(b.has_open_milestone());
        assert!(!b.all_milestones_done());
        assert!(!b.no_open_milestones());
    }

    #[test]
    fn all_done_is_false_for_an_empty_milestone_list() {
        let b = parse(&minimal("")).unwrap();
        assert!(!b.all_milestones_done());
        assert!(!b.no_open_milestones());
    }

    #[test]
    fn no_open_milestones_is_true_when_everything_is_pending() {
        let doc = minimal(
            "\n## 里程碑\n\n| ID | 状态 | 标题 |\n|---|---|---|\n| M-01 | 待审 | a |\n| M-02 | 已完成 | b |\n",
        );
        let b = parse(&doc).unwrap();
        assert!(b.no_open_milestones());
        assert!(!b.all_milestones_done());
    }

    #[test]
    fn splits_backlog_and_dispute_ids_from_their_text() {
        let b = parse(FULL).unwrap();
        assert_eq!(b.backlog[0].id, "B-01");
        assert_eq!(b.backlog[0].text, "优惠码使用次数上限");
        assert_eq!(b.disputes[0].id, "D3-P02");
        assert_eq!(b.disputes[0].text, "优惠码是否大小写敏感");
    }

    #[test]
    fn a_backlog_bullet_without_an_id_still_parses_with_a_positional_one() {
        let doc = minimal("\n## Backlog\n\n- 没有编号的一条建议\n- B-07 有编号的\n");
        let b = parse(&doc).unwrap();
        assert_eq!(b.backlog[0].id, "B-1");
        assert_eq!(b.backlog[0].text, "没有编号的一条建议");
        assert_eq!(b.backlog[1].id, "B-07");
    }

    #[test]
    fn empty_backlog_and_dispute_sections_are_fine() {
        let doc = minimal("\n## Backlog\n\n## 争议项\n");
        let b = parse(&doc).unwrap();
        assert!(b.backlog.is_empty());
        assert!(b.disputes.is_empty());
    }

    #[test]
    fn current_milestone_may_be_absent() {
        for marker in ["无", "-", ""] {
            let doc = format!(
                "status: 设计中\ndesign-round: 1/15\nimplementation-round: 0/0\ncurrent-milestone: {marker}\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n"
            );
            assert_eq!(
                parse(&doc).unwrap().current_milestone,
                None,
                "marker {marker:?}"
            );
        }
    }

    #[test]
    fn every_missing_field_is_named() {
        let fields = [
            "status",
            "design-round",
            "implementation-round",
            "current-milestone",
            "current-milestone-reopens",
            "convergence-mode",
            "next-action",
        ];
        for missing in fields {
            let doc: String = FULL
                .lines()
                .filter(|l| !l.starts_with(&format!("{missing}:")))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                parse(&doc).unwrap_err(),
                ParseError::MissingField {
                    field: missing.to_string()
                },
                "removing {missing} should be reported"
            );
        }
    }

    #[test]
    fn an_unparseable_status_names_the_field_and_the_value() {
        let doc = FULL.replace("status: 实现中", "status: 差不多完事了");
        assert_eq!(
            parse(&doc).unwrap_err(),
            ParseError::BadValue {
                field: "status".into(),
                value: "差不多完事了".into()
            }
        );
    }

    #[test]
    fn a_round_field_without_a_slash_is_rejected() {
        let doc = FULL.replace("design-round: 3/15", "design-round: 3");
        assert_eq!(
            parse(&doc).unwrap_err(),
            ParseError::BadValue {
                field: "design-round".into(),
                value: "3".into()
            }
        );
    }

    #[test]
    fn a_non_numeric_round_is_rejected() {
        let doc = FULL.replace("implementation-round: 4/25", "implementation-round: 四/25");
        assert!(matches!(
            parse(&doc).unwrap_err(),
            ParseError::BadValue { field, .. } if field == "implementation-round"
        ));
    }

    #[test]
    fn a_duplicated_field_is_rejected_rather_than_last_write_wins() {
        let doc = FULL.replace(
            "convergence-mode: normal",
            "convergence-mode: normal\nconvergence-mode: milestone-review",
        );
        assert_eq!(
            parse(&doc).unwrap_err(),
            ParseError::DuplicateField {
                field: "convergence-mode".into()
            }
        );
    }

    #[test]
    fn a_bad_milestone_state_names_the_row_and_the_value() {
        let doc = FULL.replace("| M-03 | 待审 |", "| M-03 | 差不多 |");
        match parse(&doc).unwrap_err() {
            ParseError::BadMilestoneRow { reason, .. } => assert!(reason.contains("差不多")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_milestone_row_with_too_few_columns_is_rejected() {
        let doc = minimal("\n## 里程碑\n\n| ID | 状态 | 标题 |\n|---|---|---|\n| M-01 | 开放 |\n");
        assert!(matches!(
            parse(&doc).unwrap_err(),
            ParseError::BadMilestoneRow { .. }
        ));
    }

    #[test]
    fn a_non_numeric_reopen_count_is_rejected() {
        let doc = FULL.replace(
            "| M-03 | 待审 | 优惠码校验 | 1 |",
            "| M-03 | 待审 | 优惠码校验 | 一 |",
        );
        assert!(matches!(
            parse(&doc).unwrap_err(),
            ParseError::BadMilestoneRow { .. }
        ));
    }

    #[test]
    fn a_duplicate_milestone_id_is_rejected() {
        let doc = FULL.replace(
            "| M-04 | 开放 | 库存不足阻止下单 | 0 | |",
            "| M-03 | 开放 | 又一个 | 0 | |",
        );
        assert_eq!(
            parse(&doc).unwrap_err(),
            ParseError::DuplicateMilestoneId { id: "M-03".into() }
        );
    }

    #[test]
    fn a_current_milestone_missing_from_the_table_is_rejected() {
        let doc = FULL.replace("current-milestone: M-03", "current-milestone: M-99");
        assert_eq!(
            parse(&doc).unwrap_err(),
            ParseError::UnknownCurrentMilestone { id: "M-99".into() }
        );
    }

    #[test]
    fn an_empty_milestone_table_is_fine_while_the_design_is_still_being_written() {
        // This is what a real intake round produces: the table's header with
        // nothing under it. Rejecting it would fail every task at its first
        // node.
        let doc =
            minimal("\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n");
        let b = parse(&doc).unwrap();
        assert!(b.milestones.is_empty());
    }

    #[test]
    fn a_milestone_heading_with_only_prose_is_also_fine_while_designing() {
        let doc = minimal("\n## 里程碑\n\n还没有拆分。\n");
        assert!(parse(&doc).unwrap().milestones.is_empty());
    }

    #[test]
    fn no_milestone_section_at_all_is_allowed_before_the_first_design_round() {
        let b = parse(&minimal("")).unwrap();
        assert!(b.milestones.is_empty());
    }

    #[test]
    fn claiming_the_design_is_finished_with_no_milestones_is_rejected() {
        // The implementation loop would have nothing to advance, and the
        // budget would be computed from an empty list.
        for status in ["实现中", "已完成"] {
            let doc = format!(
                "status: {status}\ndesign-round: 3/15\nimplementation-round: 0/0\n\
                 current-milestone: 无\ncurrent-milestone-reopens: 0\n\
                 convergence-mode: normal\nnext-action: 无\n"
            );
            assert_eq!(
                parse(&doc).unwrap_err(),
                ParseError::NoMilestonesAfterDesign,
                "{status} with no milestones should be rejected"
            );
        }
    }

    #[test]
    fn an_infeasible_document_needs_no_milestones() {
        // A task that cannot be done never got as far as decomposing it.
        let doc = "status: 不可实现\ndesign-round: 2/15\nimplementation-round: 0/0\ncurrent-milestone: 无\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n";
        assert!(parse(doc).is_ok());
    }

    #[test]
    fn a_status_block_inside_a_fenced_example_is_not_read_as_the_real_one() {
        let doc = format!(
            "# 任务文件\n\n下面是格式示例：\n\n```text\nstatus: 已完成\ndesign-round: 9/15\n```\n\n{}",
            minimal("")
        );
        let b = parse(&doc).unwrap();
        assert_eq!(b.status, DocStatus::Designing);
        assert_eq!(b.design_round, 1);
    }

    #[test]
    fn a_fenced_milestone_table_is_ignored() {
        let doc = minimal(
            "\n## 里程碑\n\n```\n| M-99 | 开放 | 示例 |\n```\n\n| ID | 状态 | 标题 |\n|---|---|---|\n| M-01 | 开放 | 真的 |\n",
        );
        let b = parse(&doc).unwrap();
        assert_eq!(b.milestones.len(), 1);
        assert_eq!(b.milestones[0].id, "M-01");
    }

    #[test]
    fn prose_containing_a_colon_is_not_mistaken_for_a_field() {
        let doc = format!(
            "{}\n背景: 这里有一个冒号\n注意: status 也在这句话里\n",
            minimal("")
        );
        let b = parse(&doc).unwrap();
        assert_eq!(b.status, DocStatus::Designing);
    }

    #[test]
    fn tables_without_outer_pipes_still_parse() {
        let doc =
            minimal("\n## 里程碑\n\nID | 状态 | 标题\n--- | --- | ---\nM-01 | 开放 | 无边框\n");
        let b = parse(&doc).unwrap();
        assert_eq!(b.milestones.len(), 1);
        assert_eq!(b.milestones[0].title, "无边框");
    }

    #[test]
    fn a_heading_with_an_annotation_is_still_recognised() {
        let doc = minimal("\n## Backlog（2 条）\n\n- B-01 甲\n- B-02 乙\n");
        assert_eq!(parse(&doc).unwrap().backlog.len(), 2);
    }

    #[test]
    fn a_subsection_stays_inside_its_parent_section() {
        let doc = minimal("\n## Backlog\n\n### 来自评审\n\n- B-01 甲\n");
        assert_eq!(parse(&doc).unwrap().backlog.len(), 1);
    }

    #[test]
    fn bullets_outside_backlog_and_disputes_are_ignored() {
        let doc = minimal("\n## 方案\n\n- 这不是 backlog\n- 也不是争议项\n");
        let b = parse(&doc).unwrap();
        assert!(b.backlog.is_empty());
        assert!(b.disputes.is_empty());
    }

    #[test]
    fn every_doc_status_and_convergence_mode_round_trips() {
        for (name, expected) in DocStatus::NAMES {
            assert_eq!(DocStatus::parse(name), Some(expected));
            assert_eq!(expected.as_str(), name);
        }
        for mode in [
            ConvergenceMode::Normal,
            ConvergenceMode::DomainReview,
            ConvergenceMode::MilestoneReview,
        ] {
            assert_eq!(ConvergenceMode::parse(mode.as_str()), Some(mode));
        }
        for state in [
            MilestoneState::Open,
            MilestoneState::Pending,
            MilestoneState::Done,
        ] {
            assert_eq!(MilestoneState::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn status_block_round_trips_through_json() {
        let b = parse(FULL).unwrap();
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<StatusBlock>(&json).unwrap(), b);
    }

    #[test]
    fn crlf_line_endings_parse() {
        let doc = FULL.replace('\n', "\r\n");
        assert_eq!(parse(&doc).unwrap().status, DocStatus::Implementing);
    }

    #[test]
    fn an_empty_document_reports_the_first_missing_field() {
        assert_eq!(
            parse("").unwrap_err(),
            ParseError::MissingField {
                field: "status".into()
            }
        );
    }
}
