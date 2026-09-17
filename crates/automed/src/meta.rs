//! Improving the protocol is an ordinary Loop task that runs on the protocol
//! repository.
//!
//! No new state machine, no special-case scheduling, no second set of rules.
//! The design round proposes changes, the review round picks holes, the
//! implementation round edits the text and writes the eval case, the audit
//! round runs the gate, and the two human stopping points are the two human
//! stopping points. SAME-MODEL, the milestone states, the budget, the
//! worktree: all of it applies because none of it knows this task is special.
//!
//! What this module adds is the part a meta task cannot assemble for itself:
//! the evidence. A session cannot read the store, cannot see other projects'
//! tasks, and should not be trusted to summarise its own history. So the core
//! writes `docs/<slug>/inputs/` before the task starts, and the task's request
//! says: propose changes *from these*.
//!
//! ## The self-reference
//!
//! A meta task runs under the **current** protocol and proposes the next one.
//! That is fine for most of it and not fine in one place: the review round
//! cannot judge a change to its own prompt, and neither can the audit round.
//! There is no clever fix — an evaluator judging the rules it is evaluated
//! under is a fixed point, not a check. So the core marks those changes
//! `需人工特批` and both gates show it, and layer 3 of the eval gate plus the
//! two humans are what actually hold.

use autome_domain::lesson::{self, AggregatedLesson, Lesson, LessonLevel};
use autome_domain::metrics::TaskMetrics;

/// How the version page and the settings screen decide whether to suggest a
/// meta task. Suggestions only — nothing starts by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Enough has happened since the last version to have something to say.
    TasksSinceRelease { count: usize },
    /// The same protocol-level lesson in two different tasks.
    RepeatedLesson { proposal: String, tasks: Vec<String> },
    /// A task stopped because a document would not parse or a guard fired.
    ProtocolFailure { task: String },
}

impl std::fmt::Display for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trigger::TasksSinceRelease { count } => {
                write!(f, "自上一版协议以来完成了 {count} 个任务")
            }
            Trigger::RepeatedLesson { proposal, tasks } => write!(
                f,
                "「{proposal}」在 {} 个任务里各出现了一次：{}",
                tasks.len(),
                tasks.join("、")
            ),
            Trigger::ProtocolFailure { task } => write!(f, "{task} 因协议问题停下过"),
        }
    }
}

/// Plan §6.4's three triggers.
pub const TASKS_SINCE_RELEASE: usize = 3;

pub fn triggers(
    tasks_since_release: usize,
    lessons: &[AggregatedLesson],
    protocol_failures: &[String],
) -> Vec<Trigger> {
    let mut out = Vec::new();
    if tasks_since_release >= TASKS_SINCE_RELEASE {
        out.push(Trigger::TasksSinceRelease {
            count: tasks_since_release,
        });
    }
    for l in lessons {
        if l.level == LessonLevel::Protocol && l.is_corroborated() {
            out.push(Trigger::RepeatedLesson {
                proposal: l.proposal.clone(),
                tasks: l.tasks.clone(),
            });
        }
    }
    for task in protocol_failures {
        out.push(Trigger::ProtocolFailure { task: task.clone() });
    }
    out
}

/// One task's contribution to `inputs/`.
pub struct TaskInput {
    pub project: String,
    pub slug: String,
    pub title: String,
    pub metrics: TaskMetrics,
    pub lessons: Vec<Lesson>,
    /// The closing summary of `retro.md`, which is the one part the protocol
    /// allows to run to paragraphs.
    pub retro_tail: Option<String>,
    /// Set when the task stopped on a protocol error or a guard.
    pub failure: Option<String>,
}

/// The files the core writes into `docs/<slug>/inputs/`.
pub fn inputs(tasks: &[TaskInput], deferred: &[String], contradicted: &[String]) -> Vec<(String, String)> {
    vec![
        ("inputs/metrics.md".into(), metrics_md(tasks)),
        ("inputs/lessons.md".into(), lessons_md(tasks)),
        (
            "inputs/retro-suggestions.md".into(),
            retro_suggestions_md(tasks),
        ),
        ("inputs/failures.md".into(), failures_md(tasks, contradicted)),
        ("inputs/deferred.md".into(), deferred_md(deferred)),
    ]
}

