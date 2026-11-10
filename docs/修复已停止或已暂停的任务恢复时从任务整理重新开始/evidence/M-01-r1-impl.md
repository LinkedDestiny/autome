# M-01 · 实现 #1 · 待审证据

日期：2026-11-10。实现轮：1/15。范围仅为 M-01；没有上一轮审计，也没有需要搬入的审计回归检查。

## 实现与修复

- `TaskState::Requeued { resume }` 经私有 `TaskStateRepr` 序列化为 `{"state":"queued","resume":"…"}`；旧 `Queued` 仍精确输出 `{"state":"queued"}`，schema 保持 4。
- Resume 保留原节点、检查角色开关；停止的三个门禁返回 `DiscardGateRuns`。出队区分 `Reenter` 与内核步骤，排队期间关闭角色进入 Config 失败。
- 账本队列与计数、RPC 队列位置、手动复盘排斥均纳入 Requeued。`task.stop` 在杀会话前拒绝 Requeued。
- Reenter 实现 R1（同 kind 会话仍运行时不重复启动）与 R3（按同 kind 最近会话的 session.started 事件取 inject，再走 redispatch_with）。暂缓结论、挂起输入留存及手动复盘来源标记仍按设计留在 M-02/M-03。
- `discard_node` 先等待缺失的 PID（最多 3 秒）并杀进程组，再移除该节点旧文件，包括 exit.tmp/code；保留其它节点作业。真实 I/O 错误上抛。
- 自审的 I/O 失败用例发现既有 `record_failed_action` 只收 Active，清理失败会留下 Requeued。已使它也记录 Requeued 动作失败为 Failed，避免随后出队收下旧结果。该断言先失败、修复后通过。
- 只修改设计授权的两条既有测试断言：域层 `resume_re_enters_the_queue_rather_than_jumping_the_parallel_limit`、调度层 `pause_and_resume_move_through_queued_rather_than_jumping_the_limit`；测试名和其它既有测试未改。
- `rg -n 'TaskState::Queued' crates` 全量核对：设计列出的四处已处理；另外的域层 Pause 分支由兜底拒绝 Requeued，构造新任务、序列化转换与新任务出队仍专属 Queued。CLI idle 按 B-03 保持原状，无其它遗漏。扫描原始记录：`.autome/output/m01-queued-scan.txt`。

## 命令与结果、基线差

首次复现：新增 `requeued_paused_preserves_the_original_node` 后运行 `cargo test -p autome-domain requeued`，1 个用例、0 通过、1 失败，观察到恢复后 Active Intake 而非 Design。原始记录 `.autome/output/m01-red.txt`。修复后同用例通过并纳入下列 9 个域用例。

验收命令按设计执行：

