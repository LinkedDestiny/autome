# M-04 · 实现 #4

## 范围与修复

仅推进最小开放里程碑 M-04（设计 §9、任务卡第 5 条、约束 9）。上一轮审计 M-03 通过，没有退回缺陷，也没有需逐字移植的审计失败检查。

- domain 新增 `kernel_timeout(tail) -> Option<u32>`，仅识别最后一个非空行的内核门禁终止标记；返回正整数分钟数。导出 `TIMEOUT_SUMMARY`，原指纹材料与摘要不变。
- 接入设计 §9 九处文案；CLI status 与事件共用 `gate_verdict`。普通失败原文、serde 形状、调度状态、预算与准备命令均不变。
- 保留设计 B-03 兼容例外：熔断摘要和 ReproDone 的原因码仍按任意 124 分类；文案层（熔断摘要之外）按内核终止行判断。
- 新增 12 个 `gate_timeout_` 用例，旧用例未删、未改名、未改断言。

## 验收与总数

原始输出均在 `.autome/output/`，本文件仅记录判据与结论。

| 命令 | 总数及相对上一轮差 | 结果与日志 |
|---|---|---|
| `cargo test -p autome-domain` | 397，上一轮 394，+3 判据用例 | 396 通过、1 原有忽略；`m04-accept.txt` |
| `cargo test -p automed --lib` | 684，上一轮 679，+5（真实 poll 1、注入 2、警报及简报 2） | 684 通过；`m04-accept.txt` |
| `cargo test -p autome-cli` | 46，上一轮 42，+4（事件、status、熔断、复现提示） | 32 单测 +14 集成全部通过；`m04-cli-prompt.txt` |
| `cargo test -p automed --test prompt_rendering` | 16，上一轮 16，差 0 | 16 通过；`m04-cli-prompt.txt` |

合计 1143 个，上一轮基线 1131，差 +12 全部来自上述新增用例，未减少旧用例。domain 的 397 包含原有忽略用例，并非 397 个实际通过；与 M-01/M-02 证据计数口径一致。

开发中红灯记录：`m04-target.txt` 首次新注入夹具缺少 Bug 预算，4/5 通过；补齐夹具预算。`m04-accept.txt` 新增 CLI 事件用例失败（31/32），定位为接线误落 setup.result，恢复 setup.result 并正确接入 gate.result；`m04-cli-prompt.txt` 32/32 通过。未放宽断言。

## 标待审之前的四项自审

1. **情形表逐行**（设计 §9 九处）：
   - 1/2 普通 Loop / Bug Loop 的 GateFailed：`gate_timeout_both_loop_injections_keep_ordinary_failure`，终止行显示门禁超时、提示先看卡住步骤；普通失败与自带 124 保留「没有通过」，tail 保留。
   - 3 MainCheck 警报：`gate_timeout_main_alert_note` 验证警报拼接的超时片段和普通失败空片段；源码核对 run_gate 的 main_gate_failed detail 使用此函数并保留退出码、head、命令、日志。
   - 4 审计简报：`gate_timeout_audit_line` 验证超时及普通 124 两支，保留日志路径。
   - 5 门禁事件：`gate_timeout_result_event` 验证内核超时与普通 124 两支。
   - 6 status 门禁行：`gate_timeout_status_verdict` 验证共享函数的通过、普通失败、超时三支；源码核对 main.rs 调用该函数。
   - 7 熔断 FailureReason：`gate_timeout_same_failure` 精确断言超时摘要的输出；原有普通同因失败用例不变且通过。
   - 8 ReproNotFailing：`gate_timeout_repro_injection_requires_reason_and_kernel_line` 验证原因码 × 终止行组合；仅 timed_out 且有终止行时写不能据此断言未复现。
   - 9 bug.repro_missed：`gate_timeout_repro_event` 验证相同判据、普通未复现原文与 tail 保留。
2. **失败分支**：真实 `poll_as` 超时杀进程并返回 124，tail 被 domain 识别（`gate_timeout_real_poll_tail_matches_domain`）；普通测试失败、外部 timeout 124、空 tail、准备命令终止行、标记后还有输出均跑负例。拒绝授权、读盘失败、外部服务不可达不适用：本轮只改纯文本判定与展示，不新增这些 I/O 分支。
3. **全部迁移**：不适用状态迁移，本轮不改状态机、事件形状或迁移规则。展示分支已执行通过/普通失败/内核超时，普通/Bug 两种注入通道，以及 why × tail 组合；既有 domain 与 automed 状态机测试全部通过。
4. **输入域边界**：已跑空串、单行、前置普通日志、末尾换行、分钟 1/30/u32::MAX、前导零 01；拒绝 0、负数、加号、小数、空分钟与 u32 溢出；拒绝非门禁终止行。闰日/跨年不适用：输入不包含日期。

## 负向对照

- 临时使判据将含退出码 124 的普通输出当成超时：`cargo test -p autome-domain gate_timeout_exit_124_alone` 失败（1 个）；输出 `m04-negative-code.txt`。
- 临时把真实 poll_as 的终止行「运行超过」改为「耗时超过」：`cargo test -p automed --lib gate_timeout_real_poll` 失败（1 个）；输出 `m04-negative-line.txt`。
- 两处变异已原样还原。三个 crate 的 `gate_timeout_` 过滤复跑：3 + 5 + 4 = 12 个全部通过，输出 `m04-green.txt`。测试确实能拦住仅凭 124 判超时和 producer/parser 文案失配。

本轮不宣称完成 M-05 的端到端队列验收、M-06 的全工作区/桌面验收或 H-01 人工验收；宿主 ci-local 由 Autome 后续调度。

最终整条验收命令 `cargo test -p autome-domain && cargo test -p automed --lib && cargo test -p autome-cli && cargo test -p automed --test prompt_rendering && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check` 退出 0，日志 `m04-final.txt`；总数仍为 1143（1142 通过、1 原有忽略），clippy/fmt 通过。`git diff --check` 通过。四项自审完成后，将 M-04 标为待审，implementation-round = 4/30。
