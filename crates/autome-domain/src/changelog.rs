//! The protocol repository's `CHANGELOG.md`: one section per version, one
//! block per change (plan §3.4).
//!
//! This file is what makes a protocol change falsifiable. Every entry names
//! the clause it touches, the task evidence that motivated it, and a
//! prediction about a metric the core actually records. `realized_impact` is
//! filled in by the core once `horizon` tasks have run — by the machine, not by
//! whoever wrote the entry.
//!
//! The three kinds are not decoration; they carry different gates:
//!
//! | kind | needs an eval case | needs evidence | how it is judged |
//! |---|---|---|---|
//! | `behavioral` | yes — red before, green after | ≥ 2 tasks | the eval, then the metric |
//! | `clarify` | no | ≥ 2 tasks | the review round signs off |
//! | `retire` | names the eval it removes | metric evidence | a removal experiment: `flat` |
//!
//! A `retire` predicting an improvement would be unfalsifiable in the
//! direction that matters — the claim is "removing this costs nothing", so the
//! prediction is `flat` and the experiment fails if the metric gets worse.

use serde::{Deserialize, Serialize};

use crate::metrics::{Direction, PredictedImpact, RealizedImpact};
use crate::yaml_lite::{self, Error, err};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Changes what a session does. Must come with an eval case.
    Behavioral,
    /// Says the same thing more clearly. No behaviour change is claimed, so no
    /// case can be written that fails before and passes after.
    Clarify,
    /// Removes a clause. Only ever as a removal experiment (plan §E3, §6.6).
    Retire,
}

impl ChangeKind {
    pub const ALL: [ChangeKind; 3] = [
        ChangeKind::Behavioral,
        ChangeKind::Clarify,
        ChangeKind::Retire,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Behavioral => "behavioral",
            ChangeKind::Clarify => "clarify",
            ChangeKind::Retire => "retire",
        }
    }

    pub fn parse(s: &str) -> Option<ChangeKind> {
        ChangeKind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn vocabulary() -> String {
        ChangeKind::ALL
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

pub const FIELDS: [&str; 7] = [
    "id",
    "kind",
    "clause",
    "evidence",
    "predicted_impact",
    "eval",
    "realized_impact",
];

/// How many distinct tasks must have hit a problem before the protocol changes
/// for it. One task is a bad week.
pub const MIN_EVIDENCE_TASKS: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub id: String,
    pub kind: ChangeKind,
    /// `loop-protocol.md#实现循环/自审清单` — file, then heading path.
    pub clause: String,
    /// `voice-schedule L-01` style references into tasks' `lessons.md`.
    pub evidence: Vec<String>,
    pub predicted_impact: PredictedImpact,
    /// Directory under the protocol repository, e.g.
    /// `evals/self-check-not-applicable/`. Required for `behavioral`; for
    /// `retire` it names the case being removed.
    pub eval: Option<String>,
    /// Backfilled by the core; `null` until then.
    pub realized_impact: Option<RealizedImpact>,
}

impl ChangelogEntry {
    /// The file half of `clause`, for checking that the clause exists.
    pub fn clause_file(&self) -> &str {
        self.clause.split_once('#').map(|(f, _)| f).unwrap_or(&self.clause)
    }

    /// The distinct task names in `evidence`. An entry citing the same task
    /// twice has one task's worth of evidence, not two.
    pub fn evidence_tasks(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for e in &self.evidence {
            let task = e.split_whitespace().next().unwrap_or(e.as_str());
            if !out.contains(&task) {
                out.push(task);
            }
        }
        out
    }

    /// Whether the prediction can be checked once the horizon passes.
    pub fn is_measurable(&self) -> bool {
        self.predicted_impact.is_measurable()
    }

    /// Whether the realized outcome matched. `None` until backfilled.
    pub fn held_up(&self) -> Option<bool> {
        Some(
            self.realized_impact
                .as_ref()?
                .matches(self.predicted_impact.direction),
        )
    }
}

/// Why an entry cannot pass the gate (plan §6.5 layer 1, §6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryProblem {
    /// `behavioral` without a case, or `retire` without one to remove.
    MissingEval { id: String },
    /// The named eval directory is not in the version.
    EvalNotFound { id: String, path: String },
    /// Fewer than `MIN_EVIDENCE_TASKS` distinct tasks.
    ThinEvidence { id: String, tasks: usize },
    /// The metric is not one the core records, or the horizon is zero.
    Unmeasurable { id: String, metric: String },
    /// A `retire` that predicts an improvement rather than no harm.
    RetireMustBeFlat { id: String },
    /// The clause names a file this version does not have.
    ClauseFileMissing { id: String, file: String },
}

