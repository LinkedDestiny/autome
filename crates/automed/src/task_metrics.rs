//! Aggregating one task into the numbers a later decision can be made on.
//!
//! Most of this is arithmetic over things the system already knows: the status
//! block, the session ledger, the event stream. Two are not, and both are
//! worth their cost:
//!
//! - **`closed_then_contradicted`.** A milestone that was closed and then
//!   taken back — by a later audit, or by a human at the merge gate. This is
//!   the direct measurement of an audit going soft, and it replaces an
//!   earlier heuristic ("defects down *and* reopens down") that would have
//!   flagged a protocol which had genuinely improved. It cannot be read off
//!   the final document, because the final document shows the milestone as
//!   open and says nothing about it having once been closed; it is counted as
//!   it happens, from consecutive status snapshots.
//! - **`impl_defects` / `verification_gaps`.** The audit round's three-way
//!   verdict, counted from the evidence files rather than from anything the
//!   core observes — the distinction between "the product behaved wrongly"
//!   and "the acceptance could have passed a wrong implementation" exists
//!   only in the audit's own words.

use std::collections::BTreeMap;
use std::path::Path;

use autome_domain::metrics::TaskMetrics;
use autome_domain::status_block::{MilestoneState, StatusBlock};

/// Counts the audit verdicts recorded in a task's evidence directory.
///
/// Reads the audit-side files only. An implementation round's evidence
/// describes what it built; only an audit reaches one of the three verdicts,
/// and only the audit files carry the phrases.
pub fn count_verdicts(worktree: &Path, doc_dir: &str) -> (u32, u32) {
    let dir = worktree.join(doc_dir).join("evidence");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (0, 0);
    };
    let mut defects = 0;
    let mut gaps = 0;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        // `M-xx-r<k>-audit.md`. The role suffix is what keeps an audit file
        // from colliding with the implementation round of the same k — before
        // it existed the two wrote to the same path and the second one won.
        if !name.contains("-audit") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // The verdict is written in the protocol's own words, so these are the
        // phrases to look for. A file naming both — an audit that found a
        // defect and also strengthened an acceptance — counts once for each,
        // which is what happened.
        if text.contains("实现缺陷") {
            defects += 1;
        }
        if text.contains("验证缺口") {
            gaps += 1;
        }
    }
    (defects, gaps)
}

/// How many `## 人工验收清单` entries the user has not ticked off yet.
///
/// Reads the parsed document rather than scanning the text again. The
/// hand-written scanner this replaces was the fourth one over the same file
/// and had already re-acquired the bugs the real parser fixed years earlier:
/// it counted bullets inside fenced code blocks (documents show their own
/// structure as examples), and `## 人工验收清单（4 项）` missed the heading
/// entirely and reported zero — the same "reports zero however many it really
/// had" failure the line-start fix had just closed, one keystroke away.
pub fn manual_items_open(status: &StatusBlock) -> u32 {
    status.manual_items.iter().filter(|i| !i.ticked).count() as u32
}

/// Milestones that went from `已完成` back to any other state between two
/// snapshots.
pub fn newly_contradicted(before: &[(String, MilestoneState)], after: &StatusBlock) -> Vec<String> {
    let mut out = Vec::new();
    for (id, state) in before {
        if *state != MilestoneState::Done {
            continue;
        }
        match after.milestones.iter().find(|m| &m.id == id) {
            Some(m) if m.state != MilestoneState::Done => out.push(id.clone()),
            // A milestone that vanished from the table was not closed either.
            None => out.push(id.clone()),
            Some(_) => {}
        }
    }
    out
}

/// The snapshot form stored between sessions.
pub fn snapshot(status: &StatusBlock) -> Vec<(String, MilestoneState)> {
    status
        .milestones
        .iter()
        .map(|m| (m.id.clone(), m.state))
        .collect()
}

/// Everything the aggregation needs that is not in the status block.
pub struct Counts {
    pub protocol_failures: u32,
    pub closed_then_contradicted: u32,
    pub impl_defects: u32,
    pub verification_gaps: u32,
    pub manual_items_open: u32,
    pub total_cost_usd: Option<f64>,
    pub total_tokens: u64,
    pub total_turns: u64,
}

