//! Filling in what actually happened, once enough tasks have run.
//!
//! Every changelog entry carries a prediction: this metric, this direction,
//! within this many tasks. The core fills in the outcome — not the person who
//! proposed the change, and not a session. That separation is the entire point
//! of writing the prediction down: a claim scored by its author is not a
//! claim, it is a preference.
//!
//! It runs when a task reaches a terminal state, because that is when a new
//! sample exists and never otherwise. Nothing here changes a rule; it writes
//! one field into `CHANGELOG.md` and commits it.

use autome_domain::changelog::{self, ChangelogEntry};
use autome_domain::metrics::TaskMetrics;

use crate::dispatch::Ctx;
use crate::version_page;

/// One entry that just became judgeable.
#[derive(Debug, Clone, PartialEq)]
pub struct Filled {
    pub id: String,
    pub metric: String,
    pub before: f64,
    pub after: f64,
    pub held_up: bool,
}

/// Which entries can now be judged, given the tasks that have run.
///
/// `by_version` is every task's metrics grouped in the order the versions were
/// released, oldest first. An entry in version *n* is measured against version
/// *n − 1*: the comparison a change earns is with what it replaced, not with
/// the whole history.
pub fn fillable(
    log: &changelog::Changelog,
    versions: &[(String, Vec<TaskMetrics>)],
) -> Vec<(ChangelogEntry, Filled)> {
    let mut out = Vec::new();
    for (i, (tag, after)) in versions.iter().enumerate() {
        if i == 0 {
            // The first version has nothing before it. Its entries stay
            // unjudged forever, which is honest: there is no comparison to
            // make, and inventing a baseline out of no data would be worse
            // than leaving the field null.
            continue;
        }
        let before = &versions[i - 1].1;
        let Some(version) = log.version(tag) else {
            continue;
        };
        for entry in &version.entries {
            if entry.realized_impact.is_some() {
                continue;
            }
            let Some(realized) = version_page::realized(
                &entry.predicted_impact.metric,
                before,
                after,
                entry.predicted_impact.horizon,
            ) else {
                continue;
            };
            let mut filled = entry.clone();
            let held_up = realized.matches(entry.predicted_impact.direction);
            filled.realized_impact = Some(realized.clone());
            out.push((
                filled,
                Filled {
                    id: entry.id.clone(),
                    metric: realized.metric,
                    before: realized.before,
                    after: realized.after,
                    held_up,
                },
            ));
        }
    }
    out
}

/// Rewrites one entry's `realized_impact` line in place.
///
/// Line-level rather than re-rendering the file: a changelog carries prose
/// between its blocks — the paragraph explaining what a version was for — and
/// regenerating the file would throw that away.
pub fn rewrite(text: &str, id: &str, rendered_line: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == format!("- id: {id}"))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with("- id:") || l.trim().starts_with("```"))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());
    let target = lines[start..end]
        .iter()
        .position(|l| l.trim_start().starts_with("realized_impact:"))
        .map(|i| start + i)?;

    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let indent: String = lines[target]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    out[target] = format!("{indent}{rendered_line}");
    Some(format!("{}\n", out.join("\n")))
}

