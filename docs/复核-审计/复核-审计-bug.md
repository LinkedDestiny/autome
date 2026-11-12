# 复核、审计、复盘轮缺少本任务文档目录范围检查

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --test end_to_end review_scope_ -- --test-threads=1
根因: reap_session 未校验会话提交的全量文件范围；Bug 分支绕过 apply_guards，需求守卫只检查本任务设计文档，复核字段报错又缺少目标文档路径。
复核: 待复核

## 现象

任务卡所述错误包含两个独立问题：复核轮修改其他任务文档时没有范围拦截；本任务复核字段未更新时，只报「复核 字段不符合当前轮次」，没有提示具体应更新的文档。若同时把本任务字段写成「通过」，越界改动可能直接进入待合并状态。需求审计轮、复盘轮同样缺少目录范围约束。

本轮通过当前代码的真实调度器、Git 提交、worktree、会话退出标记和状态转换复现；仅模型 CLI 使用仓库既有模拟器。未访问历史 T-33 会话，任务卡中的历史提交与会话编号不作为本轮实测证据。

## 复现

本项目为单 Git 仓库的 Cargo workspace，不是 Autome 多仓库工作区；从当前 worktree 根目录执行状态块命令，无需 repos 或切换成员仓库。

测试位置：`crates/automed/tests/end_to_end.rs`，前缀 `review_scope_`。测试在隔离临时仓库运行，不启动本任务的下一轮会话、不修改真实其他任务目录。

1. 在测试仓库主分支预置并提交 `docs/other-task/other-task-bug.md`，模拟已合并的其他任务。
2. 正常完成定位、宿主复现检查、修复及门禁；定位和修复会改 `product.txt`，验证这些轮次允许改代码。
3. 复核只改其他任务文档并提交，断言协议失败原因含「只能修改」、本任务目录及越界路径。
4. 单独测试同时正确更新本任务复核字段并夹带越界改动，防止修复仅改善字段报错而仍放行越界提交。
5. 对照：仅更新本任务复核字段应进入 `await_merge`；仅不更新字段应报「本轮没有更新」及准确 Bug 文档路径。
6. 需求任务分别在审计、复盘会话提交另一个任务文档的改动，要求同样拦截。

原始测试输出保存在 `.autome/output/review-scope-test.log`，不纳入提交。

本轮验证记录（2026-11-12，产品基线 `1b98426`）：离线编译成功，首跑退出码 101；6 个用例中 1 通过、5 失败、0 忽略。合法对照进入 `await_merge`；错误任务文档与未更新字段两例实际报「复核 字段不符合当前轮次」；其余三例实际状态为 `active` 而非期望的 `failed`。失败来自行为断言，不是超时、编译或命令错误。第二次执行同一命令仍为退出码 101、1 通过及 5 失败，日志见 `.autome/output/review-scope-repeat.log`。`rustfmt --check` 与 `git diff --check` 均通过。

## 根因分析

- `crates/automed/src/scheduler.rs:301-340`：`reap_session` 先兜底提交，Bug 会话随后直接 `read_bug_outcome` / `deliver_bug_outcome` 并返回，没有检查会话开始至结束提交之间的文件清单，也不经过需求的 `apply_guards`。
- `crates/automed/src/scheduler.rs:530`、`:706`：需求守卫构造的输入没有全量改动路径；`design_changed_outside_milestones` 的 Git diff 显式用 `-- <本任务设计文档>` 限定范围，其他任务文档和产品代码均不可见。
- `crates/automed/src/guards.rs:331`：`audit_edited_the_design` 是警告，约束的是设计文档正文与里程碑表，不是本任务目录边界；复盘也没有目录范围检查。不能将这个守卫等同于已经存在的越界拦截。
- `crates/autome-domain/src/bug_loop.rs:211-226`：复核字段为待复核时使用固定字符串构造协议失败。此处缺少目标文档路径与实际提交路径信息，因而只能报告次生字段问题。
- `reap_session` 的手动/终态复盘分支还有提前返回路径（`:272-286`），后续修复须明确覆盖；本轮新增测试覆盖自动复盘，未验证手动复盘或多仓库路径行为。

后续修复应在读取/接纳本轮结果之前，按会话开始的提交基线检查复核、审计、复盘的全量路径，包含重命名的原路径与新路径，先报告越界清单；合法路径按目录边界比较。定位、修复不受此限制。此处为修复边界说明，本轮未实现产品改动。

## 修复说明

修复第 1/3 轮：

- 在复核、审计、复盘会话启动前记录各工作仓库的 HEAD；兜底提交后、读取结果及手动/终态复盘提前返回之前，统一检查相对该基线的全部变更路径。仅文档仓库的本任务目录允许写入，越界时报告允许目录和实际路径，不接纳本轮结果。缺失基线时明确拒绝检查通过。
- Git diff 使用 NUL 分隔并关闭重命名识别，检查重命名两端；目录前缀带末尾斜线，避免同名前缀目录被放行。定位、修复不受此守卫限制。
- 本任务复核字段仍为待复核时，错误包含「本轮没有更新」和准确 Bug 文档路径；越界错误优先于字段错误。
- 原定位轮端到端回归测试未改动。另在调度器测试中补充目录内重命名合法对照、跨目录重命名、相似目录前缀以及手动复盘未提交产品修改的拒绝用例。
- 原复现命令修复前为 1 通过、5 失败；修复后为 6 通过、0 失败、0 忽略。日志分别为 `.autome/output/fix-1-before.log`、`.autome/output/fix-1-after.log`。
- 新增边界测试 `cargo test --offline -p automed --lib session_scope_ -- --test-threads=1`：2 通过、0 失败、0 忽略，日志 `.autome/output/fix-1-boundaries.log`。`cargo fmt --all -- --check`、`git diff --check` 通过。
- 已执行配置门禁 `sh scripts/ci-local.sh`，815 秒后退出码 1，**整体门禁未通过**。Format、Clippy、Rust 全套测试（含 650 个 automed 单元测试、112 个端到端测试）、Contract、Web lint/typecheck/build 通过。日志 `.autome/output/fix-1-gate.log`。
- Desktop 为 104 通过、1 失败、0 跳过：真实 Electron DOM harness 退出状态为 null。单独重跑同样失败；直接启动探针记录 `signal: SIGABRT`、stdout/stderr 为空。该测试不启动 Rust 内核，桌面源码未改动；具体启动崩溃原因未查明。日志 `.autome/output/fix-1-desktop-recheck.log`、`.autome/output/fix-1-electron-probe.log`。
- Web test 为 271 通过、11 失败、19 跳过（测试初始化失败导致），4 个测试文件失败。错误包括 `listen EPERM: operation not permitted 127.0.0.1` 与 `EMFILE: too many open files, watch`；当前执行环境无法完成这些验收，未修改或跳过测试。需由 Autome 宿主在允许本地监听、Electron 启动的环境重跑完整门禁。
- 多仓库范围限制已按文档仓库身份与逐仓库基线实现，但本轮未新增多仓库越界专项验收；不能以既有多仓库流程通过代替该专项验证。

## 复核意见

待复核。后续复核需保留失败用例及合法对照，不以删除、跳过或弱化断言换取通过。