```sh
cargo test -p autome-domain requeued
cargo test -p automed --lib requeued
cargo test -p autome-domain
cargo test -p automed --lib
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

全部退出 0：

| 检查 | 用例总数 | 基线 | 差额及来源 | 结果/原始输出 |
|---|---:|---:|---|---|
| domain requeued | 9 | 0 | +9，设计列出的域测试 | 全通过；`.autome/output/m01-domain.txt` |
| automed lib requeued | 6 | 0 | +6，账本 2、调度 3、门禁 1 | 全通过；`.autome/output/m01-lib.txt` |
| domain 全量 | 288 | 279 | +9，同上，既有 279 未减少 | 全通过；`.autome/output/m01-domain-full.txt` |
| automed lib 全量 | 540 | 534 | +6，同上，既有 534 未减少 | 全通过；`.autome/output/m01-lib-full.txt` |
| workspace 清单（仅列举） | 895 | 880 | +15 = 域 9 + 宿主 lib 6，其它套件数量不变 | `cargo test --workspace -- --list`；`.autome/output/m01-test-list.txt` |

全量实际执行的两个单元套件合计 828 个，比基线 813 多 15；均 0 失败、0 ignored。按名过滤的 15 个属于这 828 个，不重复计数。workspace 895 只是枚举，不声称执行过本轮尚未要求的 e2e/CLI/prompt 套件。

clippy、fmt 退出 0，记录 `.autome/output/m01-clippy.txt`、`m01-fmt.txt`。宿主总门禁 `sh scripts/ci-local.sh` 由 Autome 后续执行，本轮未运行。

### 负向对照

每次只改变一个生产条件，运行后恢复原文件；所有变异均退出 101，失败来自断言而非编译：

| 变异 | 命令 | 判别结果 |
|---|---|---|
| Resume 改回 Queued | `cargo test -p autome-domain requeued` | 9 个中 3 个失败（Paused、Stopped、停止门禁），其余 6 通过 |
| Requeued 的会话节点动作改 StartIntake | `cargo test -p autome-domain requeued_dispatches_all_thirteen_nodes` | 1 个失败 |
| 去掉 skip_serializing_if | `cargo test -p autome-domain requeued_json_preserves_old_shapes` | 1 个失败，queued 多出 resume:null |
| queued_tasks 排除 Requeued | `cargo test -p automed --lib requeued_waits_for_slot_then_reenters_and_rejects_stop_without_killing` | 1 个失败，队列位置为空（早于腾槽检查即拦截） |
| discard_node 不等 PID | `cargo test -p automed --lib requeued_discard_waits_for_the_wrapper_pid_before_removing_files` | 1 个失败，旧包装进程仍活着；测试清理该进程 |

原始输出 `.autome/output/m01-negative-{resume,intake,json,queue,pid}.txt`，变异执行器及恢复逻辑在 `.autome/output/m01-negative.rb`。变异恢复后重新执行两个 requeued 命令及 fmt，9 + 6 全通过，记录 `m01-final-domain.txt`、`m01-final-lib.txt`、`m01-final-fmt.txt`。没有保留任何生产代码变异。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行

- §1 四行：queued_tasks 同时返回新建/恢复队列并按 created_at 排序，task_counts 将两种都计 queued，slots_in_use 不计恢复队列（账本测试）；queue_position 返回 1（RPC task.get）；start_retro 拒绝恢复队列（调度测试）。全部通过。
- §2 Paused + Resume：13 个可暂停节点逐一保留节点，无动作；`requeued_paused_preserves_the_original_node` 通过。
- §2 Stopped 非门禁 + Resume：10 个节点逐一返回原节点与 None；`requeued_stopped_preserves_the_original_node` 通过。
- §2 Stopped 三门禁 + Resume：逐一返回 DiscardGateRuns；Paused 三门禁对照不清理；`requeued_gate_discard_is_only_for_stopped_tasks` 通过。调度测试还检查真实旧文件删除和 merge_gate 保留。
- §2 已关闭角色 + Resume：六个角色 × Paused/Stopped 均拒绝；`requeued_disabled_role_rejects_resume` 通过。
- §2 Queued + SlotAvailable：仍 Active Intake + StartIntake；新 13 节点映射测试及既有测试通过。
- §2 Requeued 会话节点出队：任务整理及六角色均为原节点 + Reenter；核心六节点为原节点 + RunCoreStep；`requeued_dispatches_all_thirteen_nodes` 通过，且不增加预算、不消费表态。
- §2 排队期间角色被关闭：六个角色均 Failed Config、None；`requeued_disabled_role_fails_at_dequeue` 通过。
- §2 Requeued 的 Resume/Pause/Stop/RerunFrom/ExtendBudget 均拒绝，Cancel 返回 Cancelled + RunCancel；13 个节点全部检查，通过。
- §3 本轮 R1：暂停恢复后沿用原运行会话，session 数不增加，Stop 拒绝不改变原 running session。R3：无历史会话时新起 Impl；有历史时带回该 session 的 GateFailed inject，另一个 session.started 事件不会覆盖它。均在调度槽位测试中通过。R2 本轮不适用：设计明确归 M-03。

### 2. 失败分支

- 角色关闭的恢复拒绝与排队失败：六角色均执行，见域测试。
- 读盘失败：将门禁作业目录替换为普通文件，Resume 报错且记录 Failed；调度门禁测试通过（先红后绿，修复说明见上）。
- PID 超时：仅有 started 而没有 pid/exit 的作业，实测等待至少 3 秒后删除；门禁测试通过。
- PID 尚未写入：真实进程被控制文件阻塞，100ms 后放行，清理等待它写 PID 后杀进程、删文件，新作业只得到 fresh 输出；门禁测试通过。
- 空输入/未知节点 JSON：空字符串、unknown、数字 resume 均反序列化失败；域 JSON 测试通过。
- 拒绝授权、外部服务不可达：不适用，本里程碑无授权或外部服务接口；角色配置拒绝已单独验证。SAME-MODEL 沿用 resolve_config/redispatch_with，既有全量配置校验测试通过。

### 3. 全部迁移

在 13 个合法恢复节点上检查 Paused→Requeued→Active；Stopped→Requeued（10 普通 + 3 门禁）；Queued→Active Intake；Requeued 出队到 7 个会话节点、6 个内核节点；六角色关闭时 Resume 拒绝与出队 Failed；Requeued × 五个被拒控制事件和 Cancel→Cancelled。覆盖 §2 每行，均通过。

调度侧检查槽满保持 Requeued 且无会话，腾槽后 Active Implement，已有会话的重入不重复启动，门禁出队仅记录 Active Gate 而不启动角色会话；Stop 拒绝前不杀会话。失败动作额外检查 Requeued→Failed。

### 4. 输入域边界

- 节点域为 15 个枚举扣除两个等待点，映射测试显式计数为 13，覆盖首尾 Intake/Cleanup 及全部六角色、三门禁。
- JSON：无 resume 的旧 queued、显式 null、合法 audit、空串、未知字符串、数字；同时断言全部旧状态形状与往返，通过。
- 账本：直接写旧 queued/paused/stopped JSON 后加载、新 Requeued 行写回再加载；混合队列顺序、单一槽位和空项目队列，通过。
- 文件域：指定 gate 前缀、保留 merge_gate、包括 exit.tmp/code、无目录时无操作、无 PID 的超时和真实延迟 PID，通过。
- 日期、前导零、闰日、跨年、数值最大值：不适用，本里程碑没有新增日期或数值解析输入域。

## 结论与后续

M-01 验收及四项自审通过，标为待审；只由审计轮决定是否关闭。M-02/M-03 保持开放，暂停已结束会话的暂缓交付与完整 e2e 尚未实现，不宣称整个任务完成。人工验收 H-01～H-03 本轮未执行。
