//! The IPC surface for the protocol and the version page.
//!
//! Kept out of `dispatch` because it is a self-contained group — the version
//! in force, its changelog, and what each version tasks ran under cost — and
//! `dispatch` is already the longest file in the crate.
//!
//! Read-only. The protocol ships inside the binary (see [`protocol::current`]);
//! there is nothing on this machine to pin, roll back or edit.

use autome_domain::changelog;
use autome_domain::metrics::TaskMetrics;
use serde_json::json;

use crate::dispatch::{Ctx, DispatchResult, rejected};
use crate::{protocol, version_page};

/// `protocol.get` — the version in force and what is in it.
pub fn get(_ctx: &mut Ctx) -> DispatchResult {
    let (current, files) = protocol::current();
    let log = files
        .get("CHANGELOG.md")
        .map(changelog::parse)
        .transpose()
        .map_err(|e| rejected(format!("CHANGELOG.md {e}")))?;

    Ok((
        json!({
            "current": { "tag": current.tag, "hash": current.hash, "wire": current.to_wire() },
            "bytes": files.sized_bytes(),
            "byte_budget": autome_domain::protocol::SIZE_BUDGET_BYTES,
            "changelog": log,
        }),
        vec![],
    ))
}

/// `protocol.versions` — the version page's table for one project.
///
/// Rows are whatever versions this project's tasks ran under. Their changelog
/// entries come from the built-in changelog, which carries every version's
/// section; a row naming a version this build does not describe simply has
/// none.
pub fn versions(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let tasks: Vec<TaskMetrics> = ctx
        .store
        .tasks_with_metrics(project_id)?
        .into_iter()
        .map(|(_, m)| m)
        .collect();
    let rows = version_page::rows(&tasks);

    let mut entries_by_tag = serde_json::Map::new();
    let (_, files) = protocol::current();
    if let Some(Ok(log)) = files.get("CHANGELOG.md").map(changelog::parse) {
        for row in &rows {
            if let Some(version) = log.version(&row.tag) {
                entries_by_tag.insert(row.tag.clone(), json!(version.entries));
            }
        }
    }

    Ok((
        json!({
            "rows": rows.iter().map(|r| json!({
                "protocol_ref": r.protocol_ref,
                "tag": r.tag,
                "samples": r.samples,
                "enough_samples": r.enough_samples(),
                "means": r.means,
                "warnings": r.warnings,
            })).collect::<Vec<_>>(),
            "metric_names": TaskMetrics::METRIC_NAMES,
            "min_samples": version_page::MIN_SAMPLES,
            "changelog_by_tag": entries_by_tag,
        }),
        vec![],
    ))
}
