//! What each protocol version cost and produced, and whether its changes did
//! what they said they would (plan §6.6).
//!
//! This page does not decide anything. There is no automatic acceptance, no
//! automatic rollback, and no ratchet — a ratchet needs samples, and this
//! system produces single digits of tasks a month across projects of wildly
//! different difficulty. What it can do honestly is put the evidence in one
//! place: what each version's tasks actually cost, what each change predicted,
//! and what happened.
//!
//! Three rules keep it from lying:
//!
//! - **Fewer than three samples says so** instead of drawing a line between
//!   two points.
//! - **Comparisons are within one project.** Two projects' tasks are not
//!   comparable, so cross-project rows are listed and never differenced.
//! - **The warnings are warnings.** `protocol_failures`, `manual_items_open`
//!   and above all `closed_then_contradicted` going up marks a version red and
//!   feeds it into the next meta task's `inputs/failures.md`. It does not
//!   revert anything.

use std::collections::BTreeMap;

use autome_domain::metrics::{Direction, RealizedImpact, TaskMetrics};

/// The number of tasks below which a version's numbers are not a measurement.
pub const MIN_SAMPLES: usize = 3;

/// The metrics whose rise marks a version red (plan §6.6).
///
/// All three measure the system telling itself a comfortable story:
/// `closed_then_contradicted` is an audit that closed something it should not
/// have, `protocol_failures` is the document format going wrong, and
/// `manual_items_open` is work handed to the user and never confirmed.
pub const WARNING_METRICS: [&str; 3] = [
    "closed_then_contradicted",
    "protocol_failures",
    "manual_items_open",
];

#[derive(Debug, Clone, PartialEq)]
pub struct VersionRow {
    /// The wire form of the version, `protocol/v7@3f9a…`.
    pub protocol_ref: String,
    pub tag: String,
    /// Tasks that ran under this version, in this project.
    pub samples: usize,
    /// Metric name → mean over those tasks.
    pub means: BTreeMap<String, f64>,
    /// Metrics that got worse than the previous version's.
    pub warnings: Vec<String>,
}

impl VersionRow {
    /// Plan §6.6: below three tasks the page says "样本不足" rather than
    /// drawing a conclusion from two points.
    pub fn enough_samples(&self) -> bool {
        self.samples >= MIN_SAMPLES
    }

    pub fn mean(&self, metric: &str) -> Option<f64> {
        self.means.get(metric).copied()
    }
}

/// Groups one project's finished tasks by the protocol version they ran under.
///
/// `tasks` is in completion order, and the output preserves it: the version
/// page reads newest-last, and "the version before this one" has to mean the
/// one that actually preceded it rather than the one with the lower number.
pub fn rows(tasks: &[TaskMetrics]) -> Vec<VersionRow> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, Vec<&TaskMetrics>> = BTreeMap::new();

    for t in tasks {
        let Some(r) = t.protocol_ref.clone() else {
            // A task from before the protocol left the binary. It has metrics
            // and no version to attribute them to; leaving it out of every
            // version is the only honest placement.
            continue;
        };
        if !order.contains(&r) {
            order.push(r.clone());
        }
        grouped.entry(r).or_default().push(t);
    }

    let mut out: Vec<VersionRow> = Vec::new();
    for protocol_ref in order {
        let group = &grouped[&protocol_ref];
        let mut means = BTreeMap::new();
        for metric in TaskMetrics::METRIC_NAMES {
            let values: Vec<f64> = group.iter().filter_map(|t| t.value(metric)).collect();
            if values.is_empty() {
                continue;
            }
            means.insert(
                metric.to_string(),
                values.iter().sum::<f64>() / values.len() as f64,
            );
        }
        let tag = protocol_ref
            .split_once('@')
            .map(|(t, _)| t.to_string())
            .unwrap_or_else(|| protocol_ref.clone());
        let mut row = VersionRow {
            protocol_ref,
            tag,
            samples: group.len(),
            means,
            warnings: Vec::new(),
        };
        // Compared against the version immediately before it, and only when
        // both have enough tasks to be a measurement.
        if let Some(prev) = out.last()
            && prev.enough_samples()
            && row.enough_samples()
        {
            for metric in WARNING_METRICS {
                if let (Some(before), Some(after)) = (prev.mean(metric), row.mean(metric))
                    && after > before
                {
                    row.warnings.push(metric.to_string());
                }
            }
        }
        out.push(row);
    }
    out
}

/// Fills in what actually happened to a change's metric, once `horizon` tasks
/// have run under the new version.
///
/// `before` is every task under the version the change was made *from*;
/// `after` is the tasks under the new one, oldest first. `None` until there
/// are `horizon` of them — a prediction judged early is judged on noise.
pub fn realized(
    metric: &str,
    before: &[TaskMetrics],
    after: &[TaskMetrics],
    horizon: u32,
) -> Option<RealizedImpact> {
    if after.len() < horizon as usize {
        return None;
    }
    let mean = |ts: &[TaskMetrics]| -> Option<(f64, u32)> {
        let values: Vec<f64> = ts.iter().filter_map(|t| t.value(metric)).collect();
        if values.is_empty() {
            return None;
        }
        Some((
            values.iter().sum::<f64>() / values.len() as f64,
            values.len() as u32,
        ))
    };
    let (before_mean, samples_before) = mean(before)?;
    // Only the first `horizon` tasks count. Letting the window grow would mean
    // a prediction's verdict kept changing after it had been reported.
    let (after_mean, samples_after) = mean(&after[..horizon as usize])?;
    Some(RealizedImpact {
        metric: metric.to_string(),
        before: before_mean,
        after: after_mean,
        samples_before,
        samples_after,
    })
}

