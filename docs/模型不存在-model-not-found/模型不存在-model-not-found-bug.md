# 模型不存在被当成崩溃重试：定位记录

status: 已定位
fix-round: 1/3
复现命令: cd crates/automed && cargo test --offline --test end_to_end model_not_found_fails_as_config_without_retry -- --exact --nocapture
根因: 非零退出被转换为 Crashed；classify_failure 仅识别限流，model_not_found 落入普通崩溃重试，耗尽后转为 SessionCrashed，未保留模型错误上下文。
复核: 待复核
repos: autome-v2

## 现象

在当前提交 7d9294c 上，Codex 会话输出含 `model_not_found` 的 `turn.failed` 并退出 1 后，实际记录两次 `session.retry`，最终状态为 `Failed { at: Design, reason: SessionCrashed { detail: "会话退出码 1" } }`。

预期第一次模型错误即进入 `Failed{Config}`，不重试，错误包含角色、CLI、模型以及 `~/.autome/config.toml` 的 `roles.<角色>.model` 修改位置。任务卡所述历史模型下线及 T-8/T-9 事件未独立核实。

## 复现

本项目为单 Git 仓库，Cargo 成员为 `crates/automed`；上述命令从当前 worktree 根目录执行，自行进入成员目录。需要本地 Rust、Git 和已缓存的 Cargo 依赖；`--offline` 禁止依赖下载，不使用真实供应商、账号或模型接口。

新增 `crates/automed/tests/end_to_end.rs` 中的 `model_not_found_fails_as_config_without_retry`：复用真实任务创建、调度器、wrapper、退出标记、存储及临时 Git worktree，只替换 CLI。任务整理成功后进入 plan 角色，配置 Codex 模型 `missing-model-regression`，连续准备三份相同错误输出并退出 1。重试次数设为 2、退避为 0，避免依赖实际退避时间或缺失 fake step 才失败。

输入为按任务卡构造的合成 fixture：

```json
{"type":"turn.failed","error":{"message":"model_not_found: The model `missing-model-regression` does not exist"}}
```

测试断言零重试、只运行整理和第一次 plan 会话、最终 `Failed{Config}`，并检查错误详情包含 `plan`、`codex`、模型名及配置路径与键。未使用忽略或条件跳过。

连续两次运行均以退出码 101 在同一业务断言失败：`left: 2, right: 0`，同时输出上述实际终态；不是超时、命令缺失或编译失败。日志为 `.autome/output/model-not-found-run1.log` 和 `.autome/output/model-not-found-run2.log`。

普通崩溃重试对照使用现有 `bug_loop_a_crashed_fix_session_is_retried_without_counting_a_round`，实跑 1 个用例通过；结果见 `.autome/output/crash-control.log`。`cargo fmt --all -- --check` 和 `git diff --check` 均通过。格式检查日志见 `.autome/output/fmt-check.log`。

## 根因分析

1. `crates/automed/src/scheduler.rs` 的 `read_outcome` 对非正常退出直接生成 `SessionOutcome::Crashed`，详情只含退出码；`deliver_outcome` 将其交给 `schedule_retry`。
2. `schedule_retry` 确实读取会话 JSONL，排除“根本未读取错误流”的假设。`crates/automed/src/usage.rs:63` 的 `classify_failure` 可提取 Codex `turn.failed.error.message`，但返回类型只有 `Option<Transient>`，匹配词全部为限流、配额或过载，没有模型配置错误类别。
3. 因而错误返回 `None`，进入 `attempts < policy.attempts` 的普通重试分支；耗尽后由任务状态机将 `Crashed` 转为 `FailureReason::SessionCrashed`。没有模型配置错误分流，自然也没有角色和修改位置的诊断。
4. `crates/autome-domain/src/config.rs:511` 的配置校验检查本地范围、技能可见性及角色模型身份冲突，不查询供应商模型可用性。任务卡提及 doctor 的现象不意味着本轮需要新增联网预检；本轮确认的是运行时错误归类缺失。

本轮仅增加回归测试和本记录，未修改产品代码。Claude 对应真实错误样本及 Codex 原始 JSONL 未提供，未伪称已采集；后续覆盖真实格式需要脱敏样本（含 CLI 版本、错误事件和退出码）。这不阻断当前 Codex 合成输入对既有调度错误的稳定复现。

## 修复说明

修复第 1/3 轮：复用原有错误事件提取逻辑，识别错误内容中的 `model_not_found`。在需求与 Bug 共用的重试入口优先将任务记为 `Failed{Config}`，不计入重试、不触发限流冷却；详情保留供应商错误、角色、实际 CLI 和模型，并指向 `~/.autome/config.toml` 的 `roles.<角色>.model`。内置系统会话单独提示检查 CLI 模型可用性，不指向不存在的角色配置。

定位轮的回归测试及复现命令未修改。新增分类单测覆盖错误事件识别、普通输出不误判、限流分类保持不变。

修复前原命令再次失败（重试次数断言失败，日志 `.autome/output/fix1-before.log`）；修复后原命令通过（1 passed，日志 `.autome/output/fix1-after.log`），最终代码再次运行同一命令也通过（1 passed，日志 `.autome/output/fix1-final-repro.log`）。最终 Format、Clippy 和 `git diff --check` 通过。

已完整执行配置的项目门禁 `sh scripts/ci-local.sh`，退出 1，日志 `.autome/output/fix1-gate.log`。Format、Clippy、Rust Test、End to end、Adapter flags、Desktop、Contract、Web install/lint/typecheck/build 均通过；普通崩溃重试对照亦通过。Web test 为 19 个文件通过、3 个失败，198 个测试通过、11 个失败、16 个跳过：测试启动服务器遇到 `listen EPERM: operation not permitted 127.0.0.1`，另有 `EMFILE: too many open files, watch` 及启动等待超时。未修改或跳过测试来规避当前执行环境限制，不能认定项目门禁全绿，需由宿主在允许本地监听的环境重跑。未运行可选 package 构建或真实供应商模型调用。

## 复核意见

待独立复核。
