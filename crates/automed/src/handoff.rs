//! Ledger-derived work handed to an implementation or fix session.
use autome_domain::scope::ScopeRestored;

pub struct Handoff {
    pub scope_patches: Vec<ScopeRestored>,
    /// Reserved for operator notes, connected in M-09.
    pub notes: Vec<serde_json::Value>,
}

impl Handoff {
    pub fn is_pending(&self) -> bool {
        !self.scope_patches.is_empty() || !self.notes.is_empty()
    }
}

pub fn pending(
    store: &crate::store::Store,
    task: &str,
) -> Result<Handoff, crate::scope::RestoreError> {
    Ok(Handoff {
        scope_patches: crate::scope::pending(store, task)?,
        notes: Vec::new(),
    })
}

pub fn sections(
    patches: &[autome_domain::scope::ScopeRestored],
    closed: Option<(&str, u32)>,
    bug: bool,
) -> Option<String> {
    if patches.is_empty() {
        return None;
    }
    let mut text = String::from("\n## 上一轮审计 / 复盘越界改动已被还原\n\n");
    for entry in patches {
        text.push_str(&format!("{}轮 #{}（会话 {}）改了文档目录之外的文件。Autome 已把它们还原到该会话开始时的样子，并把改动存成补丁：\n\n", entry.label, entry.round, entry.session_id));
        for repo in &entry.repos {
            text.push_str(&format!("- 补丁：`{}`（在仓库 `{}` 中应用）\n- 文件：{}\n- 摘要（git apply --stat）：\n{}\n", repo.patch.display(), repo.worktree.display(), repo.paths.join("、"), repo.stat));
        }
    }
    text.push_str("\n由你判断是否采纳：采纳就在仓库根 `git apply <补丁>`，按需调整，跑与改动直接相关的测试后提交；不采纳就写一句理由。\n");
    if let Some((last, round)) = closed {
        text.push_str(&format!("\n本轮只处理补丁：不改里程碑表，也不改已完成里程碑的『最新证据』行；证据写 `evidence/{last}-r{round}-impl.md`。\n"));
    }
    if bug {
        text = text.replace(
            "上一轮审计 / 复盘越界改动已被还原",
            "上一轮复核越界改动已被还原",
        );
        text.push_str("\n本轮因此按复核不通过退回修复轮。是否采纳及理由写进 Bug 文档里本轮的记录；补丁文件本身不要复制进仓库。\n");
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::scope::RestoredRepo;
    use serde_json::json;

    fn patch(id: &str) -> ScopeRestored {
        ScopeRestored {
            task: "T-1".into(),
            session_id: id.into(),
            role: "audit".into(),
            label: "审计".into(),
            round: 1,
            repos: vec![RestoredRepo {
                repo: "".into(),
                worktree: "/tmp/repo".into(),
                base: "base".into(),
                pre_head: "head".into(),
                commit: None,
                paths: vec!["test.rs".into()],
                patch: id.into(),
                stat: "1 file".into(),
            }],
        }
    }

    #[test]
    fn handoff_pending_lists_unhandled_scope_patches_in_ledger_order() {
        let store = crate::store::Store::open_in_memory().unwrap();
        assert!(!pending(&store, "T-1").unwrap().is_pending());
        for id in ["second", "handled", "first", "failed"] {
            store
                .append_event("session.scope_restore_begun", "T-1", json!(patch(id)))
                .unwrap();
        }
        store
            .append_event(
                "session.scope_patches_handled",
                "T-1",
                json!({"patches":["handled"]}),
            )
            .unwrap();
        store
            .append_event(
                "session.scope_restore_failed",
                "T-1",
                json!({"session_id":"failed"}),
            )
            .unwrap();
        let handoff = pending(&store, "T-1").unwrap();
        assert!(handoff.is_pending());
        assert!(handoff.notes.is_empty());
        assert_eq!(
            handoff
                .scope_patches
                .iter()
                .map(|p| p.session_id.as_str())
                .collect::<Vec<_>>(),
            ["second", "first"]
        );
        assert!(!pending(&store, "other").unwrap().is_pending());
    }

    #[test]
    fn handoff_brief_on_a_closed_table_says_patch_only() {
        let patches = [patch("one")];
        let text = sections(&patches, Some(("M-16", 80)), false).unwrap();
        assert!(text.contains("本轮只处理补丁：不改里程碑表，也不改已完成里程碑的『最新证据』行"));
        assert!(text.contains("evidence/M-16-r80-impl.md"));
        assert!(
            !sections(&patches, None, false)
                .unwrap()
                .contains("本轮只处理补丁")
        );
        assert!(sections(&[], Some(("M-01", 1)), false).is_none());
        let bug = sections(&patches, None, true).unwrap();
        assert!(bug.contains("上一轮复核越界改动已被还原"));
        assert!(bug.contains("按复核不通过退回修复轮"));
    }
}

pub fn test_fix_section() -> String {
    "\n## 修正回归测试\n\n这次是回来修测试的。缺陷说明见 Bug 文档『回归测试缺陷』小节。只能改回归测试与任务文档，产品修复已经在分支上。结束时删掉 `回归测试: 有缺陷` 并写 `status: 已定位`。\n".into()
}