impl std::fmt::Display for EntryProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryProblem::MissingEval { id } => {
                write!(f, "{id}：behavioral / retire 改动必须指明 eval 用例")
            }
            EntryProblem::EvalNotFound { id, path } => {
                write!(f, "{id}：eval 目录 `{path}` 不存在")
            }
            EntryProblem::ThinEvidence { id, tasks } => {
                write!(
                    f,
                    "{id}：只引用了 {tasks} 个任务的证据，至少要 {MIN_EVIDENCE_TASKS} 个"
                )
            }
            EntryProblem::Unmeasurable { id, metric } => {
                write!(f, "{id}：metric `{metric}` 无法测量")
            }
            EntryProblem::RetireMustBeFlat { id } => {
                write!(f, "{id}：retire 只能以移除实验的形式提出，direction 必须是 flat")
            }
            EntryProblem::ClauseFileMissing { id, file } => {
                write!(f, "{id}：clause 指向的文件 `{file}` 不在本版本里")
            }
        }
    }
}

/// Checks one entry against the gates its `kind` implies.
///
/// `has_path` answers "does this version contain this path"; the caller wires
/// it to the `ProtocolFiles` being gated. Passing it in keeps this function
/// free of any notion of where a version lives.
pub fn check_entry(entry: &ChangelogEntry, has_path: &dyn Fn(&str) -> bool) -> Vec<EntryProblem> {
    let mut out = Vec::new();
    let id = entry.id.clone();

    if !entry.is_measurable() {
        out.push(EntryProblem::Unmeasurable {
            id: id.clone(),
            metric: entry.predicted_impact.metric.clone(),
        });
    }

    let tasks = entry.evidence_tasks().len();
    if tasks < MIN_EVIDENCE_TASKS {
        out.push(EntryProblem::ThinEvidence {
            id: id.clone(),
            tasks,
        });
    }

    if matches!(entry.kind, ChangeKind::Behavioral | ChangeKind::Retire) {
        match entry.eval.as_deref().filter(|p| !p.trim().is_empty()) {
            None => out.push(EntryProblem::MissingEval { id: id.clone() }),
            Some(path) => {
                // `retire` names the case it removes, so that case is *not*
                // expected to still be there. Only `behavioral` must resolve.
                if entry.kind == ChangeKind::Behavioral && !has_path(path) {
                    out.push(EntryProblem::EvalNotFound {
                        id: id.clone(),
                        path: path.to_string(),
                    });
                }
            }
        }
    }

    if entry.kind == ChangeKind::Retire && entry.predicted_impact.direction != Direction::Flat {
        out.push(EntryProblem::RetireMustBeFlat { id: id.clone() });
    }

    let file = entry.clause_file();
    if !file.is_empty() && !has_path(file) {
        out.push(EntryProblem::ClauseFileMissing {
            id,
            file: file.to_string(),
        });
    }

    out
}

/// One `## protocol/vN` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangelogVersion {
    pub tag: String,
    pub entries: Vec<ChangelogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Changelog {
    /// Newest first, as written.
    pub versions: Vec<ChangelogVersion>,
}

impl Changelog {
    pub fn entries(&self) -> impl Iterator<Item = &ChangelogEntry> {
        self.versions.iter().flat_map(|v| v.entries.iter())
    }

    pub fn version(&self, tag: &str) -> Option<&ChangelogVersion> {
        self.versions.iter().find(|v| v.tag == tag)
    }

    /// Every clause path any entry claims to govern. Layer 1 of the eval gate
    /// uses this to check that no imperative line in the protocol is
    /// unaccounted for.
    pub fn clauses(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for e in self.entries() {
            if !out.contains(&e.clause.as_str()) {
                out.push(&e.clause);
            }
        }
        out
    }
}

