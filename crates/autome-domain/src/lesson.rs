//! The retro round's output: one lesson per block in `docs/<slug>/lessons.md`.
//!
//! Before this existed, what a task learned died in the task directory. The
//! closing section of `retro.md` was free prose, nobody aggregated it, and the
//! only path from "we got this wrong three times" to "the protocol says not to"
//! ran through a human remembering.
//!
//! The block syntax is read by [`crate::yaml_lite`]; see there for why it is
//! not a YAML dependency.
//!
//! ## The cross-task key
//!
//! Two tasks learned the same lesson when `domain` matches and the normalised
//! `proposal` matches — whitespace and punctuation stripped, full-width forms
//! folded to half-width. Deliberately not semantic clustering: a key that is
//! only approximately stable turns "this happened twice" into a judgement call,
//! and the whole curation step (plan §E2) hangs off that count being a fact.

use serde::{Deserialize, Serialize};

use crate::metrics::{Direction, PredictedImpact, Scope, TaskMetrics};
use crate::yaml_lite::{self, Error, err};

/// The closed vocabulary a lesson's `domain` must come from (plan §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LessonDomain {
    Design,
    Verification,
    Implementation,
    ProtocolFormat,
    Tooling,
    Process,
}

impl LessonDomain {
    pub const ALL: [LessonDomain; 6] = [
        LessonDomain::Design,
        LessonDomain::Verification,
        LessonDomain::Implementation,
        LessonDomain::ProtocolFormat,
        LessonDomain::Tooling,
        LessonDomain::Process,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            LessonDomain::Design => "design",
            LessonDomain::Verification => "verification",
            LessonDomain::Implementation => "implementation",
            LessonDomain::ProtocolFormat => "protocol-format",
            LessonDomain::Tooling => "tooling",
            LessonDomain::Process => "process",
        }
    }

    pub fn parse(s: &str) -> Option<LessonDomain> {
        LessonDomain::ALL.into_iter().find(|d| d.as_str() == s)
    }

    pub fn vocabulary() -> String {
        LessonDomain::ALL
            .iter()
            .map(|d| d.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// Where a lesson wants to end up. Each level has a different destination and
/// a different gate (plan §E2–E4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonLevel {
    /// `.autome/rules/<domain>.md`, after the user approves the diff.
    Rule,
    /// A regression test in the project's own suite. The implementation round
    /// does that itself; the lesson only records that it did.
    Test,
    /// The per-round brief the core assembles.
    Brief,
    /// The protocol itself, via a meta task.
    Protocol,
}

impl LessonLevel {
    pub const ALL: [LessonLevel; 4] = [
        LessonLevel::Rule,
        LessonLevel::Test,
        LessonLevel::Brief,
        LessonLevel::Protocol,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            LessonLevel::Rule => "rule",
            LessonLevel::Test => "test",
            LessonLevel::Brief => "brief",
            LessonLevel::Protocol => "protocol",
        }
    }

    pub fn parse(s: &str) -> Option<LessonLevel> {
        LessonLevel::ALL.into_iter().find(|l| l.as_str() == s)
    }

    pub fn vocabulary() -> String {
        LessonLevel::ALL
            .iter()
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

pub const FIELDS: [&str; 8] = [
    "id",
    "domain",
    "symptom",
    "root_cause",
    "evidence",
    "level",
    "proposal",
    "predicted_impact",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lesson {
    pub id: String,
    pub domain: LessonDomain,
    pub symptom: String,
    pub root_cause: String,
    /// Repository-relative path to the evidence file this was read off.
    pub evidence: String,
    pub level: LessonLevel,
    /// One checkable sentence. This is what gets compared across tasks.
    pub proposal: String,
    pub predicted_impact: PredictedImpact,
}

impl Lesson {
    /// The cross-task identity (plan §3.3).
    pub fn key(&self) -> LessonKey {
        LessonKey {
            domain: self.domain,
            proposal: normalise(&self.proposal),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LessonKey {
    pub domain: LessonDomain,
    pub proposal: String,
}

/// Strips whitespace and punctuation and folds full-width characters, so that
/// two retro rounds writing the same sentence with different typography land on
/// the same key.
pub fn normalise(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let c = match c {
                '\u{3000}' => ' ',
                '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
                other => other,
            };
            if c.is_whitespace() || is_stripped_punctuation(c) {
                None
            } else {
                Some(c.to_lowercase().next().unwrap_or(c))
            }
        })
        .collect()
}

fn is_stripped_punctuation(c: char) -> bool {
    if c.is_ascii_punctuation() {
        return true;
    }
    matches!(
        c,
        '，' | '。'
            | '、'
            | '；'
            | '：'
            | '？'
            | '！'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '《'
            | '》'
            | '—'
            | '…'
            | '·'
            | '“'
            | '”'
            | '‘'
            | '’'
    )
}

/// Parses every lesson in a `lessons.md`. Blocks that are not lessons — prose
/// bullets, a note the retro round left — are skipped rather than rejected.
pub fn parse(doc: &str) -> Result<Vec<Lesson>, Error> {
    let mut out = Vec::new();
    for block in yaml_lite::blocks(doc) {
        if block.first_key() != "id" {
            continue;
        }
        out.push(parse_block(&block)?);
    }
    Ok(out)
}

fn parse_block(block: &yaml_lite::Block) -> Result<Lesson, Error> {
    block.reject_unknown(&FIELDS)?;
    if let Some(dup) = block.duplicate() {
        return Err(err(dup.line, format!("字段 `{}` 出现了两次", dup.key)));
    }

    let domain_f = block.require("domain")?;
    let domain = LessonDomain::parse(&domain_f.value).ok_or_else(|| {
        err(
            domain_f.line,
            format!(
                "`{}` 不在固定词表里，只能是：{}",
                domain_f.value,
                LessonDomain::vocabulary()
            ),
        )
    })?;
    let level_f = block.require("level")?;
    let level = LessonLevel::parse(&level_f.value).ok_or_else(|| {
        err(
            level_f.line,
            format!(
                "`{}` 不是 level，只能是：{}",
                level_f.value,
                LessonLevel::vocabulary()
            ),
        )
    })?;
    let proposal = block.require("proposal")?;
    if proposal.value.trim().is_empty() {
        return Err(err(proposal.line, "proposal 是空的"));
    }
    let impact = block.require("predicted_impact")?;

    Ok(Lesson {
        id: block.require("id")?.value.clone(),
        domain,
        symptom: block.require("symptom")?.value.clone(),
        root_cause: block.require("root_cause")?.value.clone(),
        evidence: block.require("evidence")?.value.clone(),
        level,
        proposal: proposal.value.clone(),
        predicted_impact: parse_impact(impact.line, &impact.value)?,
    })
}

/// Parses the inline form
/// `{metric: verification_gaps, direction: down, scope: task, horizon: 3}`.
///
/// The metric is checked against the schema here rather than later: a
/// prediction naming something nobody measures is not a weak prediction, it is
/// not a prediction.
pub fn parse_impact(line: usize, raw: &str) -> Result<PredictedImpact, Error> {
    let mut metric = None;
    let mut direction = None;
    let mut scope = None;
    let mut horizon = None;
    for (k, v) in yaml_lite::flow_map(line, raw)? {
        match k.as_str() {
            "metric" => metric = Some(v),
            "direction" => {
                direction = Some(Direction::parse(&v).ok_or_else(|| {
                    err(line, format!("`{v}` 不是 direction（down / up / flat）"))
                })?);
            }
            "scope" => {
                scope = Some(
                    Scope::parse(&v)
                        .ok_or_else(|| err(line, format!("`{v}` 不是 scope（task / project）")))?,
                );
            }
            "horizon" => {
                horizon = Some(
                    v.parse::<u32>()
                        .map_err(|_| err(line, format!("horizon `{v}` 不是整数")))?,
                );
            }
            other => {
                return Err(err(
                    line,
                    format!("predicted_impact 里没有 `{other}` 这个字段"),
                ));
            }
        }
    }

    let metric = metric.ok_or_else(|| err(line, "predicted_impact 缺少 metric"))?;
    if !TaskMetrics::is_metric(&metric) {
        return Err(err(
            line,
            format!(
                "metric `{metric}` 不在词表里，无法测量。可用的是：{}",
                TaskMetrics::METRIC_NAMES.join(" / ")
            ),
        ));
    }
    Ok(PredictedImpact {
        metric,
        direction: direction.ok_or_else(|| err(line, "predicted_impact 缺少 direction"))?,
        scope: scope.ok_or_else(|| err(line, "predicted_impact 缺少 scope"))?,
        horizon: horizon.ok_or_else(|| err(line, "predicted_impact 缺少 horizon"))?,
    })
}

// ---------------------------------------------------------------------------
// Aggregation across tasks
// ---------------------------------------------------------------------------

/// One lesson seen across a project, with the tasks that found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatedLesson {
    pub key: LessonKey,
    pub level: LessonLevel,
    /// The wording of the first occurrence; later ones only have to normalise
    /// to the same thing, not read identically.
    pub proposal: String,
    /// Task slugs, in first-seen order, deduplicated.
    pub tasks: Vec<String>,
    /// `<task> <lesson id>` per occurrence, for the evidence column.
    pub occurrences: Vec<String>,
}

impl AggregatedLesson {
    /// Plan §E2: a lesson becomes a rule proposal when a *second* task finds
    /// it. One task is an anecdote.
    pub fn is_corroborated(&self) -> bool {
        self.tasks.len() >= 2
    }
}

/// Groups lessons from many tasks by their cross-task key. Output is sorted by
/// key so the curation panel does not reshuffle between ticks.
pub fn aggregate<'a, I>(per_task: I) -> Vec<AggregatedLesson>
where
    I: IntoIterator<Item = (&'a str, &'a [Lesson])>,
{
    let mut out: Vec<AggregatedLesson> = Vec::new();
    for (task, lessons) in per_task {
        for lesson in lessons {
            let key = lesson.key();
            match out.iter_mut().find(|a| a.key == key) {
                Some(agg) => {
                    if !agg.tasks.iter().any(|t| t == task) {
                        agg.tasks.push(task.to_string());
                    }
                    agg.occurrences.push(format!("{task} {}", lesson.id));
                }
                None => out.push(AggregatedLesson {
                    key,
                    level: lesson.level,
                    proposal: lesson.proposal.clone(),
                    tasks: vec![task.to_string()],
                    occurrences: vec![format!("{task} {}", lesson.id)],
                }),
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = r#"# 教训

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #3 在 M-02 因情形表第 4 行未覆盖退回
  root_cause: 实现轮自审清单「情形表逐行」被写成「不适用」但未说明
  evidence: docs/voice-schedule/evidence/M-02-r5-audit.md
  level: rule
  proposal: 所有标「不适用」的自审项必须引用设计文档中证明其不适用的条款
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
"#;

    #[test]
    fn a_well_formed_block_parses_every_field() {
        let l = parse(ONE).unwrap();
        assert_eq!(l.len(), 1);
        let l = &l[0];
        assert_eq!(l.id, "L-01");
        assert_eq!(l.domain, LessonDomain::Verification);
        assert_eq!(l.level, LessonLevel::Rule);
        assert_eq!(l.evidence, "docs/voice-schedule/evidence/M-02-r5-audit.md");
        assert_eq!(l.predicted_impact.metric, "verification_gaps");
        assert_eq!(l.predicted_impact.direction, Direction::Down);
        assert_eq!(l.predicted_impact.scope, Scope::Task);
        assert_eq!(l.predicted_impact.horizon, 3);
    }

    #[test]
    fn a_hash_inside_a_value_survives_because_this_is_not_a_yaml_parser() {
        let l = &parse(ONE).unwrap()[0];
        assert!(
            l.symptom.contains("#3"),
            "the round number was eaten: {}",
            l.symptom
        );
    }

    #[test]
    fn prose_bullets_in_the_same_file_are_not_lessons() {
        let doc = format!("- 这一条是说明文字，不是教训\n\n{ONE}");
        assert_eq!(parse(&doc).unwrap().len(), 1);
    }

    #[test]
    fn a_domain_outside_the_vocabulary_names_the_vocabulary() {
        let doc = ONE.replace("domain: verification", "domain: 验证");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("protocol-format"), "{}", e.detail);
    }

    #[test]
    fn a_level_outside_the_vocabulary_names_the_vocabulary() {
        let doc = ONE.replace("level: rule", "level: 规则");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("brief"), "{}", e.detail);
    }

    #[test]
    fn a_metric_outside_the_vocabulary_is_rejected_at_parse_time() {
        let doc = ONE.replace("metric: verification_gaps", "metric: 感觉更好了");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("无法测量"), "{}", e.detail);
    }

    #[test]
    fn a_missing_field_is_reported_against_the_block_that_lacks_it() {
        let doc = ONE.replace("  level: rule\n", "");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("level"), "{}", e.detail);
    }

    #[test]
    fn an_empty_proposal_is_rejected() {
        let doc = ONE.replace(
            "  proposal: 所有标「不适用」的自审项必须引用设计文档中证明其不适用的条款\n",
            "  proposal:\n",
        );
        assert!(parse(&doc).is_err());
    }

    #[test]
    fn a_repeated_field_is_an_error_rather_than_last_one_wins() {
        let doc = ONE.replace("  level: rule\n", "  level: rule\n  level: protocol\n");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("两次"), "{}", e.detail);
    }

    #[test]
    fn a_field_the_schema_does_not_define_is_rejected() {
        let doc = ONE.replace("  level: rule\n", "  level: rule\n  severity: high\n");
        let e = parse(&doc).unwrap_err();
        assert!(e.detail.contains("severity"), "{}", e.detail);
    }

    #[test]
    fn an_unknown_key_inside_predicted_impact_is_an_error() {
        let doc = ONE.replace("horizon: 3}", "horizon: 3, mood: good}");
        assert!(parse(&doc).is_err());
    }

    #[test]
    fn typography_does_not_change_the_key() {
        let a = normalise("所有标「不适用」的自审项必须引用设计文档中证明其不适用的条款");
        let b = normalise("所有标 \"不适用\" 的自审项，必须引用设计文档中证明其不适用的条款。");
        assert_eq!(a, b);
    }

    #[test]
    fn full_width_letters_fold_onto_their_ascii_forms() {
        assert_eq!(normalise("ＡＢＣ"), normalise("abc"));
    }

    #[test]
    fn a_different_sentence_keeps_a_different_key() {
        assert_ne!(normalise("必须引用条款"), normalise("必须引用证据"));
    }

    fn lesson(id: &str, domain: LessonDomain, proposal: &str) -> Lesson {
        Lesson {
            id: id.into(),
            domain,
            symptom: "s".into(),
            root_cause: "r".into(),
            evidence: "e".into(),
            level: LessonLevel::Rule,
            proposal: proposal.into(),
            predicted_impact: PredictedImpact {
                metric: "reopen_total".into(),
                direction: Direction::Down,
                scope: Scope::Task,
                horizon: 3,
            },
        }
    }

    #[test]
    fn the_same_lesson_from_two_tasks_is_corroborated() {
        let a = [lesson("L-01", LessonDomain::Verification, "必须引用条款")];
        let b = [lesson("L-04", LessonDomain::Verification, "必须引用条款。")];
        let agg = aggregate([("voice-schedule", &a[..]), ("island-workbench", &b[..])]);
        assert_eq!(agg.len(), 1);
        assert!(agg[0].is_corroborated());
        assert_eq!(agg[0].tasks, vec!["voice-schedule", "island-workbench"]);
        assert_eq!(
            agg[0].occurrences,
            vec!["voice-schedule L-01", "island-workbench L-04"]
        );
    }

    #[test]
    fn the_same_sentence_in_two_domains_stays_two_lessons() {
        let a = [lesson("L-01", LessonDomain::Verification, "写清楚")];
        let b = [lesson("L-02", LessonDomain::Tooling, "写清楚")];
        assert_eq!(aggregate([("t1", &a[..]), ("t2", &b[..])]).len(), 2);
    }

    #[test]
    fn one_task_finding_the_same_lesson_twice_is_still_one_task() {
        let a = [
            lesson("L-01", LessonDomain::Verification, "写清楚"),
            lesson("L-02", LessonDomain::Verification, "写清楚"),
        ];
        let agg = aggregate([("t1", &a[..])]);
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].tasks.len(), 1);
        assert!(!agg[0].is_corroborated());
        assert_eq!(agg[0].occurrences.len(), 2);
    }
}