fn metrics_md(tasks: &[TaskInput]) -> String {
    let mut s = String::from(
        "# 指标\n\n\
         每一行是一个已终结的任务。`协议` 列是它运行时固定的那一版——同名标签在\n\
         两台机器上可以指向不同内容，所以比较看的是哈希。\n\n\
         **跨项目只列不比。** 不同项目的任务难度不可比，把它们平均到一起得到的\n\
         数字看起来像个结论，其实什么都不是。\n\n",
    );
    if tasks.is_empty() {
        s.push_str("还没有任何带指标的已终结任务。\n");
        return s;
    }
    s.push_str(
        "| 项目 | 任务 | 协议 | 设计轮 | 实现轮 | 里程碑 | reopen | 实现缺陷 | 验证缺口 | 协议失败 | 关闭后被推翻 | 人工未确认 | tokens | turns |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for t in tasks {
        let m = &t.metrics;
        s.push_str(&format!(
            "| {} | {} | {} | {}/{} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            t.project,
            t.slug,
            m.protocol_ref.as_deref().unwrap_or("—"),
            m.design_rounds_used,
            m.design_rounds_limit,
            m.impl_rounds_used,
            m.budget_n,
            m.milestones,
            m.reopen_total,
            m.impl_defects,
            m.verification_gaps,
            m.protocol_failures,
            m.closed_then_contradicted,
            m.manual_items_open,
            m.total_tokens,
            m.total_turns,
        ));
    }

    s.push_str("\n## reopen 按领域\n\n");
    let mut any = false;
    for t in tasks {
        if t.metrics.reopen_by_domain.is_empty() {
            continue;
        }
        any = true;
        s.push_str(&format!(
            "- {}：{}\n",
            t.slug,
            t.metrics
                .reopen_by_domain
                .iter()
                .map(|(d, n)| format!("{d} × {n}"))
                .collect::<Vec<_>>()
                .join("、")
        ));
    }
    if !any {
        s.push_str("没有任何 reopen 被归到领域。\n");
    }
    s
}

fn lessons_md(tasks: &[TaskInput]) -> String {
    let per_task: Vec<(&str, &[Lesson])> = tasks
        .iter()
        .map(|t| (t.slug.as_str(), t.lessons.as_slice()))
        .collect();
    let aggregated = lesson::aggregate(per_task);
    let protocol_level: Vec<&AggregatedLesson> = aggregated
        .iter()
        .filter(|l| l.level == LessonLevel::Protocol)
        .collect();

    let mut s = String::from(
        "# 教训（level: protocol）\n\n\
         按「领域 + 归一化后的 proposal」合并。**出现在两个以上任务里的，\
         是这次迭代最该看的。** 一个任务只是一次倒霉。\n\n\
         这里只列 `level: protocol` 的条目。`rule` 走项目规则，`test` 由实现轮\n\
         自己搬进测试体系，`brief` 走简报——都不需要改协议。\n\n",
    );
    if protocol_level.is_empty() {
        s.push_str("没有任何 `level: protocol` 的教训。\n");
        return s;
    }
    for l in protocol_level {
        s.push_str(&format!(
            "## {}\n\n- 领域：`{}`\n- 出现在 {} 个任务：{}\n- 出处：{}\n\n",
            l.proposal,
            l.key.domain.as_str(),
            l.tasks.len(),
            l.tasks.join("、"),
            l.occurrences.join("、")
        ));
    }
    s
}

fn retro_suggestions_md(tasks: &[TaskInput]) -> String {
    let mut s = String::from(
        "# 各任务的终止总结\n\n\
         协议允许成段的只有这一处。它比 `lessons.md` 松，所以这里是原文照抄，\n\
         没有归并——归并会把一句没想清楚的话变成一条看起来有两个来源的证据。\n\n",
    );
    let mut any = false;
    for t in tasks {
        let Some(tail) = &t.retro_tail else { continue };
        any = true;
        s.push_str(&format!("## {} · {}\n\n{}\n\n", t.slug, t.title, tail.trim()));
    }
    if !any {
        s.push_str("没有任何任务留下终止总结。\n");
    }
    s
}

fn failures_md(tasks: &[TaskInput], contradicted: &[String]) -> String {
    let mut s = String::from(
        "# 出过问题的地方\n\n\
         两类：任务因协议问题停下，以及上一版协议的预测被结果推翻。\n\n\
         第二类尤其值得读——一条改动预测某个指标会降，结果它升了，那是关于\n\
         协议的一条真实证据，比任何复盘里的印象都硬。\n\n## 协议失败\n\n",
    );
    let mut any = false;
    for t in tasks {
        let Some(f) = &t.failure else { continue };
        any = true;
        s.push_str(&format!("### {}\n\n```text\n{}\n```\n\n", t.slug, f.trim()));
    }
    if !any {
        s.push_str("没有任务因协议问题停下。\n\n");
    }

    s.push_str("## 与预测相反的改动\n\n");
    if contradicted.is_empty() {
        s.push_str("没有。也可能是还没到 horizon，或者样本不足。\n");
    } else {
        for c in contradicted {
            s.push_str(&format!("- {c}\n"));
        }
    }
    s
}

