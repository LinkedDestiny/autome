# 裁决记录

## 裁决轮 1（design-round 0 → 1）

评审结论：第 1 轮评审提出 6 项六类问题（D1-P01…P06）、3 项非阻塞建议（N1…N3）。裁决轮逐项核对源码，六项全部成立：

- `config.rs:519` 起 provenance 取整个角色的 `over.is_empty()`，技能单独走 `over.skills.unwrap_or_else`；
- `skill.rs:62` `is_project_scoped` 是 `sources.any(scope == Project)`；
- `task.rs:132` `awaits_user` 含 `NotABug`，`task-columns.ts:2` 的 `AWAITING_NODES` 同样含 `not_a_bug`；
- `task_metrics.rs` 的 `gate_counts` 只数 `gate` / `fix_gate` / `merge_gate`；
- `task-timeline-model.ts:16` 的 `operations` 已含 `reply` 等十种用户操作；
- `task-history.ts` / `eventbus.ts` 证明服务端可用不带 `subject` 的 `events.since` 做增量消费。

结论：六项全部采纳，已改设计；没有被驳回或重复的主张，所有主张复提计数为 0，没有争议项。

### 裁决明细

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01 | 用角色级 provenance 断言「全局绑定不生效」是错的：只改模型也会让角色变成「项目覆盖」，技能仍继承 | F-Q2；M-09 | 采纳 | `config.rs:519–528`：provenance 看整个角色，技能列表单独回退到全局 | F-Q2 改为：保存前只给中性提示、不列项目；保存后按读回的 resolved 技能列出「没拿到」的项目。验-7 的写法增加「A 只覆盖模型」判别场景。M-09 的名单用例、skills-real、负向对照同步 | 0 |
| D1-P02 | `project_scoped` 是「任一来源在项目」，同名混合技能会让合法的全局绑定被禁用 | F-Q3；M-09 | 采纳 | `skill.rs:62` 的 `any`；全局 `set_role` 用 `scan_global` 校验 | F-Q3 重写：全局范围只看 global 来源，项目范围看全部来源；纯全局 / 纯项目 / 同名混合三类分开；M-09 增加同名混合用例与负向对照 | 0 |
| D1-P03 | 候选任务按 `completed_at` / 未终结过滤，会丢掉今天已发生、随后取消或旧终结任务复盘的活动 | B-Q1；M-03 | 采纳 | `cancelled` 终结但不写 `completed_at`；`scheduler.rs` 支持终结任务的独立复盘会话 | B-Q1 改为服务端用不带 subject 的 `events.since` 增量消费，累加器只存小记录；合并数仍只数今天 Done。M-03 增加「取消后」「旧终结任务今天复盘」用例与负向对照 | 0 |
| D1-P04 | 总览数所有 `gate.result`，详情只数三个节点，同名「门禁」口径不同 | B-Q1；D-2；M-03；M-05 | 采纳 | `task_metrics.rs` 的 `gate_counts` 忽略 `repro_check` 等节点 | 新增「共用的等待口径」小节，把门禁口径定为 `gate` / `fix_gate` / `merge_gate` 的 `gate.result`；B-Q1、D-2 引用它；M-03、M-05 用例与 real 断言同步 | 0 |
| D1-P05 | 等待区间漏掉 `not_a_bug`；用户行枚举漏掉 `reply` 等已有操作 | B-Q1；B-2；D-2；D-3；M-03；M-05 | 采纳 | `task.rs:132` 与 `task-columns.ts:2` 含 not_a_bug；`task-timeline-model.ts:16` 已有 reply | `app/lib/waiting.ts` 共用口径：按状态枚举（不用 `await_` 前缀），含 `failed`，`paused` / `stopped` 明确不计时；D-3 用户行沿用 `operations` 全集加 `task.decided`，退出等待的那条写「等了 X」 | 0 |
| D1-P06 | 按事件序号缓存最终结果，会冻结仍在增长的等待时长，也不处理跨午夜 | B-Q1；D-2；M-03 | 采纳 | 序号不变时 `now` 与日期边界仍在变 | 缓存只存序号与闭合区间；loader 返回已闭合毫秒数、开放区间起点与 `dayStart` / `dayEnd`，组件用 `clock` 的 `now` 现算；越过 `dayEnd` 触发重新验证；D 的总用时复用同一纯函数。M-03 用例改为「序号不变但 now 前进则增长」 | 0 |

### 非阻塞建议

- N1（「全部 N 个」落点）：不属于六类，但改动只有一句，已顺手处理——只有一个项目时显示该链接，多项目时不显示；任务卡字面取舍第 4 条同步。
- N2（M-04 负向对照）：已把判别落到仍调用 `gatePhrase` 的在途卡片与 labels 用例，去掉对 board-done-real 的错误指涉。
- N3（批准设计摘要缺少评审 / 裁决入口）：进 Backlog，BL-06。

### 设计定稿判断

六项修改都落在具体的里程碑与验收用例上，没有新增里程碑，没有改动范围、依赖顺序或对任务卡字面的取舍（`set_loop` 修复仍在 M-08，等用户批准时确认）。裁决轮复核后没有剩余的六类问题：`status` 改为 `实现中`，里程碑表为完整的 M-01…M-11，`current-milestone` 指向 M-01，`design-round` 为 1/15，实现预算 5 × 11 = 55。

提交由 Autome 兜底（若本轮提交成功则以 git 记录为准）。
