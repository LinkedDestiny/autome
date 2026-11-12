# M-02 实现 #2 证据

开工 HEAD：`7976648`。本轮只推进 M-02；M-01 审计通过，没有退回的实现缺陷，因此没有必须逐字搬迁的审计检查。实现轮计数为 **2/30**。

## 实现与修复说明

- 新增 `automed::gate_queue`，按同一账本的未结束任务及作业文件现算占用；队列由事件 seq 恢复，严格 FIFO，项目名额受全局封顶。未知占用保守阻止新启动，同时给出可恢复的 tick 信息。
- 在准备命令真正启动前和门禁真正启动前判准入。保留 `setup::ensure` 的返回接口，通过 `ensure_with_hold` 传递本次收取准备作业后的接续资格；会话准备和主检出准备共用同一实现。排队不产生作业文件，不写 started，沿用不计数重试。
- `gate::discard_stale` 复用原有停止/删除逻辑，先清扫旧 head。每 tick 对所有未结束任务回收超时作业；`Ctx.gate_prev_tick` 仅用于已退出未收取作业的占位期限，不保存队列。业务路径恢复后仍收到真实结果。
- 准入之前出错的队首写 `gate.queue_dropped`。Fix 先写 `bug.fix_round_requested`，准备就绪后才写一次 `bug.fix_round_started`，重试和恢复不重复计数。
- 明确设计 §5 的接续边界：除 Active、无 task.updated 外，退出标记还必须未过 §2(b) 的占位期限。否则该名额已释放，不能在收取旧准备结果后免准入。这是“持续占位”的必要条件，已就地补入设计，并由过期准备用例断言。
- 旧用例无删除、无改名、无修改；新用例通过 `include!` 放在独立文件，仍属于 `scheduler::tests`，避免扩写已有大文件中的测试正文。

## 命令与结果

所有原始输出和临时缺陷注入脚本均在 `.autome/output/`，此处仅记录判据。

| 命令 | 结果与数量 | 原始输出 |
|---|---|---|
| 接入后、加入新测试前 `cargo test -p automed --lib` | 655/655，通过；与上一轮基线相同 | `impl2-baseline.txt` |
| `cargo test -p automed --lib scheduler::tests::gate_queue_` | 21/21，通过；其余 655 为过滤，不是跳过 | `impl2-queue.txt` |
| 最终 `cargo test -p automed --lib` | **676 总数，676 通过，0 失败，0 忽略**；29.46 秒 | `impl2-final-lib.txt` |
| `cargo test -p autome-domain gate_queue_` | 6/6，通过；核对事件迁移和纯函数域边界 | `impl2-domain-queue.txt` |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | `impl2-final-clippy.txt` |
| `cargo fmt --all -- --check`；`rustfmt --edition 2024 --check crates/automed/src/scheduler_gate_queue_tests.rs` | 退出 0，无输出（后者显式检查 include 文件） | `impl2-final-fmt.txt` |
| 逐名对比接入前后库测试输出 | 基线 655 → 676，**新增 21、减少 0**，新增均为 `scheduler::tests::gate_queue_*` | `impl2-test-delta.txt` |
| `git diff --check` | 通过 | 无输出 |

总数差 **+21** 全部来自 M-02 的调度器用例；不是已有测试删减、重命名或改变过滤方式造成。全工作区实跑、端到端、CLI/桌面展示由后续里程碑负责，本轮不宣称完成这些验收。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行：已执行

下表用例名前缀统一为 `scheduler::tests::gate_queue_`；每一行均在最终 676 用例运行中通过。

