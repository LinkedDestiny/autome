//! From a lesson two tasks learned to a rule the next one is held to.
//!
//! The retro round writes lessons; this is what happens to the ones marked
//! `level: rule`. Three deliberate constraints:
//!
//! 1. **Two tasks, or nothing.** One task is a bad week. The cross-task key is
//!    exact (domain plus the normalised sentence), so "this happened twice" is
//!    a fact rather than a judgement call — which matters, because the whole
//!    step hangs off that count.
//! 2. **The user approves the diff.** A session may not write `.autome/`; nor
//!    may the core, silently. What the panel shows is the exact text that will
//!    be added, with a provenance comment naming the tasks it came from.
//! 3. **Retiring a rule is an experiment, not a cleanup.** A rule that has not
//!    been needed lately is a rule that is working — its absence from recent
//!    lessons is caused by its presence. So removal is proposed with a
//!    prediction ("nothing gets worse"), a deadline, and a one-click restore.

use autome_domain::lesson::{AggregatedLesson, LessonDomain, LessonLevel};
use autome_domain::metrics::{Direction, PredictedImpact, Scope};

/// How many tasks a rule can go unreferenced before removal is *offered*.
///
/// Deliberately long. The cost of keeping a rule that is no longer needed is
/// a few lines in a file a session reads; the cost of removing one that still
/// is, is the defect it was written for coming back.
pub const RETIREMENT_IDLE_TASKS: u32 = 6;

/// How many tasks a removal experiment runs for before it is judged.
pub const EXPERIMENT_HORIZON: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    /// Waiting for the user.
    Pending,
    Approved,
    /// The user said no. Recorded so the same proposal is not offered again
    /// every time the aggregation runs.
    Dismissed,
}

impl ProposalState {
    pub const fn as_str(self) -> &'static str {
        match self {
            ProposalState::Pending => "pending",
            ProposalState::Approved => "approved",
            ProposalState::Dismissed => "dismissed",
        }
    }

    pub fn parse(s: &str) -> Option<ProposalState> {
        match s {
            "pending" => Some(ProposalState::Pending),
            "approved" => Some(ProposalState::Approved),
            "dismissed" => Some(ProposalState::Dismissed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// `domain/normalised-proposal`, stable across runs.
    pub key: String,
    pub domain: LessonDomain,
    /// The sentence, in the wording of the first task that found it.
    pub proposal: String,
    /// `<task> <lesson id>` per occurrence.
    pub evidence: Vec<String>,
    pub tasks: Vec<String>,
}

impl Proposal {
    /// Where the rule goes. One file per domain, so a project's rules stay
    /// grouped by the kind of mistake they prevent rather than by the order
    /// they were found in.
    pub fn rule_file(&self) -> String {
        format!(".autome/rules/{}.md", self.domain.as_str())
    }

    /// Exactly what will be appended, provenance included.
    ///
    /// The comment is not decoration: six months later the only way to judge
    /// whether a rule is still earning its place is to see what it was written
    /// for, and a rule with no origin is one nobody dares remove.
    pub fn rule_text(&self, date: &str) -> String {
        format!(
            "<!-- since: {date} · from: {} -->\n- {}\n",
            self.evidence.join(", "),
            self.proposal
        )
    }
}

/// The key a lesson is proposed, stored and answered under.
///
/// One function rather than one `format!` per call site: the string is written
/// into `rule_proposals` when a proposal is made and read back when the rule
/// it became is checked for retirement, and those two have to agree or an
/// approved rule can never be matched to the lesson it came from.
pub fn proposal_key(lesson: &AggregatedLesson) -> String {
    format!("{}/{}", lesson.key.domain.as_str(), lesson.key.proposal)
}

/// Turns aggregated lessons into the proposals worth showing.
///
/// `already` is the keys that have been approved or dismissed before; a
/// proposal the user has answered is not asked again.
pub fn proposals(lessons: &[AggregatedLesson], already: &[String]) -> Vec<Proposal> {
    lessons
        .iter()
        .filter(|l| l.level == LessonLevel::Rule && l.is_corroborated())
        .map(|l| Proposal {
            key: proposal_key(l),
            domain: l.key.domain,
            proposal: l.proposal.clone(),
            evidence: l.occurrences.clone(),
            tasks: l.tasks.clone(),
        })
        .filter(|p| !already.contains(&p.key))
        .collect()
}

/// One rule in a project's `.autome/rules/`, as the retirement check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub file: String,
    /// The rule's own line, without the provenance comment.
    pub body: String,
    /// How many tasks have finished since a lesson last referred to it.
    pub idle_tasks: u32,
}

/// A rule the panel may offer to remove — as an experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalExperiment {
    pub rule_file: String,
    pub body: String,
    /// The metric the rule's domain moves, if it is doing anything.
    pub metric: String,
    /// Carried through from the `Rule` this was built from: it is the whole
    /// reason the offer is being made, and the panel has to be able to say
    /// "this has gone N tasks unmentioned" rather than just "remove?".
    pub idle_tasks: u32,
    pub predicted: PredictedImpact,
}

