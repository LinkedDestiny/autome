# M-03 实现证据 · 实现 #7

## 范围与修复

仅推进 M-03；上一轮 M-02 审计通过，没有待搬入的缺陷检查。代码仅改 `crates/automed`，不改 schema、暂停/停止语义、其它里程碑或已有测试。

- 会话照常 finish、计量、扫尾、guard、结算表态；Paused/Requeued 中匹配节点的非手动会话结论记为 `session.outcome_held`，暂不推进或安排重试。
- Reenter 区分正在运行、未交付结论、需要重起三种情况。交付先写 `session.outcome_held_applied`，复用正常结束的重试/推进逻辑。出队动作传入的是旧任务快照，因此交付时重新加载 Active 状态，避免跳过崩溃重试。
- 手动复盘启动记录 `session.manual_retro`，排除于原节点会话选择及暂缓；运行中经真实 `task.resume` 派发拒绝恢复。保留终止/失败/停止状态下老 Retro 会话的兼容收尾。

## 验收命令与数量

执行设计契约：

```sh
cargo test -p automed --lib deferred
cargo test -p automed --test end_to_end resumes_at
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

最终结果：deferred 3/3、resumes_at 12/12；工作区 911/911（CLI 8+4、domain 288、automed lib 544、e2e 50、prompt 10、real_cli 7），失败 0、忽略 0；clippy 与 fmt 退出 0。验收串行命令整体退出 0。原始输出只保存在 `.autome/output/m03-{unit,resumes,workspace,clippy,fmt}-final.txt`。

新增 10 个测试：调度 3 个、e2e 7 个，未修改已有测试。上一轮 M-02 基线 901 个（domain 288、automed lib 541、e2e 43、prompt 10、real_cli 7、CLI 8+4）；本轮实测 911 个，差 +10，符合设计最初 880 +31 的总数。过滤集 deferred 从 0 到 3，resumes_at 从 5 到 12；过滤数不重复计入全量。

开发中修正：初次编译暴露测试辅助方法名/响应格式引用错误，已纠正。调度测试先红，指出 R2 使用旧任务快照导致没有重试，修后通过；提前恢复 e2e 曾以“无运行会话”为等待条件，可能错过立即启动的后继会话，改为观察原会话的 ended_at，不弱化角色次数与原节点断言。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计行 | 实际检查与判据 |
|---|---|
| §3 R1 原会话仍在跑 | `paused_running_session_resumes_at_same_session`：恢复后 session id 不变，原会话结束，实现只有一次、Intake 只有一次，下一节点审计。 |
| §3 R2 未交付结论 | `paused_implementation_resumes_at_successor_without_rerun`、`paused_audit_resumes_at_successor_without_rerun`：原角色均仅一次，分别到审计/await_merge；无重复任务整理。 |
| §3 R3 已交付或无结论 | `deferred_crash_is_delivered_only_once_across_retry_pause`：第一次交付进入退避，再暂停恢复必须新建会话、applied 仍为 1；保留 M-02 五条停止恢复 e2e 全绿。 |
| §4 Paused/Requeued 收尾但暂缓 | `deferred_reap_finishes_session_without_advancing` 同一用例遍历两状态：会话结束、状态不变、无 pending retry、held 恰 1；暂停后 tick 无错。槽满 e2e 在 Requeued 时结束会话后留存结论。 |
| §4 暂停跑完再停止 | `paused_then_stopped_resumes_at_held_outcome`：恢复交付一次、不重起实现。 |
| §4 暂停门禁 | `paused_gate_resumes_at_gate_without_restarting_job`：暂停期间等真实包装脚本退出文件，恢复进入审计，gate.started 仍为 1。 |
| §5 同 kind 手动/Loop Retro | `deferred_loop_retro_survives_a_manual_retro_with_the_same_kind`：S1 暂缓，S2 有手动标记；S2 跑时 RPC 拒绝且状态/id 不变，S2 收尾不增加 held；恢复 applied 指向 S1，进入 Rebase。 |
| §5 手动复盘互斥 | `manual_retro_resumes_at_original_node_only_after_finishing`：Implement 暂缓后手动轮运行时拒绝、结束后恢复到审计；加上上行覆盖不同 kind 和相同 kind。 |
| §1 恢复排队拒绝手动轮、§5 Stop 无副作用 | 全量保留 `requeued_rejects_manual_retro`、`requeued_waits_for_slot_then_reenters_and_rejects_stop_without_killing`。 |

### 2. 失败分支

- 崩溃：调度测试以真实非零退出标记产生 Crashed，暂停时不重试，恢复后才进入 retry；再次暂停恢复不会消费同一结论。
- 手动轮互斥拒绝：调度同 kind 与 e2e 不同 kind 均通过派发器检查中文错误，任务不变，原运行会话未被终止。
- 槽满：`paused_queued_session_resumes_at_original_outcome` 用 parallel=1 和真实占槽任务；恢复仍 queued、位置 1、原 session id 不变；结束原会话仍 queued，释放槽后 applied=1 并到审计。
- 拒绝授权、外部服务不可达：不适用，本里程碑未增加授权或网络调用；真实模型依既有 e2e 契约用替身。
- 读盘失败、超时、空输入：本里程碑不新增这些输入/错误判定，沿用 read_outcome/apply_guards 与门禁；原有测试由全量覆盖。新增暂缓交付的失败处理由上面的 Crashed 实例检查，未声称本轮增加了文件权限或真实网络故障注入。

### 3. 全部迁移

本里程碑改变的迁移均已执行：Active→Paused→原会话结束→保持 Paused→Resume→Requeued→出队交付；Active→Paused→Resume→Active→同一会话结束正常推进；Paused→Resume→Requeued（槽满）→会话结束并保持 Requeued→释放槽后交付；Paused 已暂缓→Stopped→Resume→交付；交付 Crashed→退避→Paused→Resume→新会话；Paused Gate→作业结束→Resume→收结果进入 Audit；Paused Retro→手动 S2 运行时拒绝 Resume→S2 结束仍 Paused→恢复消费 S1→Rebase。对应判据见上表。旧停止重起、配置角色关闭及队列控制迁移由原有全量测试保持。

### 4. 输入域边界

- 会话数量：无未交付结论走 R3、恰一条结论消费一次、已消费结论再恢复必须重起，均在调度测试与既有停止 e2e 中执行。
- 会话来源：同一 Retro kind 的 S1/S2 区分，已检查 latest 排除手动轮；非 Retro 的原节点也检查拒绝手动轮期间恢复。
- 槽位边界：parallel=1，满与刚释放两个边界在 e2e 执行。
- 日期、前导零、闰日、跨年、最大数值、用户空串：不适用，本里程碑没有新增上述输入域或解析器。

## 负向对照

逐一临时修改生产代码，执行 `cargo test -p automed --lib deferred`，每次结束都恢复源码；脚本与原始结果在 `.autome/output/m03-mutations.rb`、`m03-negative-*.txt`。

- 关闭 held 分支：测试失败。
- 跳过 finish_session：测试失败。
- 不写 applied 标记：一次交付/Loop Retro 两条测试失败。
- 不写 manual_retro 来源标记：Loop Retro 测试失败。

最终验收在全部注入还原后运行。没有遗留负向注入。

## 边界与后续

H-01～H-03 桌面人工验收未运行，仍留在设计人工清单。宿主 `sh scripts/ci-local.sh` 按任务要求由 Autome 后续运行；本轮不启动下个会话、不推送。