/// Folds a task's status block and its counted events into `TaskMetrics`.
pub fn aggregate(
    status: Option<&StatusBlock>,
    budget_n: u32,
    protocol_ref: Option<String>,
    counts: Counts,
) -> TaskMetrics {
    let mut by_domain: BTreeMap<String, u32> = BTreeMap::new();
    let (mut milestones, mut reopen_total) = (0, 0);
    let (mut design_used, mut design_limit, mut impl_used) = (0, 0, 0);

    if let Some(s) = status {
        milestones = s.milestones.len() as u32;
        design_used = s.design_round;
        design_limit = s.design_round_limit;
        impl_used = s.impl_round;
        for m in &s.milestones {
            reopen_total += m.reopen_count;
            for domain in &m.reopen_domains {
                *by_domain.entry(domain.clone()).or_insert(0) += 1;
            }
        }
    }

    TaskMetrics {
        protocol_ref,
        // Never populated: nothing writes `tasks.rules_hash`, so the source
        // was always `None`. `skip_serializing_if` kept it off the wire
        // either way, so dropping the argument changes no output.
        rules_hash: None,
        design_rounds_used: design_used,
        design_rounds_limit: design_limit,
        impl_rounds_used: impl_used,
        budget_n,
        milestones,
        reopen_total,
        reopen_by_domain: by_domain.into_iter().collect(),
        impl_defects: counts.impl_defects,
        verification_gaps: counts.verification_gaps,
        protocol_failures: counts.protocol_failures,
        closed_then_contradicted: counts.closed_then_contradicted,
        manual_items_open: counts.manual_items_open,
        total_cost_usd: counts.total_cost_usd,
        total_tokens: counts.total_tokens,
        total_turns: counts.total_turns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::status_block::{ConvergenceMode, DocStatus, Milestone};

    fn milestone(id: &str, state: MilestoneState, reopen: u32, domains: &[&str]) -> Milestone {
        Milestone {
            id: id.into(),
            state,
            title: id.into(),
            reopen_count: reopen,
            reopen_domains: domains.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn block(milestones: Vec<Milestone>) -> StatusBlock {
        StatusBlock {
            status: DocStatus::Implementing,
            design_round: 2,
            design_round_limit: 15,
            impl_round: 9,
            impl_round_limit: 35,
            current_milestone: None,
            current_milestone_reopens: 0,
            convergence_mode: ConvergenceMode::Normal,
            next_action: String::new(),
            milestones,
            backlog: vec![],
            disputes: vec![],
            manual_items: vec![],
        }
    }

    fn counts() -> Counts {
        Counts {
            protocol_failures: 0,
            closed_then_contradicted: 0,
            impl_defects: 0,
            verification_gaps: 0,
            manual_items_open: 0,
            total_cost_usd: None,
            total_tokens: 0,
            total_turns: 0,
        }
    }

    #[test]
    fn reopens_are_totalled_and_grouped_by_domain() {
        let s = block(vec![
            milestone("M-01", MilestoneState::Done, 2, &["escaping", "escaping"]),
            milestone("M-02", MilestoneState::Done, 1, &["timing"]),
        ]);
        let m = aggregate(Some(&s), 35, None, counts());
        assert_eq!(m.reopen_total, 3);
        assert_eq!(m.reopens_in("escaping"), 2);
        assert_eq!(m.reopens_in("timing"), 1);
        assert_eq!(m.milestones, 2);
        assert_eq!(m.impl_rounds_used, 9);
        assert_eq!(m.budget_n, 35);
    }

    #[test]
    fn a_task_with_no_readable_document_still_aggregates_what_is_known() {
        // A task that failed on a parse error has no status block at all. The
        // session ledger still knows what it cost.
        let m = aggregate(
            None,
            0,
            Some("protocol/v1@abc".into()),
            Counts {
                protocol_failures: 1,
                total_tokens: 500,
                ..counts()
            },
        );
        assert_eq!(m.protocol_failures, 1);
        assert_eq!(m.total_tokens, 500);
        assert_eq!(m.milestones, 0);
        assert_eq!(m.protocol_ref.as_deref(), Some("protocol/v1@abc"));
    }

    #[test]
    fn a_milestone_that_was_closed_and_then_reopened_is_contradicted() {
        let before = vec![
            ("M-01".to_string(), MilestoneState::Done),
            ("M-02".to_string(), MilestoneState::Pending),
        ];
        let after = block(vec![
            milestone("M-01", MilestoneState::Open, 1, &["escaping"]),
            milestone("M-02", MilestoneState::Done, 0, &[]),
        ]);
        assert_eq!(newly_contradicted(&before, &after), vec!["M-01"]);
    }

    #[test]
    fn a_milestone_closing_normally_is_not_contradicted() {
        let before = vec![("M-01".to_string(), MilestoneState::Pending)];
        let after = block(vec![milestone("M-01", MilestoneState::Done, 0, &[])]);
        assert!(newly_contradicted(&before, &after).is_empty());
    }

    #[test]
    fn a_closed_milestone_deleted_from_the_table_counts_as_contradicted() {
        // Otherwise a round could clear a reopen by removing the row.
        let before = vec![("M-01".to_string(), MilestoneState::Done)];
        let after = block(vec![milestone("M-02", MilestoneState::Open, 0, &[])]);
        assert_eq!(newly_contradicted(&before, &after), vec!["M-01"]);
    }

    /// Drives the real parser, which is the point of the change: every one of
    /// these cases is something the hand-written scanner got wrong or was one
    /// keystroke from getting wrong.
    fn manual(doc: &str) -> u32 {
        manual_items_open(&autome_domain::status_block::parse(doc).expect("parses"))
    }

    fn doc_with(sections: &str) -> String {
        format!(
            "status: 实现中\ndesign-round: 1/15\nimplementation-round: 3/10\n\
             current-milestone: M-01\ncurrent-milestone-reopens: 0\n\
             convergence-mode: normal\nnext-action: 无\n\n# T\n\n\
             ## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n\
             | M-01 | 开放 | 甲 | 0 | |\n\n{sections}"
        )
    }

    #[test]
    fn manual_items_are_counted_until_they_are_ticked() {
        let d = doc_with("## 人工验收清单\n\n- H-01 长按三秒\n- [x] H-02 已确认\n- H-03 看横幅\n");
        assert_eq!(manual(&d), 2);
    }

    #[test]
    fn a_design_document_without_the_section_has_no_open_items() {
        assert_eq!(manual(&doc_with("## Backlog\n\n- B-01 只是待办\n")), 0);
    }

    #[test]
    fn the_manual_list_stops_at_the_next_heading() {
        let d = doc_with("## 人工验收清单\n\n- H-01\n\n## Backlog\n\n- B-01\n- B-02\n- B-03\n");
        assert_eq!(manual(&d), 1);
    }

    #[test]
    fn naming_the_section_in_prose_does_not_shadow_the_real_one() {
        // A real document did this: `next-action` said "由用户逐条确认
        // `## 人工验收清单`". A scanner that searched the whole text bounded
        // the section from that sentence and reported zero.
        let d = doc_with(
            "next-action 提到了 `## 人工验收清单`。\n\n## 任务\n\n略\n\n\
             ## 人工验收清单\n\n- H-01 长按\n- H-02 横幅\n\n## Backlog\n\n- B-01\n",
        );
        assert_eq!(manual(&d), 2);
    }

    #[test]
    fn a_fenced_example_of_the_section_is_not_the_section() {
        // Design documents show their own structure. The parser skips fences
        // wholesale; the scanner this replaces counted the example.
        let d = doc_with(
            "```\n## 人工验收清单\n\n- H-99 这是示例\n```\n\n\
             ## 人工验收清单\n\n- H-01 真的那条\n",
        );
        assert_eq!(manual(&d), 1);
    }

    #[test]
    fn a_heading_with_a_count_after_it_is_still_the_section() {
        // `classify_heading` already tolerated `## Backlog（3）`; the scanner
        // required an exact match and would have reported zero here.
        let d = doc_with("## 人工验收清单（4 项）\n\n- H-01 甲\n- H-02 乙\n");
        assert_eq!(manual(&d), 2);
    }

    #[test]
    fn audit_verdicts_are_counted_from_the_audit_side_evidence_only() {
        let dir = std::env::temp_dir().join(format!("autome-verdicts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let evidence = dir.join("docs/demo/evidence");
        std::fs::create_dir_all(&evidence).unwrap();
        std::fs::write(
            evidence.join("M-01-r3-audit.md"),
            "结论：**实现缺陷**。退回 开放。",
        )
        .unwrap();
        std::fs::write(
            evidence.join("M-02-r5-audit.md"),
            "结论：验证缺口。当场加强验收并立即复验，通过。",
        )
        .unwrap();
        // An implementation round quoting the audit's words must not be
        // counted a second time.
        std::fs::write(
            evidence.join("M-01-r4-impl.md"),
            "上一轮审计判为实现缺陷，本轮修复。",
        )
        .unwrap();
        let (defects, gaps) = count_verdicts(&dir, "docs/demo");
        assert_eq!(defects, 1);
        assert_eq!(gaps, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_with_no_evidence_directory_counts_zero_rather_than_failing() {
        let dir = std::env::temp_dir().join(format!("autome-verdicts-none-{}", std::process::id()));
        assert_eq!(count_verdicts(&dir, "docs/demo"), (0, 0));
    }
}