/// Runs the backfill against the protocol repository. Best-effort: a failure
/// here loses a measurement, and making it fatal would lose a task.
pub fn run(ctx: &mut Ctx) -> Vec<Filled> {
    let Some(repo) = crate::protocol::open(&ctx.autome_home) else {
        return vec![];
    };
    let Ok(files) = repo.working_files() else {
        return vec![];
    };
    let Some(text) = files.get("CHANGELOG.md") else {
        return vec![];
    };
    let Ok(log) = changelog::parse(text) else {
        return vec![];
    };

    // Tasks grouped by the version they ran under, oldest version first.
    let mut all: Vec<TaskMetrics> = Vec::new();
    let Ok(projects) = ctx.store.list_projects() else {
        return vec![];
    };
    for project in projects {
        if let Ok(rows) = ctx.store.tasks_with_metrics(&project.id) {
            all.extend(rows.into_iter().map(|(_, m)| m));
        }
    }
    let rows = version_page::rows(&all);
    let versions: Vec<(String, Vec<TaskMetrics>)> = rows
        .iter()
        .map(|r| {
            (
                r.tag.clone(),
                all.iter()
                    .filter(|m| m.protocol_ref.as_deref() == Some(r.protocol_ref.as_str()))
                    .cloned()
                    .collect(),
            )
        })
        .collect();

    let filled = fillable(&log, &versions);
    if filled.is_empty() {
        return vec![];
    }

    let mut text = text.to_string();
    let mut done = Vec::new();
    for (entry, f) in filled {
        let rendered = changelog::render_entry(&entry);
        let Some(line) = rendered
            .lines()
            .find(|l| l.trim_start().starts_with("realized_impact:"))
        else {
            continue;
        };
        if let Some(next) = rewrite(&text, &entry.id, line.trim()) {
            text = next;
            done.push(f);
        }
    }
    if done.is_empty() {
        return vec![];
    }
    let path = repo.path.join("CHANGELOG.md");
    if std::fs::write(&path, &text).is_err() {
        return vec![];
    }
    let _ = crate::git::commit_paths(
        &repo.path,
        &["CHANGELOG.md"],
        &format!(
            "chore(protocol): 回填 {} 条改动的实际影响",
            done.len()
        ),
    );
    for f in &done {
        let _ = ctx.store.append_event(
            "protocol.impact_filled",
            "protocol",
            serde_json::json!({
                "id": f.id,
                "metric": f.metric,
                "before": f.before,
                "after": f.after,
                "held_up": f.held_up,
            }),
        );
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"# 协议改动记录

## protocol/v2

这一版是为了让审计先跑负向对照。

```yaml
- id: C-07
  kind: behavioral
  clause: loop-protocol.md
  evidence: [a L-01, b L-04]
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
  eval: evals/x/
  realized_impact: null
```

## protocol/v1

```yaml
- id: C-01
  kind: clarify
  clause: loop-protocol.md
  evidence: [a L-02, b L-05]
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```
"#;

    fn task(protocol: &str, reopen: u32) -> TaskMetrics {
        TaskMetrics {
            protocol_ref: Some(protocol.into()),
            reopen_total: reopen,
            ..Default::default()
        }
    }

    fn versions(v1: &[u32], v2: &[u32]) -> Vec<(String, Vec<TaskMetrics>)> {
        vec![
            (
                "protocol/v1".into(),
                v1.iter().map(|n| task("protocol/v1@a", *n)).collect(),
            ),
            (
                "protocol/v2".into(),
                v2.iter().map(|n| task("protocol/v2@b", *n)).collect(),
            ),
        ]
    }

    #[test]
    fn nothing_is_judged_before_the_horizon() {
        let log = changelog::parse(LOG).unwrap();
        assert!(fillable(&log, &versions(&[6, 4, 5], &[2, 2])).is_empty());
    }

    #[test]
    fn a_prediction_that_held_is_filled_in_as_such() {
        let log = changelog::parse(LOG).unwrap();
        let filled = fillable(&log, &versions(&[6, 4, 5], &[2, 2, 2]));
        assert_eq!(filled.len(), 1);
        assert_eq!(filled[0].1.id, "C-07");
        assert_eq!(filled[0].1.before, 5.0);
        assert_eq!(filled[0].1.after, 2.0);
        assert!(filled[0].1.held_up);
    }

    #[test]
    fn a_prediction_that_went_the_other_way_is_filled_in_as_such() {
        // This is the entry that lands in the next meta task's failures.md,
        // and it is the hardest evidence the system produces about itself.
        let log = changelog::parse(LOG).unwrap();
        let filled = fillable(&log, &versions(&[2, 2, 2], &[6, 5, 7]));
        assert_eq!(filled.len(), 1);
        assert!(!filled[0].1.held_up);
    }

    #[test]
    fn the_first_version_is_never_judged_because_there_is_nothing_before_it() {
        let log = changelog::parse(LOG).unwrap();
        let filled = fillable(&log, &versions(&[6, 4, 5], &[2, 2, 2]));
        assert!(
            filled.iter().all(|(e, _)| e.id != "C-01"),
            "v1 was judged against nothing"
        );
    }

    #[test]
    fn an_entry_that_already_has_an_outcome_is_left_alone() {
        let text = LOG.replace(
            "  realized_impact: null\n```\n\n## protocol/v1",
            "  realized_impact: {metric: reopen_total, before: 5, after: 2, samples_before: 3, samples_after: 3}\n```\n\n## protocol/v1",
        );
        let log = changelog::parse(&text).unwrap();
        assert!(fillable(&log, &versions(&[6, 4, 5], &[2, 2, 2])).is_empty());
    }

    #[test]
    fn rewriting_replaces_one_line_and_keeps_the_prose_around_it() {
        // Regenerating the file would throw away the paragraph explaining what
        // the version was for, which is the only part a person reads.
        let out = rewrite(
            LOG,
            "C-07",
            "realized_impact: {metric: reopen_total, before: 5, after: 2, samples_before: 3, samples_after: 3}",
        )
        .unwrap();
        assert!(out.contains("这一版是为了让审计先跑负向对照。"), "{out}");
        assert!(out.contains("before: 5"), "{out}");
        // The other entry is untouched.
        assert!(out.contains("- id: C-01"), "{out}");
        assert_eq!(out.matches("realized_impact: null").count(), 1, "{out}");
        // And it still parses.
        let back = changelog::parse(&out).unwrap();
        let e = back
            .entries()
            .find(|e| e.id == "C-07")
            .expect("C-07 survived");
        assert_eq!(e.held_up(), Some(true));
    }

    #[test]
    fn rewriting_an_entry_that_is_not_there_changes_nothing() {
        assert_eq!(rewrite(LOG, "C-99", "realized_impact: null"), None);
    }

    #[test]
    fn the_indentation_of_the_block_is_preserved() {
        let out = rewrite(LOG, "C-07", "realized_impact: null").unwrap();
        assert!(out.contains("\n  realized_impact: null"), "{out}");
    }
}
