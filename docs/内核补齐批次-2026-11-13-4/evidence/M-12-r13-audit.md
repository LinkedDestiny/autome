# M-12 审计复验（审计 #12，实现轮 r13）

- 验收命令 `cargo test -p automed -- task_documents_ && cargo test -p autome-cli task_documents_`：库 5 + end_to_end 3 + CLI 1 = 9 个全过，与契约预期一致（8 + 1）。
- 负向对照 1：`DocumentSource` 忽略默认分支（`branch` 恒为 None 的等价注入）→ `…after_cleanup_come_from_the_default_branch`、`…cancelled_unmerged_task_is_empty`、`…workspace_reads_the_docs_repo_default_branch` 三条红（5 中 3 红）。
- 负向对照 2：任务卡挪到任务文件之前（设计文档排在任务卡之后）→ `…lists_card_bug_doc_and_lessons_in_order` 红，另有两条默认分支用例顺序断言也红。
- 两次注入均已还原（`git status` 干净，仅 `.autome/output/` 有临时文件）。
- 代码核对：清单顺序、标签、`source`、ls-tree 过滤只列默认分支上存在的路径、取消未合并→空清单均与 F-R1/F-R2/F-R6 一致。
- 缺陷：无。人工验收项：无。