/// Which metric a domain's rules are supposed to move.
///
/// A removal experiment needs something to measure, and "the rule was about
/// verification" is what says which number to watch.
pub fn metric_for(domain: LessonDomain) -> &'static str {
    match domain {
        LessonDomain::Verification => "verification_gaps",
        LessonDomain::Implementation => "impl_defects",
        LessonDomain::Design => "reopen_total",
        LessonDomain::ProtocolFormat => "protocol_failures",
        LessonDomain::Tooling | LessonDomain::Process => "impl_rounds_used",
    }
}

/// Plan §E3: a rule is never retired for having gone quiet. It is removed as
/// an experiment that predicts nothing gets worse, and restored if something
/// does.
pub fn removal_experiments(rules: &[Rule]) -> Vec<RemovalExperiment> {
    rules
        .iter()
        .filter(|r| r.idle_tasks >= RETIREMENT_IDLE_TASKS)
        .map(|r| {
            let domain = r
                .file
                .rsplit('/')
                .next()
                .and_then(|n| LessonDomain::parse(n.trim_end_matches(".md")))
                .unwrap_or(LessonDomain::Process);
            let metric = metric_for(domain);
            RemovalExperiment {
                rule_file: r.file.clone(),
                body: r.body.clone(),
                metric: metric.to_string(),
                idle_tasks: r.idle_tasks,
                predicted: PredictedImpact {
                    metric: metric.to_string(),
                    // Not `down`. The claim is "removing this costs nothing",
                    // and a prediction that removal *improves* things would be
                    // unfalsifiable in the direction that matters.
                    direction: Direction::Flat,
                    scope: Scope::Project,
                    horizon: EXPERIMENT_HORIZON,
                },
            }
        })
        .collect()
}

/// Whether a running experiment should be judged, and what the verdict is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentVerdict {
    /// Fewer than `horizon` tasks have finished since the removal.
    TooEarly,
    /// The metric held or improved: the rule was not carrying its weight.
    Held,
    /// The metric got worse. The panel offers to put the rule back.
    Regressed,
}

pub fn judge(baseline: f64, after: &[f64], horizon: u32) -> ExperimentVerdict {
    if after.len() < horizon as usize {
        return ExperimentVerdict::TooEarly;
    }
    let window = &after[..horizon as usize];
    let mean = window.iter().sum::<f64>() / window.len() as f64;
    if mean > baseline {
        ExperimentVerdict::Regressed
    } else {
        ExperimentVerdict::Held
    }
}

/// Appends a rule to a rules file, creating the file's heading if it is new.
pub fn apply(existing: Option<&str>, domain: LessonDomain, text: &str) -> String {
    match existing {
        Some(current) => {
            let mut out = current.trim_end().to_string();
            out.push_str("\n\n");
            out.push_str(text);
            out
        }
        None => format!(
            "# {} 规则\n\n\
             这些是从任务的复盘里长出来的：同一条教训在两个不同任务里各出现过一次，\n\
             你批准之后才写进来。每条前面的注释记着它的出处。\n\n\
             一条规则不会因为「最近没再出问题」就被删掉——它没再出问题，很可能\n\
             正是因为它在。要删只能作为一次移除实验：预测什么都不会变差，到期\n\
             回看，变差了就放回来。\n\n{text}",
            domain.as_str()
        ),
    }
}