/// A one-line verdict for the version page.
pub fn verdict(realized: &RealizedImpact, predicted: Direction) -> &'static str {
    if !realized.enough_samples() {
        return "样本不足";
    }
    if realized.matches(predicted) {
        "符合预测"
    } else {
        "与预测相反"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(protocol: &str, reopen: u32, contradicted: u32) -> TaskMetrics {
        TaskMetrics {
            protocol_ref: Some(protocol.into()),
            reopen_total: reopen,
            closed_then_contradicted: contradicted,
            ..Default::default()
        }
    }

    #[test]
    fn tasks_are_grouped_by_version_in_the_order_they_ran() {
        let tasks = [
            task("protocol/v1@a", 4, 0),
            task("protocol/v2@b", 2, 0),
            task("protocol/v1@a", 6, 0),
        ];
        let rows = rows(&tasks);
        assert_eq!(
            rows.iter().map(|r| r.tag.as_str()).collect::<Vec<_>>(),
            vec!["protocol/v1", "protocol/v2"]
        );
        assert_eq!(rows[0].samples, 2);
        assert_eq!(rows[0].mean("reopen_total"), Some(5.0));
    }

    #[test]
    fn the_same_tag_with_a_different_hash_is_a_different_version() {
        // Two machines can have protocol/v7 pointing at different content.
        // Grouping by tag would silently average them together.
        let tasks = [task("protocol/v7@aaa", 2, 0), task("protocol/v7@bbb", 8, 0)];
        let rows = rows(&tasks);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].tag, rows[1].tag);
        assert_ne!(rows[0].protocol_ref, rows[1].protocol_ref);
    }

    #[test]
    fn a_task_from_before_the_protocol_was_versioned_is_left_out() {
        let tasks = [TaskMetrics::default(), task("protocol/v1@a", 3, 0)];
        let rows = rows(&tasks);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].samples, 1);
    }

    #[test]
    fn two_tasks_are_not_enough_to_be_a_measurement() {
        let tasks = [task("protocol/v1@a", 1, 0), task("protocol/v1@a", 3, 0)];
        assert!(!rows(&tasks)[0].enough_samples());
        let three = [
            task("protocol/v1@a", 1, 0),
            task("protocol/v1@a", 3, 0),
            task("protocol/v1@a", 2, 0),
        ];
        assert!(rows(&three)[0].enough_samples());
    }

    #[test]
    fn a_version_whose_audits_went_soft_is_flagged() {
        let mut tasks: Vec<TaskMetrics> = Vec::new();
        for _ in 0..3 {
            tasks.push(task("protocol/v1@a", 4, 0));
        }
        for _ in 0..3 {
            tasks.push(task("protocol/v2@b", 1, 2));
        }
        let rows = rows(&tasks);
        // Reopens went *down*, which the earlier heuristic would have read as
        // an improvement. Milestones being closed and taken back went up, and
        // that is the thing that means the audit stopped biting.
        assert!(rows[1].mean("reopen_total").unwrap() < rows[0].mean("reopen_total").unwrap());
        assert!(
            rows[1]
                .warnings
                .contains(&"closed_then_contradicted".to_string()),
            "{:?}",
            rows[1].warnings
        );
    }

    #[test]
    fn a_version_with_too_few_tasks_is_not_flagged_on_noise() {
        let tasks = [
            task("protocol/v1@a", 4, 0),
            task("protocol/v1@a", 4, 0),
            task("protocol/v1@a", 4, 0),
            task("protocol/v2@b", 1, 9),
        ];
        assert!(rows(&tasks)[1].warnings.is_empty());
    }

    #[test]
    fn a_prediction_is_not_judged_before_its_horizon() {
        let before = [task("protocol/v1@a", 6, 0), task("protocol/v1@a", 4, 0)];
        let after = [task("protocol/v2@b", 2, 0)];
        assert_eq!(realized("reopen_total", &before, &after, 3), None);
    }

    #[test]
    fn a_prediction_is_judged_on_exactly_its_horizon_and_not_on_what_came_later() {
        let before = [task("protocol/v1@a", 6, 0), task("protocol/v1@a", 4, 0)];
        let after = [
            task("protocol/v2@b", 2, 0),
            task("protocol/v2@b", 2, 0),
            task("protocol/v2@b", 2, 0),
            // A later disaster must not change a verdict already reported.
            task("protocol/v2@b", 90, 0),
        ];
        let r = realized("reopen_total", &before, &after, 3).unwrap();
        assert_eq!(r.before, 5.0);
        assert_eq!(r.after, 2.0);
        assert_eq!(r.samples_after, 3);
        assert!(r.matches(Direction::Down));
    }

    #[test]
    fn the_verdict_says_when_there_is_not_enough_to_judge_on() {
        let thin = RealizedImpact {
            metric: "reopen_total".into(),
            before: 5.0,
            after: 2.0,
            samples_before: 2,
            samples_after: 3,
        };
        assert_eq!(verdict(&thin, Direction::Down), "样本不足");
    }

    #[test]
    fn a_prediction_that_went_the_wrong_way_says_so() {
        let worse = RealizedImpact {
            metric: "reopen_total".into(),
            before: 2.0,
            after: 5.0,
            samples_before: 3,
            samples_after: 3,
        };
        assert_eq!(verdict(&worse, Direction::Down), "与预测相反");
        assert_eq!(verdict(&worse, Direction::Up), "符合预测");
    }

    #[test]
    fn a_metric_no_task_measured_is_absent_rather_than_zero() {
        let tasks = [task("protocol/v1@a", 3, 0)];
        assert_eq!(rows(&tasks)[0].mean("total_cost_usd"), None);
    }
}
