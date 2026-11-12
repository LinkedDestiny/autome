# M-03 · 实现轮 #3

结论：验收通过，申请待审。仅推进 M-03；implementation-round 为 3/40。

## 实现与修复说明

- Bug ReproCheck 保持只跑复现；FixGate 仅在 each_round 下拼项目门禁；MergeGate 始终拼两者。未修改 Bug 状态机、超时取值或沿用条件。
- Job 新增 `.repro-only` 旁路标记 API。Bug FixGate 启动前按本次配置写入或删除；start/clear 保留标记，discard_node 清除标记（包括只有标记的孤儿作业）。其它节点不读写标记。
- 带标记的 FixGate 结束时，gate.result.command 和失败注入 command 均使用已记录的复现命令；未带标记仍按本 tick 配置记账，保留缺省模式 A→B 的旧行为。
- 新增 2 个 e2e、4 个 scheduler 测试、1 个 gate 测试，现有测试未改。真实 Git、worktree、后台 shell 作业与 SQLite；e2e 仅模型使用项目原有替身。
- 测试夹具先固定协议，再启动门禁；同 head 沿用检查先断言结果 SHA 等于当前 SHA，避免首次启动会话固定协议产生提交，让检查错误地靠 SHA 不同而通过。
- 开发过程的编译错误（插入位置、注释换行和整数类型）及夹具重复任务 slug 已修正。最终验收在所有缺陷注入还原及夹具补强后运行。
- 上轮审计 #2 为通过，无退回缺陷，因此没有必须逐字搬运的审计检查；本轮未改上一轮证据。

## 验收命令与用例数量

按契约顺序执行以下三条命令，整体退出码 0：

| 命令 | 用例总数 | 上轮基线 | 差与来源 | 结果 | 原始输出 |
|---|---:|---:|---|---|---|
| `cargo test -p automed --test end_to_end bug_loop` | 16 | 14 | +2，新增 before_merge 完整回路及无效修复用例 | 16 通过，0 失败，0 忽略 | `.autome/output/m03-accept-e2e.txt` |
| `cargo test -p automed --lib scheduler` | 91 | 87 | +4，切换/失败、同 head 不沿用、崩溃事件、旧模式记账 | 91 通过，0 失败，0 忽略 | `.autome/output/m03-accept-scheduler.txt` |
| `cargo test -p automed --lib gate::` | 12 | 11 | +1，标记生命周期与文件系统失败 | 12 通过，0 失败，0 忽略 | `.autome/output/m03-accept-gate.txt` |

合计 119 个不同用例，相比对应子集基线 112 个增加 7 个，没有减少或新增忽略。上轮审计记录全套 automed lib 655、e2e 116；本轮过滤结果显示可发现总数分别为 660（91+569 或 12+648）、118（16+102），与 +5 / +2 一致，不将过滤掉的用例计为已运行。

`cargo fmt --all -- --check` 退出码 0（`.autome/output/m03-fmt-check.txt`）；`git diff --check` 退出码 0。未自行执行全量宿主门禁或全量 clippy，由 Autome 在本轮提交上执行；上述结果不替代宿主门禁。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行

逐项对照设计 §5 的五行，均执行通过：

| 启动 → 结束 | 检查与结果 |
|---|---|
| each_round → each_round，含门禁 A→B | `before_merge_unconfigured_fix_gate_keeps_legacy_current_command_accounting`：缺省和显式 each_round 两组；启动命令含 true，结束记 echo B，启动后和结束后均无标记。 |
| each_round → before_merge | `before_merge_fix_gate_result_cannot_be_reused_on_the_same_head`：无标记，结果记复现命令；同 SHA MergeGate 真启动，不沿用。 |
| before_merge → before_merge | 同上：有标记，结果只记复现；同 SHA 不沿用；e2e 两次 FixGate 都只跑复现。 |
| before_merge → each_round | 切换测试及同 head 测试：标记与 started 同时存在；结果和失败注入均记复现；同 SHA MergeGate 真启动。 |
| 标记与作业已写、gate.started 事件丢失 | `before_merge_fix_gate_crash_window_ignores_missing_and_stale_start_events`：真实启动后删最新 gate.started 事件，再切 each_round；分别覆盖无旧事件、同 node/head 有 each_round 旧事件两种情况，均记复现且同 SHA 不沿用。 |

### 2. 失败分支

