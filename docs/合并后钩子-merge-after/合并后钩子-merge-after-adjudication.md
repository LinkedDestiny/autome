# 设计裁决记录：合并后钩子 `[merge] after`

## 裁决 #1（设计轮 d=1）

- 评审依据：`合并后钩子-merge-after-review.md`（评审 #1，2 项六类问题 + 1 项非阻塞建议）。
- 评审结论：两项均属实，均已核对源码（`scheduler.rs::run_core_step` Cleanup 段、`setup.rs`、`store.rs::append_event` / `raise_alert` 为独立 INSERT）。
- 本轮 `design-round` 0 → 1。修改后无剩余六类问题，`status` 由「设计中」改为「实现中」，里程碑表（M-01..M-04）保持完整。
- 提交：见 git 记录；若被拒绝由 Autome 兜底提交。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01 | 钩子因主检出互斥返回 `Pending` 时没有任何「已开始」痕迹，`begun=false`，下一 tick 又进入 `[setup]` 主检出准备；准备失败（不写 `main.fp`）时重复执行并重复报 `setup_failed` | §3 `begun`、步骤 4、整体顺序第 1 项；§7 | **采纳** | 反例成立：T1 钩子在跑，T2 准备失败后因互斥 `Pending`，下一 tick 无标记 → 再准备、再报警；`store.rs::raise_alert` 不去重。根因是用「钩子是否启动」代替「准备段是否处理完」 | 方案 §3 新增 `prepared` 标记（`gates/<task>/after_merge.prepared`，准备段 Ready/Failed 或不存在时即写，Pending 不写），`begun` 以它为主；§7 第三条改写并说明剩余窗口与今天同形；§8、§9 同步；M-03 新增 L5 与对应负向对照 | 0 |
| D1-P02-a | 失败时先写 `merge.after` 再写警报，两者是独立 INSERT，崩溃于其间则 `concluded` 为真而警报永缺 | §3 步骤 1、2；§8 | **采纳** | `append_event` 与 `raise_alert` 各自提交，`run_core_step` 无事务；交换顺序只会换成重复警报窗口，需原子写 | 新增 `Store::conclude_merge_after`：一个事务写警报、`project.alert`、`merge.after`；§3 步骤 2、§8、§10 改动清单加 `store.rs`；M-02 新增 S1（触发器回滚用例）及负向对照 | 0 |
| D1-P02-b | `gate::start` 成功后、`merge.after_started` 写入前崩溃，恢复时作业存在，直接 poll，缺开始事件 | §3 步骤 2、4；§8 | **采纳** | `gate.rs` 先写 `.started` 再起 wrapper，事件 INSERT 不在同一提交内 | §3 步骤 2 先「补开始事件」（本任务无任何 `merge.after_started` 时）再 poll；步骤 4 注明；§8 加一行；M-02 新增 L4 | 0 |
| N1 | M-04 文档核对的 grep 鉴别力不足以证明示例块与条目都改了 | M-04 验收命令与负向对照 | **进 Backlog**（非阻塞，属「验证还可以更严格」） | 不属六类 | Backlog B-07 | 0 |

## 计数与影响

- 新增用例由 20 增至 23（automed lib 7 → 10：+S1、L4、L5），全工作区目标 1196 → **1219**；M-02 的 lib 预期 655（过滤 8）、M-03 的 lib 预期 657（过滤 10），e2e 与 domain/cli 不变。
- 争议项：无（所有主张复提计数为 0）。