fn deferred_md(deferred: &[String]) -> String {
    let mut s = String::from(
        "# 上一次推迟的改动\n\n\
         上一次元任务 Backlog 里的条目。Backlog 在元任务里的语义就是「推迟到\n\
         下一次迭代」，所以它们在这里等着——不是待办清单，是候选。\n\n",
    );
    if deferred.is_empty() {
        s.push_str("没有推迟的条目。\n");
    } else {
        for d in deferred {
            s.push_str(&format!("- {d}\n"));
        }
    }
    s
}

/// The one-line request a meta task is created with.
///
/// Fixed text rather than something the user types: the constraints are what
/// make a proposal checkable, and a user typing their own request would be
/// typing around them.
pub const REQUEST: &str = "依据 docs/<slug>/inputs/ 里的证据，提出对协议正文与 prompt 模板的改动。\
每条改动是 CHANGELOG 里的一条：引用至少两个任务的证据，predicted_impact 的 metric 只能取内核\
真的记录的那些指标，behavioral 类必须带一个「改前红、改后绿」的 eval 用例。\
不得触碰内核契约区（`<!-- kernel-contract: … -->` 包住的部分）。\
里程碑表每行一条改动，验收命令固定为 `automed protocol eval`。";

/// Files whose change the review and audit rounds are not competent to judge,
/// because judging them is judging their own instructions.
pub const NEEDS_HUMAN_APPROVAL: [&str; 2] = ["prompts/review.md", "prompts/audit.md"];