/// Parses a `CHANGELOG.md`.
///
/// Sections are `## <tag>` headings; blocks before the first heading belong to
/// an unnamed version, which is what an in-progress meta task is writing into.
pub fn parse(doc: &str) -> Result<Changelog, Error> {
    let mut versions: Vec<ChangelogVersion> = Vec::new();
    let mut tag = String::from("未发布");

    // Split on `## ` headings, keeping the line offsets so errors point at the
    // real line in the file.
    let mut section_start = 0usize;
    let mut sections: Vec<(String, usize, String)> = Vec::new();
    let lines: Vec<&str> = doc.lines().collect();
    let mut current = String::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("## ") {
            sections.push((tag.clone(), section_start, std::mem::take(&mut current)));
            tag = rest.trim().to_string();
            section_start = i + 1;
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    sections.push((tag, section_start, current));

    for (tag, offset, body) in sections {
        let mut entries = Vec::new();
        for block in yaml_lite::blocks(&body) {
            if block.first_key() != "id" {
                continue;
            }
            entries.push(parse_block(&block, offset)?);
        }
        if entries.is_empty() && versions.iter().any(|v| v.tag == tag) {
            continue;
        }
        if !entries.is_empty() || tag != "未发布" {
            versions.push(ChangelogVersion { tag, entries });
        }
    }
    Ok(Changelog { versions })
}

fn parse_block(block: &yaml_lite::Block, offset: usize) -> Result<ChangelogEntry, Error> {
    let at = |line: usize| line + offset;
    block
        .reject_unknown(&FIELDS)
        .map_err(|e| err(at(e.line), e.detail))?;
    if let Some(dup) = block.duplicate() {
        return Err(err(
            at(dup.line),
            format!("字段 `{}` 出现了两次", dup.key),
        ));
    }

    let require = |key: &str| -> Result<&yaml_lite::Field, Error> {
        block
            .get(key)
            .ok_or_else(|| err(at(block.line), format!("缺少字段 `{key}`")))
    };

    let kind_f = require("kind")?;
    let kind = ChangeKind::parse(&kind_f.value).ok_or_else(|| {
        err(
            at(kind_f.line),
            format!(
                "`{}` 不是 kind，只能是：{}",
                kind_f.value,
                ChangeKind::vocabulary()
            ),
        )
    })?;
    let impact = require("predicted_impact")?;
    let predicted_impact = crate::lesson::parse_impact(impact.line, &impact.value)
        .map_err(|e| err(at(e.line), e.detail))?;

    let eval = block
        .get("eval")
        .map(|f| f.value.trim().to_string())
        .filter(|v| !v.is_empty() && v != "null");

    let realized_impact = match block.get("realized_impact") {
        Some(f) if f.value.trim() != "null" && !f.value.trim().is_empty() => {
            Some(parse_realized(at(f.line), &f.value)?)
        }
        _ => None,
    };

    Ok(ChangelogEntry {
        id: require("id")?.value.clone(),
        kind,
        clause: require("clause")?.value.clone(),
        evidence: yaml_lite::flow_seq(&require("evidence")?.value),
        predicted_impact,
        eval,
        realized_impact,
    })
}

fn parse_realized(line: usize, raw: &str) -> Result<RealizedImpact, Error> {
    let mut metric = None;
    let mut before = None;
    let mut after = None;
    let mut samples_before = None;
    let mut samples_after = None;
    for (k, v) in yaml_lite::flow_map(line, raw)? {
        let num = |v: &str| -> Result<f64, Error> {
            v.parse::<f64>()
                .map_err(|_| err(line, format!("`{v}` 不是数字")))
        };
        let count = |v: &str| -> Result<u32, Error> {
            v.parse::<u32>()
                .map_err(|_| err(line, format!("`{v}` 不是整数")))
        };
        match k.as_str() {
            "metric" => metric = Some(v),
            "before" => before = Some(num(&v)?),
            "after" => after = Some(num(&v)?),
            "samples_before" => samples_before = Some(count(&v)?),
            "samples_after" => samples_after = Some(count(&v)?),
            other => {
                return Err(err(
                    line,
                    format!("realized_impact 里没有 `{other}` 这个字段"),
                ));
            }
        }
    }
    Ok(RealizedImpact {
        metric: metric.ok_or_else(|| err(line, "realized_impact 缺少 metric"))?,
        before: before.ok_or_else(|| err(line, "realized_impact 缺少 before"))?,
        after: after.ok_or_else(|| err(line, "realized_impact 缺少 after"))?,
        samples_before: samples_before.unwrap_or(0),
        samples_after: samples_after.unwrap_or(0),
    })
}

/// Renders one entry back into the block form, for the core's own appends
/// (`retire` entries written by a forward rollback, `realized_impact`
/// backfill).
pub fn render_entry(e: &ChangelogEntry) -> String {
    let mut s = format!(
        "- id: {}\n  kind: {}\n  clause: {}\n  evidence: [{}]\n",
        e.id,
        e.kind.as_str(),
        e.clause,
        e.evidence.join(", ")
    );
    s.push_str(&format!(
        "  predicted_impact: {{metric: {}, direction: {}, scope: {}, horizon: {}}}\n",
        e.predicted_impact.metric,
        e.predicted_impact.direction.as_str(),
        e.predicted_impact.scope.as_str(),
        e.predicted_impact.horizon
    ));
    s.push_str(&format!(
        "  eval: {}\n",
        e.eval.clone().unwrap_or_else(|| "null".into())
    ));
    match &e.realized_impact {
        None => s.push_str("  realized_impact: null\n"),
        Some(r) => s.push_str(&format!(
            "  realized_impact: {{metric: {}, before: {}, after: {}, samples_before: {}, samples_after: {}}}\n",
            r.metric, r.before, r.after, r.samples_before, r.samples_after
        )),
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Scope;

    const DOC: &str = r#"# 协议改动记录

## protocol/v2

```yaml
- id: C-07
  kind: behavioral
  clause: loop-protocol.md#实现循环/自审清单
  evidence: [voice-schedule L-01, island-workbench L-04]
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
  eval: evals/self-check-not-applicable/
  realized_impact: null
```

## protocol/v1

```yaml
- id: C-01
  kind: clarify
  clause: session-protocol.md
  evidence: [audit-2026-09-16 S-01, voice-schedule L-09]
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 5}
  eval: null
  realized_impact: {metric: protocol_failures, before: 1.0, after: 0.0, samples_before: 3, samples_after: 3}
```
"#;

    fn has(paths: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |p: &str| paths.contains(&p)
    }

    #[test]
    fn versions_parse_in_document_order_with_their_entries() {
        let c = parse(DOC).unwrap();
        assert_eq!(
            c.versions.iter().map(|v| v.tag.as_str()).collect::<Vec<_>>(),
            vec!["protocol/v2", "protocol/v1"]
        );
        assert_eq!(c.entries().count(), 2);
    }

    #[test]
    fn an_entry_carries_its_clause_evidence_and_prediction() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        assert_eq!(e.id, "C-07");
        assert_eq!(e.kind, ChangeKind::Behavioral);
        assert_eq!(e.clause_file(), "loop-protocol.md");
        assert_eq!(e.evidence_tasks(), vec!["voice-schedule", "island-workbench"]);
        assert_eq!(e.predicted_impact.scope, Scope::Task);
        assert_eq!(e.eval.as_deref(), Some("evals/self-check-not-applicable/"));
        assert_eq!(e.realized_impact, None);
        assert_eq!(e.held_up(), None);
    }

    #[test]
    fn a_backfilled_entry_reports_whether_the_prediction_held() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v1").unwrap().entries[0];
        assert_eq!(e.held_up(), Some(true));
        assert!(e.realized_impact.as_ref().unwrap().enough_samples());
    }

    #[test]
    fn a_null_eval_reads_as_absent_rather_than_as_the_string_null() {
        let c = parse(DOC).unwrap();
        assert_eq!(c.version("protocol/v1").unwrap().entries[0].eval, None);
    }

    #[test]
    fn a_behavioral_change_with_a_real_case_and_two_tasks_passes() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        let problems = check_entry(
            e,
            &has(&[
                "loop-protocol.md",
                "evals/self-check-not-applicable/",
            ]),
        );
        assert_eq!(problems, vec![]);
    }

    #[test]
    fn a_behavioral_change_without_a_case_is_rejected() {
        let c = parse(&DOC.replace("  eval: evals/self-check-not-applicable/\n", "  eval: null\n"))
            .unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        let problems = check_entry(e, &has(&["loop-protocol.md"]));
        assert!(problems.contains(&EntryProblem::MissingEval { id: "C-07".into() }));
    }

    #[test]
    fn a_case_that_does_not_exist_is_a_different_complaint_from_no_case_at_all() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        let problems = check_entry(e, &has(&["loop-protocol.md"]));
        assert!(problems.contains(&EntryProblem::EvalNotFound {
            id: "C-07".into(),
            path: "evals/self-check-not-applicable/".into()
        }));
    }

    #[test]
    fn a_clarify_change_needs_no_case() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v1").unwrap().entries[0];
        assert_eq!(check_entry(e, &has(&["session-protocol.md"])), vec![]);
    }

    #[test]
    fn one_task_cited_twice_is_still_one_task() {
        let doc = DOC.replace(
            "evidence: [voice-schedule L-01, island-workbench L-04]",
            "evidence: [voice-schedule L-01, voice-schedule L-02]",
        );
        let c = parse(&doc).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        assert_eq!(e.evidence_tasks(), vec!["voice-schedule"]);
        let problems = check_entry(
            e,
            &has(&["loop-protocol.md", "evals/self-check-not-applicable/"]),
        );
        assert!(problems.contains(&EntryProblem::ThinEvidence {
            id: "C-07".into(),
            tasks: 1
        }));
    }

    #[test]
    fn a_retire_predicting_an_improvement_is_rejected() {
        let doc = DOC.replace(
            "- id: C-07\n  kind: behavioral",
            "- id: C-07\n  kind: retire",
        );
        let c = parse(&doc).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        let problems = check_entry(e, &has(&["loop-protocol.md"]));
        assert!(problems.contains(&EntryProblem::RetireMustBeFlat { id: "C-07".into() }));
    }

    #[test]
    fn a_retire_names_a_case_that_is_gone_and_that_is_fine() {
        let doc = DOC
            .replace("- id: C-07\n  kind: behavioral", "- id: C-07\n  kind: retire")
            .replace("direction: down", "direction: flat");
        let c = parse(&doc).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        assert_eq!(check_entry(e, &has(&["loop-protocol.md"])), vec![]);
    }

    #[test]
    fn a_clause_pointing_at_a_file_this_version_lacks_is_rejected() {
        let c = parse(DOC).unwrap();
        let e = &c.version("protocol/v2").unwrap().entries[0];
        let problems = check_entry(e, &has(&["evals/self-check-not-applicable/"]));
        assert!(problems.contains(&EntryProblem::ClauseFileMissing {
            id: "C-07".into(),
            file: "loop-protocol.md".into()
        }));
    }

    #[test]
    fn an_unmeasurable_metric_is_rejected_at_parse_time_not_gate_time() {
        let doc = DOC.replace("metric: verification_gaps", "metric: 顺畅度");
        assert!(parse(&doc).is_err());
    }

    #[test]
    fn a_bad_kind_names_the_vocabulary() {
        let doc = DOC.replace("kind: behavioral", "kind: tweak");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("clarify"), "{}", e.detail);
    }

    #[test]
    fn an_error_line_points_into_the_whole_file_not_into_its_section() {
        let doc = DOC.replace("kind: behavioral", "kind: tweak");
        let e = parse(&doc).unwrap_err();
        let want = doc
            .lines()
            .position(|l| l.contains("kind: tweak"))
            .unwrap()
            + 1;
        assert_eq!(e.line, want);
    }

    #[test]
    fn rendering_an_entry_and_reading_it_back_is_a_round_trip() {
        let c = parse(DOC).unwrap();
        for original in c.entries() {
            let text = format!("## protocol/vX\n\n```yaml\n{}```\n", render_entry(original));
            let back = parse(&text).unwrap();
            assert_eq!(&back.versions[0].entries[0], original);
        }
    }

    #[test]
    fn a_file_with_no_entries_yields_no_versions_worth_showing() {
        let c = parse("# 协议改动记录\n\n还没有任何改动。\n").unwrap();
        assert!(c.versions.is_empty());
    }
}