| 设计情形 | 用例后缀 / 检查与结果 |
|---|---|
| 第二门禁等待且不写作业、不扣预算 | `second_gate_waits_without_files_or_budget_changes`：重复请求只一条 queued，无 started、无目录，任务状态/预算/重试数不变；释放后启动 |
| 准备 → 门禁连占名额 | `setup_and_gate_keep_one_slot`：B 已排队，A 收完准备仍直接启动门禁，B 没有 setup.started |
| B 先处理，A 已退出但未收取 | `finished_uncollected_job_blocks_earlier_task`：B 不抢位，A result.seq < B started.seq |
| 升级时已有两个超额作业 | `upgrade_collects_two_old_jobs_without_killing_them`：旧启动窗口仍保留，分别收结果后第三者才启动 |
| 排队暂停/停止、恢复 | `pause_stop_and_resume_rejoin_tail`：task.updated 退队，恢复排到原后继之后 |
| stop_all / discard_node / Vanished | `stop_discard_and_vanished_release_slots`：真实进程停止和文件删除后可准入，旧启动时间且无进程/结果按 Vanished 放行 |
| 暂停作业存活/结束 | `paused_live_job_holds_but_completed_job_does_not`：活作业占位，退出后释放且保留文件 |
| 门禁移除 / reused / skipped / 指纹未变 | `no_process_paths_do_not_enqueue`：各分支不写 queued；原有工作区准备跳过与 verify 失败跳过用例同时保持通过 |
| 会话前准备等待不计 retry | `session_setup_wait_does_not_count_retry`：重复等待 attempts=0，无 session/setup started |
| 主检出互斥与准入 | `main_setup_mutex_precedes_admission`：被互斥挡住不入队，互斥消失即启动；原有 Cleanup 接入测试通过 |
| 项目小名额队首不能被越过；重建上下文 | `project_limit_preserves_fifo_and_restart_order`：全局 3、队首项目 1、后排项目 3；保持同一 store 重建 Ctx 后顺序不变 |
| 排队不打断同因失败计数 | `events_do_not_reset_same_failure_count`：queued 后重读同一结果不增计数；下一次真实 started 才增加 |
| 旧 head 活作业 | `stale_live_head_is_killed_before_admission`：杀真实旧进程、删文件、新 head 启动 |
| 旧 head 已退出未收 | `stale_exited_head_is_deleted_before_admission`：删旧退出文件、不记录旧结果、新 head 启动 |
| 暂停期间准备完成，恢复不能继承名额 | `resumed_completed_setup_requeues_for_all_checkouts`：worktree / verify / main 三种检出，各跑 task.updated 打断和占位期限过期两种情形；另一任务占满时不重启准备，重新排队 |
| 不可推进的队首 | `unreachable_head_drops_queue_and_advances_followers`：移动 worktree 使 head 读取失败，tick 报错且写 dropped，后继同 tick 启动 |
| 暂停任务超时 | `paused_timeout_reclaims_and_preserves_124_for_resume`：真实包装进程回拨 started，回收后直接读取退出标记 124 和终止行；恢复收到同一结果 |
| Active 业务路径不可达、setup 配置不存在 | `active_unreachable_jobs_timeout_without_business_poll`：移动 worktree，tick 独立回收门禁/准备；直接读 124 标记，不用测试里的 poll 偷做回收；下一 tick 名额释放，门禁和准备恢复均照收 124 |
| 已退出遗留作业、setup 被移除 | `old_uncollected_and_removed_setup_jobs_release_but_keep_results`：未过期限阻塞；过期后准入，文件保留；门禁恢复收 9，准备无接续资格 |
| 作业目录读取失败 | `unknown_directory_blocks_only_inflight_ledger_jobs`：另一项目目录消失，有 started 未收尾则阻塞并给恢复提示，无在途证据则不阻塞，目录恢复放行 |
| Bug Fix 真正启动才计轮次 | `fix_round_is_counted_once_after_setup_not_while_waiting`：perform 请求后与多次等待均为 0，准备完成启动变 1；重复重试与 task.updated 后再启动仍为 1 |

### 2. 失败分支：已执行

- 读盘失败：真实移动项目目录（未知占用）与 worktree（head 失败）两条分支均执行，分别断言保守等待、可恢复信息，以及队首退队。缺少作业目录但项目目录可读为合法零占用。
- 超时：暂停与 Active、gate 与 setup 均以真实包装进程和回拨 `.started` 验证 124、终止行、释放名额；无一分钟固定等待。负向对照去掉回收会断言失败。
- 普通准备命令失败、verify 失败跳过 MainCheck：原有 `setup_gate_nodes_wait_then_start_or_fail_and_merge_reuse_skips_preparation` 等测试未改并通过；涵盖 Gate / MergeGate / ReproCheck / FixGate、失败 9、超时 124，MainCheck/主检出沿用既有测试。
- 不可用/移除配置：新增 Active 准备回收用例使用无 setup 的配置，按缺省超时收束；门禁移除不排队。配置无效的既有 tick 隔离用例通过。
- 拒绝授权、外部服务不可达：**不适用**，新增模块没有授权流程或远端调用；只操作现有账本和本地作业。
- 空输入：空队列、没有作业目录、没有在途事件、空 command（移除门禁）分别运行；不是忽略这些分支。

