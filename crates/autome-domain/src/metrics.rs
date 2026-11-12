//! What a session cost and what a task produced.
//!
//! The core ran for months without recording either. The CLIs were already
//! writing everything needed — Claude's stream ends in a `result` event
//! carrying cost, token counts and turn count; Codex will once it is invoked
//! with `--json` — and the kernel was throwing it away. Without this there is
//! no way to answer "did that protocol change help", which makes the whole
//! self-improvement idea decorative.
//!
//! Two rules that shaped the shapes below:
//!
//! - **Cross-runtime comparisons use tokens and turns, never money.** Codex
//!   does not report a price, and inventing one from a table we maintain would
//!   produce a number that looks authoritative and is not.
//! - **`mean_request_input` is the definition of "context per turn".** Not
//!   `cache_read ÷ turns`, which collapses the moment a cache misses.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::session::Session;

/// Reported dollars for finished sessions; missing prices contribute zero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSummary {
    pub usd_total: f64,
    pub sessions: u32,
    pub sessions_without_cost: u32,
    pub by_role: BTreeMap<String, RoleCost>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleCost {
    pub usd_total: f64,
    pub sessions: u32,
    pub sessions_without_cost: u32,
}

impl CostSummary {
    pub fn of<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> Self {
        let mut summary = Self::default();
        for session in sessions {
            if session.is_running() {
                continue;
            }
            let cost = session.metrics.cost_usd.unwrap_or(0.0);
            let missing = u32::from(session.metrics.cost_usd.is_none());
            summary.usd_total += cost;
            summary.sessions += 1;
            summary.sessions_without_cost += missing;
            let role = summary
                .by_role
                .entry(session.kind.cost_key().into())
                .or_default();
            role.usd_total += cost;
            role.sessions += 1;
            role.sessions_without_cost += missing;
        }
        summary
    }
}

/// One session's usage, filled in when the session is reaped (plan §3.1).
///
/// Every numeric field is `Option`: a session whose stream could not be parsed
/// records nothing rather than zero, because zero is a claim and absence is
/// not.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionMetrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Claude's `total_cost_usd`. Always `None` for Codex — see the module
    /// note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Claude: `num_turns`. Codex: the number of `turn.completed` events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<u64>,
    /// Claude: `duration_api_ms`. Codex: wall clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Mean input tokens per model request, cache hits included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_request_input: Option<u64>,
    /// Size of the design document when the session ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_doc_bytes: Option<u64>,
    /// Files under `docs/<slug>/evidence/` when the session ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_files: Option<u64>,
}

impl SessionMetrics {
    /// Whether anything at all was captured. A session that produced nothing
    /// measurable should not be averaged into a version's numbers.
    pub fn is_empty(&self) -> bool {
        self == &SessionMetrics::default()
    }

    pub fn total_tokens(&self) -> Option<u64> {
        match (
            self.input_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
            self.output_tokens,
        ) {
            (None, None, None, None) => None,
            (i, r, w, o) => Some(i.unwrap_or(0) + r.unwrap_or(0) + w.unwrap_or(0) + o.unwrap_or(0)),
        }
    }
}

/// Counted once per task when it reaches a terminal state (plan §3.2).
///
/// `closed_then_contradicted` is the one to read first. It counts milestones
/// that were closed and then taken back — by a later audit, or by a human at
/// the merge gate — and it is the direct measurement of an audit going soft.
/// The earlier heuristic ("defects down *and* reopens down") flagged a
/// protocol that had genuinely improved, which is exactly backwards.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskMetrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_hash: Option<String>,

    pub design_rounds_used: u32,
    pub design_rounds_limit: u32,
    pub impl_rounds_used: u32,
    pub budget_n: u32,

    pub milestones: u32,
    pub reopen_total: u32,
    /// Behaviour domain → how many reopens it accounted for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reopen_by_domain: Vec<(String, u32)>,

    /// Three-way audit conclusions, counted from the evidence files.
    pub impl_defects: u32,
    pub verification_gaps: u32,
    /// `Failed(protocol)` occurrences, re-runs included.
    pub protocol_failures: u32,
    /// Milestones closed and later taken back.
    pub closed_then_contradicted: u32,
    /// Unticked lines in `## 人工验收清单` at the merge gate.
    pub manual_items_open: u32,

    /// Claude sessions only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
    pub total_tokens: u64,
    pub total_turns: u64,
}

