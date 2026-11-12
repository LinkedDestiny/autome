# M-05 · 实现 #7 · 合并前门禁修复

本轮为修复轮，implementation-round: 7/25；不推进新里程碑，M-01–M-05 保留审计已完成状态，不重新关闭或标待审。完整门禁结论由 Autome 合并前重跑决定。

## 失败、根因与修复

宿主提交 `2b8f011910` 的 `merge_gate-2b8f011910*.log` 显示 Rust CLI 集成测试 23 个中 22 通过、1 失败：`run_json_tick_error_and_interrupt_have_no_idle_trailer`。失败在 cli.rs 原第 1016 行，首条 JSON error 是 `ses-…: …TOML parse error`，测试却断言包含任务 ID。摘要存 `.autome/output/r7-host-failures.txt`。

原测试 `task new` 自动启动会话，然后直接把任务状态改成 Gate 并破坏配置。scheduler.tick 先 reap_session，后 run_core_step；fake 会话何时结束决定首条错误来自会话还是任务。单跑第一次通过、随后原样重复第 1 次即失败，和宿主症状一致。独立临时目录按 PID 隔离；报错发生在错误来源断言，SIGINT 退出与 stderr 检查已经通过，排除目录串扰与信号退出是本次根因。

修复仅改测试夹具：`task new --draft`，断言不存在运行会话，再设置 Gate。首条错误加强为任务 ID 前缀且含 TOML parse error；退出成功、stderr 为空、SIGINT 有界等待、所有后续行不得有 idle 的原断言保留，首行也明确不得有 idle。未放宽接受任意会话错误，未更改产品调度或输出。设计 §13 逐条列明本次既有测试修改例外。

检查同类范围：CLI 集成测试内 `first`、`recv_timeout`、`tick must emit`、`broken project`，仅本例将自动启动的任务强制改为 Gate 后假设第一条错误来源；范围检索见 `r7-scope.txt`。上一审计 #6 无退回判别脚本，本次失败检查本身已在项目测试体系，无需迁入外部测试。

## 命令与红绿证据

原始输出均在 `.autome/output/`。

- 原样 `cargo test -p autome-cli --test cli run_json_tick_error_and_interrupt_have_no_idle_trailer`：首次 1 通过（`r7-red.txt`，文件名不代表结果）；原样重跑第 1 次退出 101，1 失败（`r7-repro-1.txt`），首条 error 为 ses 前缀，复现宿主失败。
- 修后 `cargo test -p autome-cli --test cli run_json_`：退出 0，3 个用例通过、0 失败、0 忽略、20 筛选外（`r7-green.txt`）。覆盖 JSON 事件流与正常 idle 尾行、断管/驱动锁跳过的静默输出、错误后 SIGINT 无 idle 尾行。
- 修后原失败命令独立执行 12 次：12/12 通过，每次 1 个用例（`r7-repeat-1.txt` 至 `r7-repeat-12.txt`）。重复运行不计为新增用例。
- `cargo fmt --all -- --check`：退出 0（`r7-fmt.txt`）。`git diff --check`：退出 0。

本轮最终子集 **3 个不同用例**，相对上一实现轮 45 个是 **-42**：换出协议、调度、金样 45 个，换入 CLI JSON 3 个，范围不同，不是删除测试。同范围修前修后仍为 3 个，净增减 **0**；额外 12 次重复只是稳定性验证。宿主失败所在 CLI 集成套件共 23 个，本轮筛选外 20 个未运行；不把旧门禁其它步骤通过或本轮子集通过当作完整门禁通过。

## 标待审之前的自审清单

本轮不标待审，仍逐项执行。

1. **情形表逐行**：本次未修改设计 §7 的配置切换表、§10 快检标签行为，均不适用本次夹具修复。相关 CLI JSON 情形逐项实跑：正常事件逐行 JSON 且最终 idle；驱动锁跳过不输出；断管静默退出；配置错误输出任务错误后 SIGINT 退出且无 idle，全部通过。
2. **失败分支**：非法 TOML 触发真实 Gate core-step 失败并验证错误内容；闭合读端触发断管；驱动锁竞争触发跳过，均通过。授权、读盘失败、外部服务不可达、门禁超时不适用：本轮没有改变对应分支。
3. **全部迁移**：本例草稿 → 测试直接设置 Active(Gate) → tick 报错 → SIGINT 结束驱动；正常 `--until-idle` 进入 await_design_approval 并打印 idle 作为正对照。全部经过真实 CLI 子进程验证；没有新增产品状态迁移，测试设置状态不是产品转移验收。
4. **域边界**：空 stdout/stderr（锁跳过与断管）、第一条错误与后续零至多条输出、有限 10 秒首行等待与 5 秒中断退出界限，相关用例通过。前导零、日期、最大整数不适用，本次未改这些输入域。

## 边界

未运行 `sh scripts/ci-local.sh`、项目快检门禁命令或全量 Rust 测试；未修改 .autome 配置与规则，仅按要求在 output 保存日志和临时脚本。未切换分支、操作其它 worktree 或推送。人工验收状态不变。