### 3. 全部迁移：已执行

- 队列状态 false/true × 事件：domain 用例逐个执行 queued → true；gate.started、setup.started、session.started、session.deferred、task.updated、queue_dropped → false；gate.result、setup.result、未知/空事件维持原值。
- 作业：无作业 → 等待（无文件）→ 准入 → 存活 → 正常成功/失败/超时 → 收取释放；Vanished → 清除重排；旧 head → 杀/删 → 新 head 准入。新用例及原有门禁/准备用例覆盖每条。
- 生命周期：排队暂停/停止 → 退队 → 恢复队尾；暂停中的活作业继续占位 → 结束或超时释放 → 恢复收同一结果。取消与 rerun 复用原有停止/删除和 task.updated 路径，原有对应用例保持通过。
- 接续：三种检出 × 连续占位/被 task.updated 打断/已过占位期限；连续占位直接接门禁，后两者重新准入。过期文件保留且不伪造通过。
- Fix 请求 → 等准备（计数 0）→ 启动（计数 1）→ 重试/恢复（仍 1）。没有新增 TaskState 或 schema。

### 4. 输入域边界：已执行

- 空队列、单元素、未知任务、乱序 seq、最大 seq、held=0/满额/超额/usize::MAX、limit=1/10：domain 的六个用例通过。
- I/O 边界：无作业目录、项目目录不存在且有/无在途证据；启动窗口无 pid、Vanished 的旧 started；已退出结果在 prev_tick 为空与过期两侧；暂停前后同一准备文件。
- 跨项目值 1 与 3、无配置缺省 1、重建 Ctx 清空 prev_tick：新增调度测试通过。配置字符串/前导零/0/11 的解析属于 M-01，原有库配置测试仍全部通过。
- 闰日/跨年/日期空串：**不适用**，本轮不解析日历输入，使用 SystemTime 与原有 started 秒数。

## 负向对照：10 项均红，全部还原后全绿

`.autome/output/impl2-mutations.rb` 逐项临时改正式源码，运行对应的项目回归用例，每次用 ensure 恢复原文件。每项都编译成功且在断言处失败；编译错误不算判别证据。输出为 `impl2-negative-<名称>.txt`，摘要 `impl2-negative-summary.txt`。

| 名称 | 注入错误 | 捕获用例（同上后缀） |
|---|---|---|
| chain | 丢掉准备接续名额、门禁另排一次 | setup_and_gate_keep_one_slot |
| uncollected | 不计已结束未收取作业 | finished_uncollected_job_blocks_earlier_task |
| paused | 只数 Active 的活作业 | paused_live_job_holds_but_completed_job_does_not |
| stale | 不清扫旧 head | stale_live_head_is_killed_before_admission |
| resume | 接续资格忽略 task.updated | resumed_completed_setup_requeues_for_all_checkouts |
| timeout | 不做全任务超时回收 | active_unreachable_jobs_timeout_without_business_poll |
| expiry | (b) 没有期限 | old_uncollected_and_removed_setup_jobs_release_but_keep_results |
| unknown | 读盘未知当作 0 | unknown_directory_blocks_only_inflight_ledger_jobs |
| fix | perform 提前写 fix_round_started | fix_round_is_counted_once_after_setup_not_while_waiting |
| streak | queued 被视为新运行 | events_do_not_reset_same_failure_count |

还原后的最终库测试 **676/676**，clippy 与格式检查通过。自审完成，M-02 可标待审，由下一审计轮独立复验；未把任何里程碑标为已完成。

## 限制与交接

本轮没有跑全工作区测试、端到端和桌面测试（归 M-05/M-06）；没有取得 H-01 人工证据。没有修改 schema、协议、配置或其它 worktree；`.autome/output/` 仅存命令输出与临时验证脚本。