- 无效修复：e2e `bug_loop_before_merge_an_ineffective_fix_fails_the_fix_gate` 验证 FixGate false→true，返回修复的 prompt 包含 regression-output 和 2/3。
- 合并前完整门禁失败：e2e `bug_loop_before_merge_fix_gate_runs_only_the_repro_and_merge_gate_runs_both` 验证 MergeGate false→true、修复 prompt 含项目门禁输出，以及完整节点序列。
- 命令失败和退出边界：切换测试循环跑 0、9、126、127、124；验证退出码、ok、FixReview/Fix 转换，以及失败注入仍是复现命令。此处 124 是命令退出码，不冒充真实超时证据。
- 真实超时：本轮 gate:: 验收内 `a_gate_that_runs_too_long_is_killed_and_reported_as_timed_out` 与 `setup_job_past_its_timeout_is_killed_with_124_and_a_setup_note` 通过；超时读取逻辑未变。
- 标记写入/删除失败：将标记路径建成目录，set_repro_only(true/false) 都返回 Err；调度器在启动前传播该错误。标记读接口按设计为存在性布尔查询，无新增独立读盘错误分支。
- 同因失败：scheduler 验收包含原有 same_failure 测试；本次命令分支仍共用 record_gate_result 路径，未改熔断逻辑。
- 拒绝授权、外部服务不可达：不适用，本里程碑没有新增授权或外部服务调用。

### 3. 全部迁移

- 命令模式的四种启动→结束组合及崩溃窗口：见逐行情形表，全覆盖。
- e2e 成功及回退序列逐节点断言：locate→repro_check→fix→fix_gate→fix_review→rebase→merge_gate→fix→fix_gate→fix_review→rebase→merge_gate→await_merge；另一个 e2e 覆盖 FixGate 失败直接返回 Fix。
- 作业标记：不存在→false 删除幂等；false→true→false→true；start 后仍存在；clear 后标记保留且 started 消失；discard_node 清除孤儿标记。上述路径在新增 gate 测试执行。
- 运行作业的丢弃并重启：崩溃窗口旧事件分支真实 discard_node 后重新启动，验证不读取上一次命令。
- 原有 Running/Done/Vanished 与超时：gate:: 12 用例通过，包括 `a_job_with_no_process_and_no_marker_has_vanished`；本次没有修改 start/poll/clear 实现。head 变化分支仍调用同一 clear，其保留标记行为由新增生命周期测试覆盖，未另声称执行过新的 head 变化集成用例。

### 4. 输入域边界

- 配置域：缺省、显式 each_round、before_merge、双向运行中切换、command 为空（等于无项目门禁）均执行；空门禁下 FixGate 只跑复现但不写标记。
- 退出码域：0、9、124、126、127；标记域：有/无、孤儿、路径被目录占用；事件域：缺失/陈旧；SHA 域：相同 SHA 断言后验证不沿用。
- 前导零、闰日、跨年、数值最大值：不适用，无新增日期或数值输入域；既有 timeout 的解析边界未改变。

## 缺陷注入负向对照

每组运行后恢复源码；五组均退出 101，失败原因是断言而非编译失败。执行脚本及简表位于 `.autome/output/m03-mutations.rb`、`.autome/output/m03-mutations-summary.txt`。最终验收覆盖已恢复的实现；后续新增的同 SHA 断言也通过。

| 注入 | 命令 | 红色证据 |
|---|---|---|
| FixGate 忽略 gate_run，恢复旧拼接条件 | `cargo test -p automed --test end_to_end bug_loop_before_merge_fix_gate_runs_only_the_repro_and_merge_gate_runs_both` | 1 失败，不能到 await_merge；`m03-negative-old-command.txt` |
| 结果记账忽略标记 | `cargo test -p automed --lib scheduler::tests::before_merge_fix_gate` | 3 失败，记成完整命令而不是复现；`m03-negative-ignore-marker.txt` |
| clear 清掉标记 | `cargo test -p automed --lib gate::tests::before_merge` | 1 失败，start 后标记不存在；`m03-negative-clear-marker.txt` |
| 所有 Bug FixGate 都写标记 | `cargo test -p automed --lib scheduler::tests::before_merge_unconfigured` | 1 失败，旧模式不应有标记；`m03-negative-all-markers.txt` |
| 无条件从 gate.started 回溯命令 | `cargo test -p automed --lib scheduler::tests::before_merge_unconfigured` | 1 失败，结果记录 A 而非当前 B，破坏旧契约；`m03-negative-event-command.txt` |

表中日志均位于 `.autome/output/`。无阻塞；未启动下一个会话，等待独立审计。