/// Whether a set of changed paths includes one of those.
pub fn needs_human_approval(changed: &[String]) -> Vec<String> {
    NEEDS_HUMAN_APPROVAL
        .iter()
        .filter(|f| changed.iter().any(|c| c.ends_with(*f)))
        .map(|f| f.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::lesson::{LessonDomain, LessonKey};
    use autome_domain::metrics::{Direction, PredictedImpact, Scope};

    fn lesson(id: &str, level: LessonLevel, proposal: &str) -> Lesson {
        Lesson {
            id: id.into(),
            domain: LessonDomain::Verification,
            symptom: "s".into(),
            root_cause: "r".into(),
            evidence: "e".into(),
            level,
            proposal: proposal.into(),
            predicted_impact: PredictedImpact {
                metric: "verification_gaps".into(),
                direction: Direction::Down,
                scope: Scope::Task,
                horizon: 3,
            },
        }
    }

    fn task(slug: &str, lessons: Vec<Lesson>) -> TaskInput {
        TaskInput {
            project: "autome".into(),
            slug: slug.into(),
            title: format!("{slug} 的标题"),
            metrics: TaskMetrics {
                protocol_ref: Some("protocol/v1@abc".into()),
                impl_rounds_used: 9,
                budget_n: 35,
                milestones: 4,
                reopen_total: 3,
                impl_defects: 2,
                reopen_by_domain: vec![("escaping".into(), 2)],
                ..Default::default()
            },
            lessons,
            retro_tail: Some(format!("{slug} 的终止总结。")),
            failure: None,
        }
    }

    fn agg(level: LessonLevel, proposal: &str, tasks: &[&str]) -> AggregatedLesson {
        AggregatedLesson {
            key: LessonKey {
                domain: LessonDomain::Verification,
                proposal: lesson::normalise(proposal),
            },
            level,
            proposal: proposal.into(),
            tasks: tasks.iter().map(|t| t.to_string()).collect(),
            occurrences: tasks.iter().map(|t| format!("{t} L-01")).collect(),
        }
    }

    #[test]
    fn three_finished_tasks_are_enough_to_suggest_an_iteration() {
        assert!(triggers(3, &[], &[]).iter().any(|t| matches!(
            t,
            Trigger::TasksSinceRelease { count: 3 }
        )));
        assert!(triggers(2, &[], &[]).is_empty());
    }

    #[test]
    fn a_protocol_lesson_has_to_repeat_before_it_suggests_anything() {
        let once = agg(LessonLevel::Protocol, "审计要先跑负向对照", &["a"]);
        assert!(triggers(0, &[once], &[]).is_empty());
        let twice = agg(LessonLevel::Protocol, "审计要先跑负向对照", &["a", "b"]);
        assert_eq!(triggers(0, &[twice], &[]).len(), 1);
    }

    #[test]
    fn a_repeated_rule_level_lesson_is_not_a_protocol_trigger() {
        // It goes to `.autome/rules/` through the curation card instead.
        let twice = agg(LessonLevel::Rule, "证据要写命令", &["a", "b"]);
        assert!(triggers(0, &[twice], &[]).is_empty());
    }

    #[test]
    fn one_protocol_failure_is_enough() {
        let t = triggers(0, &[], &["voice-schedule".into()]);
        assert_eq!(t.len(), 1);
        assert!(t[0].to_string().contains("voice-schedule"), "{}", t[0]);
    }

    #[test]
    fn the_metrics_table_names_the_version_each_task_ran_under() {
        let files = inputs(&[task("a", vec![])], &[], &[]);
        let metrics = &files.iter().find(|(p, _)| p.ends_with("metrics.md")).unwrap().1;
        assert!(metrics.contains("protocol/v1@abc"), "{metrics}");
        assert!(metrics.contains("| autome | a |"), "{metrics}");
        assert!(metrics.contains("escaping × 2"), "{metrics}");
        assert!(metrics.contains("跨项目只列不比"), "{metrics}");
    }

    #[test]
    fn only_protocol_level_lessons_reach_the_meta_task() {
        let tasks = [
            task(
                "a",
                vec![
                    lesson("L-01", LessonLevel::Protocol, "审计要先跑负向对照"),
                    lesson("L-02", LessonLevel::Rule, "证据要写命令"),
                ],
            ),
            task(
                "b",
                vec![lesson("L-01", LessonLevel::Protocol, "审计要先跑负向对照。")],
            ),
        ];
        let files = inputs(&tasks, &[], &[]);
        let lessons = &files.iter().find(|(p, _)| p.ends_with("lessons.md")).unwrap().1;
        assert!(lessons.contains("审计要先跑负向对照"), "{lessons}");
        assert!(lessons.contains("出现在 2 个任务"), "{lessons}");
        // A rule-level lesson is someone else's business.
        assert!(!lessons.contains("证据要写命令"), "{lessons}");
    }

    #[test]
    fn the_retro_summaries_are_quoted_rather_than_merged() {
        // Merging would turn one half-formed sentence into evidence that looks
        // like it has two sources.
        let files = inputs(&[task("a", vec![]), task("b", vec![])], &[], &[]);
        let s = &files
            .iter()
            .find(|(p, _)| p.ends_with("retro-suggestions.md"))
            .unwrap()
            .1;
        assert!(s.contains("a 的终止总结"), "{s}");
        assert!(s.contains("b 的终止总结"), "{s}");
    }

    #[test]
    fn a_prediction_that_went_the_other_way_lands_in_failures() {
        let files = inputs(
            &[task("a", vec![])],
            &[],
            &["C-07 预测 verification_gaps 会降，实际从 2.0 升到 5.0".into()],
        );
        let f = &files.iter().find(|(p, _)| p.ends_with("failures.md")).unwrap().1;
        assert!(f.contains("C-07"), "{f}");
        assert!(f.contains("与预测相反的改动"), "{f}");
    }

    #[test]
    fn every_input_file_says_something_when_there_is_nothing_to_say() {
        // An empty file reads as a bug; "there is none" reads as a fact.
        for (path, body) in inputs(&[], &[], &[]) {
            assert!(body.len() > 40, "{path} is empty");
            assert!(body.starts_with("# "), "{path} has no heading");
        }
    }

    #[test]
    fn deferred_items_carry_over_from_the_last_iteration() {
        let files = inputs(&[], &["B-02 把收敛模式的阈值做成可配置".into()], &[]);
        let d = &files.iter().find(|(p, _)| p.ends_with("deferred.md")).unwrap().1;
        assert!(d.contains("B-02"), "{d}");
    }

    #[test]
    fn a_change_to_the_reviewers_own_prompt_needs_a_human() {
        // An evaluator judging the rules it is evaluated under is a fixed
        // point, not a check.
        let changed = vec![
            "prompts/impl.md".to_string(),
            "prompts/review.md".to_string(),
        ];
        assert_eq!(needs_human_approval(&changed), vec!["prompts/review.md"]);
        assert!(needs_human_approval(&["loop-protocol.md".to_string()]).is_empty());
    }

    #[test]
    fn the_request_states_every_constraint_a_proposal_has_to_meet() {
        for phrase in [
            "两个任务",
            "predicted_impact",
            "eval 用例",
            "契约区",
            "automed protocol eval",
        ] {
            assert!(REQUEST.contains(phrase), "the request omits {phrase}");
        }
    }
}
