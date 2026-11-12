# 费用可见：任务与项目的累计费用，可选预算告警
## 背景
每个会话的 `cost_usd` 已入库（`store.rs` sessions 表），但只在 `autome status <任务>` 的「最近 5 个会话」里逐条显示。T-27 跑了 25 个实现轮，没有任何地方能看到这个任务总共花了多少钱；项目维度更没有。用户是按任务付费的，这是最该有的数字之一。
## 要做什么
1. 内核：`task.get` 返回 `cost: {usd_total, sessions, by_role: {...}}`；`project.get`/`project.list` 返回 `cost_usd_total`（未完成 + 已完成分开）；`dashboard.get` 的任务卡带 `cost_usd`。费用缺失（Codex 没有计费数据）的会话计为 0 但 `sessions_without_cost` 计数单列。
2. CLI：`autome status <任务>` 顶部一行「费用 $x.xx（n 个会话，其中 m 个无计费数据）」；`autome ls` 每行末尾加费用列；`autome status` 的「运行中」行带费用。`--json` 同步。
3. 可选预算：项目 `.autome/config.toml` `[budget] per_task_usd = 20`（可省）。任务累计超过时写一条 `task.budget_exceeded` 事件并 `raise_alert("budget_exceeded")`，**不停任务**（停不停由用户决定，可后续加 `stop_on_exceed`）。
4. 网页网关 read 白名单无需新增方法（走现有 `task.get`/`project.get`）。
## 不做什么
不做图表；不做按模型单价估算；不改桌面端渲染进程（W11 删除）。
## 验收标准
单测：聚合函数对缺失费用的处理；e2e：两个会话各记费用后 `task.get.cost.usd_total` 等于和；`[budget]` 超限产生一条警报且任务继续；CLI 快照测试。`cargo test --workspace` 全绿。
## 参考
`crates/automed/src/store.rs`（sessions、`cost_usd`）、`crates/automed/src/dispatch.rs`（`task.get`、`project.get`、`dashboard.get`）、`crates/autome-cli/src/{main.rs:720,render.rs}`。