impl TaskMetrics {
    /// The metric names a `predicted_impact` may refer to (plan §3.3, §3.4).
    /// A proposal naming anything else is not measurable, and the review round
    /// rejects it on that ground alone.
    pub const METRIC_NAMES: [&'static str; 12] = [
        "design_rounds_used",
        "impl_rounds_used",
        "reopen_total",
        "impl_defects",
        "verification_gaps",
        "protocol_failures",
        "closed_then_contradicted",
        "manual_items_open",
        "total_tokens",
        "total_turns",
        "total_cost_usd",
        "mean_request_input",
    ];

    pub fn is_metric(name: &str) -> bool {
        Self::METRIC_NAMES.contains(&name)
    }

    /// Reads one metric by name, for `realized_impact` backfill. Returns
    /// `None` for names this task did not measure.
    pub fn value(&self, name: &str) -> Option<f64> {
        let v = match name {
            "design_rounds_used" => self.design_rounds_used as f64,
            "impl_rounds_used" => self.impl_rounds_used as f64,
            "reopen_total" => self.reopen_total as f64,
            "impl_defects" => self.impl_defects as f64,
            "verification_gaps" => self.verification_gaps as f64,
            "protocol_failures" => self.protocol_failures as f64,
            "closed_then_contradicted" => self.closed_then_contradicted as f64,
            "manual_items_open" => self.manual_items_open as f64,
            "total_tokens" => self.total_tokens as f64,
            "total_turns" => self.total_turns as f64,
            "total_cost_usd" => self.total_cost_usd?,
            _ => return None,
        };
        Some(v)
    }

    pub fn reopens_in(&self, domain: &str) -> u32 {
        self.reopen_by_domain
            .iter()
            .find(|(d, _)| d == domain)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }
}

/// Which way a change is predicted to move a metric (plan §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Down,
    Up,
    /// A removal experiment: the prediction is that nothing gets worse
    /// (plan §6.6). Not "we do not know" — a `flat` prediction is falsifiable.
    Flat,
}

impl Direction {
    pub const ALL: [Direction; 3] = [Direction::Down, Direction::Up, Direction::Flat];

    pub const fn as_str(self) -> &'static str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::Flat => "flat",
        }
    }

    pub fn parse(s: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Task,
    Project,
}

impl Scope {
    pub const ALL: [Scope; 2] = [Scope::Task, Scope::Project];

    pub const fn as_str(self) -> &'static str {
        match self {
            Scope::Task => "task",
            Scope::Project => "project",
        }
    }

    pub fn parse(s: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s2| s2.as_str() == s)
    }
}

/// A falsifiable claim attached to a lesson or a changelog entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictedImpact {
    pub metric: String,
    pub direction: Direction,
    pub scope: Scope,
    /// How many completed tasks to wait before judging.
    pub horizon: u32,
}

impl PredictedImpact {
    /// Whether this prediction can be checked at all. The review round of a
    /// meta task rejects an unmeasurable one (plan §6.3).
    pub fn is_measurable(&self) -> bool {
        TaskMetrics::is_metric(&self.metric) && self.horizon > 0
    }
}

/// What actually happened, filled in by the core `horizon` tasks later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealizedImpact {
    pub metric: String,
    /// Mean over the tasks that ran before the change.
    pub before: f64,
    /// Mean over the `horizon` tasks that ran after it.
    pub after: f64,
    pub samples_before: u32,
    pub samples_after: u32,
}

impl RealizedImpact {
    /// Whether the outcome matches the prediction. `Flat` counts as met when
    /// the metric did not get worse — a removal experiment is not required to
    /// improve anything, only to cost nothing.
    pub fn matches(&self, predicted: Direction) -> bool {
        match predicted {
            Direction::Down => self.after < self.before,
            Direction::Up => self.after > self.before,
            Direction::Flat => self.after <= self.before,
        }
    }

