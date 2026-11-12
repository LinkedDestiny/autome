# 审计角色关闭时 Bug Loop 静默跳过复核

status: 已定位
fix-round: 1/3
复现命令: cd crates/automed && cargo test --offline --test end_to_end bug_review_policy -- --nocapture
根因: Bug Loop 在审计关闭时将 FixGate 成功直接转到 Rebase；启动入口没有 Bug 专用审计必需检查，通用 SAME-MODEL 校验又豁免关闭的角色，且跳过分支不记录告警。
复核: 待复核

## 现象

任务卡为第一输入，无 replies 补充文件。定位基线为 `1642aaa`。

关闭项目审计角色后，直接创建 Bug，以及先创建草稿再关闭审计并启动，均可完成定位、复现检查、修复和修复门禁，最终到达 `await_merge`。账本仅有 Locate、Fix 两个会话，没有 FixReview，也没有 `bug.review_skipped` 事件。

这与 `docs/plans/2026-11-10-web-dashboard-via-loop.md:96` 的 §4.2 流程及第 124 行的独立复核要求不符。当前任务协议同样要求复核必经，因此判定为 Bug。

需要区分：并非开启审计后的模型相等校验失效；本次新增 Bug 专用测试确认，同一 `claude:shared-model` 配置下，`task.start` 拒绝启动，草稿状态保留且没有运行会话。实际缺陷是关闭角色后整个复核被绕开。

## 复现

从本 worktree 根目录执行状态块命令。此项目为单 Git 仓库中的 Cargo workspace，`crates/automed` 是包目录，不是 Autome 多仓库项目成员，因此不填写多仓库 `repos`。

新增测试位于 `crates/automed/tests/end_to_end.rs`，复用既有 World、假模型 CLI、真实 Git 仓库、RPC 分派、调度器、账本与门禁。无模型请求或网络访问；Cargo 使用离线缓存。调度使用既有有界条件轮询，不以固定等待时长作为断言。

| 测试 | 实跑结果 |
| --- | --- |
| `bug_review_policy_disabled_audit_must_not_silently_skip_review` | 失败：直接创建后无复核、无告警却到达待合并 |
| `bug_review_policy_disabling_audit_before_start_must_not_silently_skip_review` | 失败：草稿创建后关闭审计再启动，同样静默跳过 |
| `bug_review_policy_same_model_is_refused_for_bug_start` | 通过：开启审计且修复/复核同模型时拒绝启动 |
| 既有 `bug_loop_a_bug_runs_from_creation_to_a_merge_commit` | 通过：正常配置完整经过复核并合并 |

最终回归的失败断言为 `Bug reached await_merge without fix_review or bug.review_skipped`，两条路径分别标记 `draft=false`、`draft=true`。退出码 101 来自测试断言，不是超时、命令缺失或编译错误。会话返回顺序不作为断言依据。

日志保存在 `.autome/output/bug-review-policy-final.log`、`.autome/output/bug-review-policy-final-repeat.log`；正常完整流程日志在 `.autome/output/bug-review-positive.log`。不提交临时日志。未运行全仓门禁或真实模型会话。

测试保留任务卡允许的两种处理：启动入口以包含“审计”“启用”的提示拒绝，或者继续执行但必须记录 `bug.review_skipped`。后者的 `autome status`/`run` 告警显示尚未覆盖，若后续选择告警方案，必须另补展示验收，不能只凭本测试通过宣布完成。

## 根因分析

1. `crates/autome-domain/src/bug_loop.rs:294`：FixGate 通过后，审计开启时启动 FixReview，关闭时直接 `core(Node::Rebase)`；后者没有告警动作。公共 `session` 的角色关闭检查也被这个分支绕开。
2. `crates/autome-domain/src/config.rs:577`：SAME-MODEL 只在审计与实现均 enabled 时比较身份。这是通用角色配置的既有规则，不应直接认定其对需求任务的豁免有错；Bug 必经复核约束缺少单独落实。
3. `crates/automed/src/dispatch.rs:1431` 的创建路径和第 1514 行的启动路径没有 Bug 专用审计必需检查。`crates/automed/src/scheduler.rs:3663` 解析配置使用上述通用校验，因而无法阻止关闭审计的 Bug 执行。
4. `crates/autome-domain/src/session.rs:38` 将 Locate/Fix 映射到 Impl、FixReview 映射到 Audit。开启审计时 SAME-MODEL 配对有效，但关闭后复核节点根本不执行，独立复核保证随之落空。

后续修复建议采用任务卡倾向的启动拒绝方案，同时考虑草稿创建后配置变化。仅改通用 SAME-MODEL 比较条件并不能确保复核实际发生。本轮仅提交回归测试与定位记录，不修改产品代码，不创建需求设计或里程碑文档。

## 修复说明

第 1/3 轮采用启动拒绝方案：直接创建并启动 Bug 时，在持久化任务前检查项目与全局合并配置；审计关闭则提示“Bug 任务需要独立复核，请启用审计角色后启动”。草稿仍允许保存，在 Start 和 SlotAvailable 转移时重新检查当前审计开关，覆盖草稿创建后以及排队期间的配置变化。Feature 任务规则不变。

FixGate 成功后统一进入 FixReview，不再按审计开关跳到 Rebase；运行途中关闭审计会通过既有角色检查停在 FixReview 配置失败状态。同步修正原有领域单元测试中允许跳过复核的预期，明确断言配置失败且没有启动动作。定位轮的 `end_to_end.rs` 回归测试未修改、删除、跳过或弱化。

验证：原复现命令修复前为 1 通过、2 失败，修复后为 3 通过、0 失败。日志分别为 `.autome/output/fix-1-before.log` 和 `.autome/output/fix-1-after.log`。

配置门禁 `sh scripts/ci-local.sh` 已完整运行，耗时 1026 秒，退出码 1，未全绿。Format、Clippy、Rust 全仓测试（含 106 条端到端测试）、Contract、Web lint/typecheck/build 均通过；Rust 另有 1 条既有 ignored。Desktop 为 104 通过、1 失败：`test/dom.test.js:30` 的 Electron DOM harness 返回空退出码，stderr 为空，未进一步定位原因。Web 为 271 通过、11 失败、19 跳过，4 个测试文件失败；日志记录本地监听 `listen EPERM: operation not permitted 127.0.0.1`、文件监听 `EMFILE` 以及 server 钩子超时，部分用例因初始化失败未执行，不能视为通过。未改动或弱化这些检查，需宿主在具备本地监听与 Electron 运行条件的环境重新验收。完整日志为 `.autome/output/fix-1-gate.log`。

`git diff --check` 通过。本轮不启动后续会话，不推送、不合并，独立复核仍待 Autome 调度。

## 复核意见

待独立复核。
