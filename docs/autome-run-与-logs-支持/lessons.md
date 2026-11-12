# 教训：`autome run` 与 `autome logs` 支持 `--json`（T-46）

本任务没有记录到 Autome 指标；下列条目只引用证据文件里的事实，不含推算数字。

```yaml
- id: L-01
  domain: design
  symptom: 实现 #2 跑 M-02 集成用例时发现设计 §4「会话选择…取最后一个」选中的是最早会话，只好就地改设计为 JSON 模式取 first()；设计声称「以下均已对照源码核实」，评审 #1 也逐条给了 file:line，两轮都没发现
  root_cause: 设计对「从列表里按位置取元素」的决定没有核对列表顺序——Store::list_sessions 是 ORDER BY started_at DESC（crates/automed/src/store.rs:1444），task.get 不反转，所以 last() 是最早而不是最新
  evidence: docs/autome-run-与-logs-支持/evidence/M-02-r2-impl.md
  level: brief
  proposal: 设计文档里凡是按位置从列表取元素（first/last/最新/最早/默认取一个）的决定，都必须引用产生该列表顺序的 ORDER BY 或排序代码的 file:line
  predicted_impact: {metric: impl_defects, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-02
  domain: process
  symptom: 实现 #2 查明人类模式 `autome logs <任务>`（不带 --session）一直显示最早会话（crates/autome-cli/src/main.rs:973 在 DESC 列表上取 .last()），按任务 §6-5 保留不改，但设计 Backlog B-01..B-04 里没有这个已知缺陷的条目，任务结束后它只留在一份证据文件里
  root_cause: 协议只要求「还可以更好」进 Backlog，没有要求把实现中查明、但因范围约束而保留的既有缺陷登记进 Backlog
  evidence: docs/autome-run-与-logs-支持/evidence/M-02-r2-impl.md
  level: brief
  proposal: 证据文件里每一处「保留原有行为」且该行为已被查明有错的地方，设计文档 `## Backlog` 必须有一条引用对应 file:line 的 B-xx
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-03
  domain: verification
  symptom: 评审 #1 R1-N01 指出 M-01 验收「stdout 事件 seq 列表 == 事后 events.since 列表」在起点 seq 取晚时两边都是空集，空集相等能通过，负向对照（恢复人类模式过滤）触发不了；裁决 #1 采纳，在契约里补了「事件行非空且含 task.head」
  root_cause: 验收契约用「输出集合等于参照集合」作断言，但没有排除两边都为空、比较空洞成立的情况
  evidence: docs/autome-run-与-logs-支持/autome-run-与-logs-支持-review.md
  level: rule
  proposal: 以「输出集合/列表等于参照集合/列表」为断言的验收契约与测试用例，必须同时断言该集合非空且包含至少一个具名元素
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-04
  domain: tooling
  symptom: 实现 #1、#2 都在会话沙箱里跑完整 `sh scripts/ci-local.sh`（168 秒、387 秒），两次都因 Desktop `electron dom harness` 子进程退出码 null 与 Web `listen EPERM 127.0.0.1` / `EMFILE` 退出 1；本任务没碰 apps/，Rust 部分全绿、两次审计都通过，retro.md 两条实现行的阻塞列却都登记成「阻止宿主门禁全绿」
  root_cause: 会话沙箱不允许起 Electron、不允许监听本地端口，Desktop/Web 门禁步骤在会话里必然失败；宿主门禁本来由 Autome 在沙箱外跑（任务 §6-6），实现轮重跑这些步骤拿不到有效证据，还会把环境限制写成阻塞
  evidence: docs/autome-run-与-logs-支持/evidence/M-01-r1-impl.md
  level: brief
  proposal: 任务没改动 apps/ 时，实现轮在会话内只跑 fmt、clippy、Rust 全量与里程碑验收命令，不跑 Desktop/Web 门禁步骤，retro.md 阻塞列也不登记沙箱的 EPERM/EMFILE/Electron 退出码 null
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