    /// Plan §6.6: fewer than three samples on either side and the version page
    /// says "样本不足" instead of drawing a conclusion.
    pub fn enough_samples(&self) -> bool {
        self.samples_before >= 3 && self.samples_after >= 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_metrics_are_distinguishable_from_measured_zeroes() {
        let empty = SessionMetrics::default();
        assert!(empty.is_empty());
        assert_eq!(empty.total_tokens(), None);

        let measured = SessionMetrics {
            output_tokens: Some(0),
            ..Default::default()
        };
        assert!(!measured.is_empty());
        assert_eq!(measured.total_tokens(), Some(0));
    }

    #[test]
    fn total_tokens_sums_every_bucket_including_cache() {
        let m = SessionMetrics {
            input_tokens: Some(10),
            cache_read_tokens: Some(100),
            cache_write_tokens: Some(5),
            output_tokens: Some(1),
            ..Default::default()
        };
        assert_eq!(m.total_tokens(), Some(116));
    }

    #[test]
    fn every_metric_name_can_actually_be_read_back() {
        let m = TaskMetrics {
            total_cost_usd: Some(1.5),
            ..Default::default()
        };
        for name in TaskMetrics::METRIC_NAMES {
            if name == "mean_request_input" {
                // Session-level; it has no task-level accessor by design.
                continue;
            }
            assert!(m.value(name).is_some(), "{name} is named but unreadable");
        }
    }

    #[test]
    fn a_cost_that_was_never_measured_reads_as_absent_not_zero() {
        let m = TaskMetrics::default();
        assert_eq!(m.value("total_cost_usd"), None);
    }

    #[test]
    fn an_unknown_metric_name_is_not_measurable() {
        let p = PredictedImpact {
            metric: "vibes".into(),
            direction: Direction::Down,
            scope: Scope::Task,
            horizon: 3,
        };
        assert!(!p.is_measurable());
    }

    #[test]
    fn a_zero_horizon_is_not_measurable_either() {
        let p = PredictedImpact {
            metric: "reopen_total".into(),
            direction: Direction::Down,
            scope: Scope::Task,
            horizon: 0,
        };
        assert!(!p.is_measurable());
    }

    #[test]
    fn a_flat_prediction_is_met_when_the_metric_holds_or_improves() {
        let same = RealizedImpact {
            metric: "reopen_total".into(),
            before: 4.0,
            after: 4.0,
            samples_before: 3,
            samples_after: 3,
        };
        assert!(same.matches(Direction::Flat));
        assert!(!same.matches(Direction::Down));

        let worse = RealizedImpact {
            after: 5.0,
            ..same.clone()
        };
        assert!(!worse.matches(Direction::Flat));

        let better = RealizedImpact { after: 3.0, ..same };
        assert!(better.matches(Direction::Flat));
        assert!(better.matches(Direction::Down));
    }

    #[test]
    fn three_samples_on_each_side_is_the_bar() {
        let r = RealizedImpact {
            metric: "reopen_total".into(),
            before: 1.0,
            after: 1.0,
            samples_before: 3,
            samples_after: 2,
        };
        assert!(!r.enough_samples());
        assert!(
            RealizedImpact {
                samples_after: 3,
                ..r
            }
            .enough_samples()
        );
    }

    #[test]
    fn reopens_in_an_unseen_domain_is_zero() {
        let m = TaskMetrics {
            reopen_by_domain: vec![("promo-case".into(), 2)],
            ..Default::default()
        };
        assert_eq!(m.reopens_in("promo-case"), 2);
        assert_eq!(m.reopens_in("nope"), 0);
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;
    use crate::role::{Role, Runtime};
    use crate::session::{BugStep, SessionKind, SessionLifecycle};

    fn session(cost: Option<f64>) -> Session {
        Session {
            id: "s".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Plan },
            runtime: Runtime::Claude,
            model: "test".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: "t0".into(),
            ended_at: Some("t1".into()),
            lifecycle: SessionLifecycle::Exited { exit_code: 0 },
            log_path: "".into(),
            pid: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: SessionMetrics {
                cost_usd: cost,
                ..Default::default()
            },
        }
    }

    #[test]
    fn cost_summary_counts_a_session_without_cost_as_zero_and_lists_it_separately() {
        let sessions = [session(Some(1.25)), session(Some(2.5)), session(None)];
        let result = CostSummary::of(&sessions);
        assert_eq!(
            (
                result.usd_total,
                result.sessions,
                result.sessions_without_cost
            ),
            (3.75, 3, 1)
        );
        for cost in [None, Some(0.0), Some(0.001), Some(1_000_000.0)] {
            let result = CostSummary::of(&[session(cost)]);
            assert_eq!(
                (
                    result.usd_total,
                    result.sessions,
                    result.sessions_without_cost
                ),
                (cost.unwrap_or(0.0), 1, u32::from(cost.is_none()))
            );
        }
    }

    #[test]
    fn cost_summary_skips_running_sessions() {
        let mut sessions = vec![session(Some(1.25)), session(Some(2.5)), session(None)];
        let baseline = CostSummary::of(&sessions);
        let mut running = session(Some(99.0));
        running.lifecycle = SessionLifecycle::Running;
        running.kind = SessionKind::Onboarding;
        sessions.push(running);
        assert_eq!(CostSummary::of(&sessions), baseline);
        for lifecycle in [
            SessionLifecycle::Exited { exit_code: 0 },
            SessionLifecycle::Exited { exit_code: 1 },
            SessionLifecycle::Killed,
            SessionLifecycle::Vanished,
        ] {
            sessions[3].lifecycle = lifecycle;
            assert_eq!(CostSummary::of(&sessions).sessions, 4);
            assert_eq!(CostSummary::of(&sessions).usd_total, 102.75);
        }
    }

    #[test]
    fn cost_summary_splits_by_role_with_bug_steps_and_intake_as_their_own_keys() {
        let kinds = [
            (SessionKind::Role { role: Role::Plan }, "plan"),
            (SessionKind::Role { role: Role::Review }, "review"),
            (
                SessionKind::Role {
                    role: Role::Adjudicate,
                },
                "adjudicate",
            ),
            (SessionKind::Role { role: Role::Impl }, "impl"),
            (SessionKind::Role { role: Role::Audit }, "audit"),
            (SessionKind::Role { role: Role::Retro }, "retro"),
            (
                SessionKind::Bug {
                    step: BugStep::Locate,
                },
                "locate",
            ),
            (SessionKind::Bug { step: BugStep::Fix }, "fix"),
            (
                SessionKind::Bug {
                    step: BugStep::FixReview,
                },
                "fix_review",
            ),
            (SessionKind::Intake, "intake"),
            (SessionKind::Onboarding, "onboarding"),
        ];
        let mut sessions = vec![];
        for (kind, key) in kinds {
            assert_eq!(kind.cost_key(), key);
            for cost in [Some(1.25), None] {
                let mut s = session(cost);
                s.kind = kind;
                sessions.push(s);
            }
            let summary = CostSummary::of(&sessions);
            assert_eq!(
                summary.by_role[key],
                RoleCost {
                    usd_total: 1.25,
                    sessions: 2,
                    sessions_without_cost: 1
                }
            );
            assert_eq!(summary.by_role.len(), sessions.len() / 2);
        }
    }

    #[test]
    fn cost_summary_of_no_sessions_is_zero_not_absent() {
        let summary = CostSummary::of(&[]);
        assert_eq!(
            serde_json::to_value(summary).unwrap(),
            serde_json::json!({
                "usd_total": 0.0, "sessions": 0, "sessions_without_cost": 0, "by_role": {}
            })
        );
    }
}

/// Warn once per increasing budget, absorbing only floating point tail error.
pub fn budget_crossed(usd_total: f64, limit_cents: u64, last_alerted_cents: Option<u64>) -> bool {
    let limit = limit_cents as f64;
    let tolerance = 1e-6 + limit * 1e-12;
    usd_total * 100.0 > limit + tolerance
        && last_alerted_cents.is_none_or(|last| limit_cents > last)
}

#[cfg(test)]
mod budget_tests {
    use super::budget_crossed;
    #[test]
    fn budget_alert_requires_strictly_more_than_the_limit() {
        for (total, cents, expected) in [
            (20.0, 2000, false),
            (20.25, 2000, true),
            (0.07, 7, false),
            (0.071, 7, true),
            (0.1 + 0.2, 30, false),
            (0.0, 1, false),
            (0.01, 1, false),
            (0.011, 1, true),
            (1_000_000.0, 100_000_000, false),
            (1_000_000.001, 100_000_000, true),
        ] {
            assert_eq!(budget_crossed(total, cents, None), expected);
        }
    }
    #[test]
    fn budget_alert_does_not_repeat_for_the_same_or_a_lower_limit() {
        assert!(!budget_crossed(40.0, 2000, Some(2000)));
        assert!(!budget_crossed(40.0, 1500, Some(2000)));
    }
    #[test]
    fn budget_alert_fires_again_only_after_a_raised_limit_is_crossed() {
        assert!(!budget_crossed(30.0, 3000, Some(2000)));
        assert!(!budget_crossed(25.0, 3000, Some(2000)));
        assert!(budget_crossed(30.25, 3000, Some(2000)));
    }
}
