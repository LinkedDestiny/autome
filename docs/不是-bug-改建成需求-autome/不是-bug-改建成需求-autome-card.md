# 「不是 Bug」改建成需求任务：`autome task convert`
## 背景
计划 §4.2：定位轮写出「不是 Bug」时任务停下等你决定——取消它，或者用同一份正文改建成需求任务。现在状态机会停在 `NotABug`（`bug_loop.rs:227`），CLI 提示让用户自己 `task new --brief <卡片>`（`render.rs:112`），但没有任何命令或 RPC 能导出卡片并改建；e2e `bug_loop_not_a_bug_…` 是在测试里直接读 store 再调 `task.create`。W7 任务详情页的「改为需求」按钮也以此为前提。
## 要做什么
1. 内核新增写方法 `task.convert {task_id, kind: "feature", start?: bool}`：只接受停在 `not_a_bug` 的 Bug 任务（其它状态拒绝并说明）；复制 brief、附件、`doc_refs` 新建一个需求任务（默认存为待开始，`start: true` 则直接排队），原任务取消并写事件 `task.converted {from, to}`，新任务写 `task.created {converted_from}`。新任务的卡片正文沿用原文，标题沿用原标题。
2. CLI：`autome task convert <任务> [--start]`，支持 `--json`（输出新任务）；`render.rs` 里 `not_a_bug` 的提示改成指向这条命令。
3. 网页网关 `write-gate.ts` 放行 `task.convert`（与 T-37 协调：若 T-37 已合并则在其基础上加）。
4. e2e：改建后新任务 `kind=feature`、brief 相同、附件文件存在、原任务 `cancelled`；非 `not_a_bug` 状态被拒。
## 不做什么
不做 W7 的按钮；不改协议 prompt。
## 验收标准
见「要做什么」4，另：`autome --help` 列出 `task convert`；`cargo test --workspace`、`npm test --prefix apps/web` 全绿。
## 参考
`crates/automed/src/dispatch.rs`（`task.create`、`task.cancel`）、`crates/autome-cli/src/{main.rs,render.rs}`、`crates/automed/tests/end_to_end.rs:4466`、计划 §4.2、§8。