/// Removes a rule and its provenance comment from a rules file.
pub fn remove(existing: &str, body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let lines: Vec<&str> = existing.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        // A provenance comment belongs to the rule under it; dropping the rule
        // without it leaves an orphan comment pointing at nothing.
        if line.trim_start().starts_with("<!-- since:")
            && lines.get(i + 1).is_some_and(|next| next.contains(body))
        {
            i += 2;
            continue;
        }
        if line.contains(body) && line.trim_start().starts_with("- ") {
            i += 1;
            continue;
        }
        out.push(line);
        i += 1;
    }
    let mut text = out.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", text.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::lesson::LessonKey;

    fn agg(
        level: LessonLevel,
        domain: LessonDomain,
        proposal: &str,
        tasks: &[&str],
    ) -> AggregatedLesson {
        AggregatedLesson {
            key: LessonKey {
                domain,
                proposal: autome_domain::lesson::normalise(proposal),
            },
            level,
            proposal: proposal.into(),
            tasks: tasks.iter().map(|t| t.to_string()).collect(),
            occurrences: tasks.iter().map(|t| format!("{t} L-01")).collect(),
        }
    }

    #[test]
    fn a_lesson_from_one_task_is_not_proposed() {
        let l = agg(
            LessonLevel::Rule,
            LessonDomain::Verification,
            "证据文件必须逐字写出跑过的命令",
            &["a"],
        );
        assert!(proposals(&[l], &[]).is_empty());
    }

    #[test]
    fn the_same_lesson_from_two_tasks_becomes_a_proposal_with_its_evidence() {
        let l = agg(
            LessonLevel::Rule,
            LessonDomain::Verification,
            "证据文件必须逐字写出跑过的命令",
            &["voice-schedule", "island-workbench"],
        );
        let p = proposals(&[l], &[]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].rule_file(), ".autome/rules/verification.md");
        assert_eq!(p[0].tasks.len(), 2);
        let text = p[0].rule_text("2026-09-17");
        assert!(text.contains("voice-schedule L-01"), "{text}");
        assert!(text.contains("island-workbench L-01"), "{text}");
        assert!(text.contains("2026-09-17"), "{text}");
    }

    #[test]
    fn a_protocol_level_lesson_is_not_a_project_rule() {
        // It goes to a meta task instead.
        let l = agg(
            LessonLevel::Protocol,
            LessonDomain::Verification,
            "审计要先跑负向对照",
            &["a", "b"],
        );
        assert!(proposals(&[l], &[]).is_empty());
    }

    #[test]
    fn a_proposal_the_user_already_answered_is_not_asked_again() {
        let l = agg(
            LessonLevel::Rule,
            LessonDomain::Tooling,
            "跑测试前先 cargo build",
            &["a", "b"],
        );
        let key = proposals(std::slice::from_ref(&l), &[])[0].key.clone();
        assert!(proposals(&[l], &[key]).is_empty());
    }

    #[test]
    fn a_new_rules_file_explains_where_its_rules_come_from() {
        let text = apply(None, LessonDomain::Verification, "- 一条规则\n");
        assert!(text.contains("两个不同任务"), "{text}");
        assert!(text.contains("移除实验"), "{text}");
        assert!(text.contains("- 一条规则"), "{text}");
    }

    #[test]
    fn appending_to_an_existing_file_keeps_what_is_there() {
        let existing = "# verification 规则\n\n- 旧规则\n";
        let text = apply(Some(existing), LessonDomain::Verification, "- 新规则\n");
        assert!(text.contains("- 旧规则"), "{text}");
        assert!(text.contains("- 新规则"), "{text}");
        assert!(!text.contains("\n\n\n"), "{text}");
    }

    #[test]
    fn a_rule_that_has_gone_quiet_is_offered_as_an_experiment_not_a_deletion() {
        let rules = [Rule {
            file: ".autome/rules/verification.md".into(),
            body: "证据文件必须逐字写出跑过的命令".into(),
            idle_tasks: RETIREMENT_IDLE_TASKS,
        }];
        let e = removal_experiments(&rules);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].metric, "verification_gaps");
        // The claim is "removing this costs nothing", not "removing this
        // helps" — the second would be unfalsifiable where it matters.
        assert_eq!(e[0].predicted.direction, Direction::Flat);
        assert_eq!(e[0].predicted.horizon, EXPERIMENT_HORIZON);
    }

    #[test]
    fn a_rule_that_is_still_being_referred_to_is_left_alone() {
        let rules = [Rule {
            file: ".autome/rules/verification.md".into(),
            body: "x".into(),
            idle_tasks: RETIREMENT_IDLE_TASKS - 1,
        }];
        assert!(removal_experiments(&rules).is_empty());
    }

    #[test]
    fn an_experiment_is_not_judged_before_its_horizon() {
        assert_eq!(judge(2.0, &[1.0, 1.0], 3), ExperimentVerdict::TooEarly);
    }

    #[test]
    fn an_experiment_that_held_and_one_that_did_not() {
        assert_eq!(judge(2.0, &[2.0, 1.0, 2.0], 3), ExperimentVerdict::Held);
        assert_eq!(
            judge(2.0, &[3.0, 3.0, 3.0], 3),
            ExperimentVerdict::Regressed
        );
        // Exactly at the baseline holds: the prediction was that nothing gets
        // worse, and nothing did.
        assert_eq!(judge(2.0, &[2.0, 2.0, 2.0], 3), ExperimentVerdict::Held);
    }

    #[test]
    fn a_later_task_does_not_change_a_verdict_already_reported() {
        assert_eq!(
            judge(2.0, &[1.0, 1.0, 1.0, 99.0], 3),
            ExperimentVerdict::Held
        );
    }

    #[test]
    fn removing_a_rule_takes_its_provenance_comment_with_it() {
        let existing = "# verification 规则\n\n\
            <!-- since: 2026-01-01 · from: a L-01, b L-02 -->\n\
            - 甲规则\n\n\
            <!-- since: 2026-02-01 · from: c L-01, d L-03 -->\n\
            - 乙规则\n";
        let after = remove(existing, "甲规则");
        assert!(!after.contains("甲规则"), "{after}");
        assert!(
            !after.contains("a L-01"),
            "an orphan comment survived:\n{after}"
        );
        assert!(after.contains("乙规则"), "{after}");
        assert!(after.contains("c L-01"), "{after}");
    }

    #[test]
    fn removing_a_rule_that_is_not_there_changes_nothing_but_whitespace() {
        let existing = "# verification 规则\n\n- 甲规则\n";
        assert_eq!(remove(existing, "丙规则"), existing);
    }

    #[test]
    fn every_domain_has_a_metric_a_removal_experiment_can_watch() {
        for domain in LessonDomain::ALL {
            let metric = metric_for(domain);
            assert!(
                autome_domain::metrics::TaskMetrics::is_metric(metric),
                "{domain:?} watches {metric}, which nothing records"
            );
        }
    }
}
