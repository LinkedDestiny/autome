# M-05 r7 实现轮：项目门禁复现调查

## 结论与范围

本轮为用户指定的门禁修复轮，不推进新里程碑。原有 M-01..M-05 均已由审计关闭，状态原样保留；implementation-round 更新为 7/25。**未定位并修复原始 30 分钟超时，不能报修复通过。**

基线为 `a637303`，与用户给出的失败提交 `59d26775e4` 的差异只有本轮简报；`git diff 59d26775e4 HEAD -- crates scripts apps` 为空。没有修改产品代码、测试断言、跳过条件或门禁命令。保留现有失败，避免用延长超时或缩减验收掩盖问题。

## 原命令与最小复验

- 原样执行 `sh scripts/ci-local.sh`，日志 `.autome/output/r7-gate-red.txt`。此前停住的 CLI 集成测试本次 13/13 通过，耗时 17.81 秒；没有复现原始超时。
- `cargo test -p autome-cli --test cli a_task_goes_from_one_line_to_a_merge_commit_one_command_at_a_time -- --exact --nocapture`：退出码 0，1/1 通过，52.98 秒；日志 `.autome/output/r7-cli-isolated.txt`。这是原输出中唯一没有完成结果的 CLI 测试，沿用现有项目用例原文，未新增替代检查。
- 全量中 `bug_regression_guard_survives_retry_and_extend` 在 `crates/automed/tests/end_to_end.rs:5231` 失败：期望 `TaskState::Failed { reason: FailureReason::FixBudget { .. }, .. }`；101 条集成用例为 100 通过、1 失败，399.81 秒。
- 原样单跑 `cargo test -p automed --test end_to_end bug_regression_guard_survives_retry_and_extend -- --exact --nocapture`：退出码 0，1/1 通过，99.88 秒；日志 `.autome/output/r7-guard-isolated.txt`。期间没有代码修改，因此不是修复后的绿灯，不能当作红绿回归闭环。

## 调查与未证实假设

检查了 CLI `run --until-idle`、会话启动/回收、PID 轮询、Git 子进程等待和提交 hook。对递归 CI、子进程等待及状态未收敛三个假设，均未取得足以确定根因的证据。

只读查看失败运行留下的 `autome-cli-happy-28221` 账本：部分 session 的退出标记时间早于 `started_at`，PID 为空。当前实现先启动 wrapper，再轮询 PID，最后记录 `started_at`；wrapper 退出会删除 PID 文件，说明时间倒置可以由短会话先退出引起，**不能据此证明十几分钟延迟的根因**。原始查询结果在 `.autome/output/r7-old-ledger.txt`、`r7-old-events.txt`。进程列表查询被沙箱拒绝（`ps: operation not permitted`），未能取得宿主进程栈或并发负载证据。没有终止其他会话或清理其他工作树。

## 标待审前的四项自审

1. **情形表逐行**：不适用；本轮不改 W5 功能、不新增或重新标记待审里程碑。门禁原命令及原先未完成的 CLI 用例均已实跑，结果如上。
2. **失败分支**：本轮实际覆盖原门禁执行、Rust 集成断言失败及该用例单独重跑；没有改动设计点名的授权、读盘、超时、空输入、外部服务分支，不声称新增这些分支的验收。
3. **全部迁移**：沿用 CLI 原测试，实跑从创建任务、批准设计、等待合并到完成的链路；不改状态机。其余迁移不适用，本轮没有新增迁移。
4. **输入域边界**：不适用；没有修改输入解析、输入域或边界规则。

上一轮审计为通过，没有发现缺陷的判别检查需要搬迁；本次涉及的两个检查已经在项目 Rust 测试体系中。原超时未复现，不伪造红灯或声称回归修复。

## 后续

由 Autome 在本轮提交上重跑 `sh scripts/ci-local.sh`。若再次卡在 CLI，需在宿主保留当时进程树/等待栈以及对应临时账本，定位实际阻塞点；若重复出现 FixBudget 断言失败，则保留失败时实际状态与会话事件，再按证据修复。没有足够证据前不修改内核行为。本轮不是协议失败。

## 宿主负载证据

2026-11-12 16:58（Asia/Shanghai），`uptime` 连续读到 load averages `137.68 120.57 87.44` 与 `122.62 117.90 86.88`，记录见 `.autome/output/r7-host-load.txt`。这是本轮运行期间的高负载证据，不是对 15:28 原失败原因的证明。进程列表和 CPU 数量查询被沙箱拒绝，未尝试绕过权限。没有启动额外全量来增加负载。

## 全量结果与用例基线差

`sh scripts/ci-local.sh` 最终退出码 **1**，耗时 **790 秒**，未达到用户要求的同命令通过。Format、Clippy、Contract、Web lint、Web typecheck、Web build 通过；Rust test、Desktop、Web test 失败。

- Rust 已执行的套件合计 1171 条（1169 通过、1 失败、1 原有 ignored）；其中 CLI 单元 25、CLI 集成 13、domain 384、domain 回归 1 ignored、automed 单元 647、end_to_end 101。Cargo 在失败套件后停止，不能把该数字称为全部 Rust 测试总数。用户提供的上轮日志只展示 CLI 25+13，不能计算整个 Rust 全量差；CLI 集成总数 13 相对原日志 13 为 **0**。未增加、删除或跳过任何项目用例。
- Web：43 文件 **364 条**，333 通过、12 失败、19 skipped；相对审计 #6 的 364 为 **0**。12 条失败是 gateway-http 的 11 条和 shell-live 的监听 SSR 1 条，报 `listen EPERM: operation not permitted 127.0.0.1`；19 skipped 是文件级前置失败后没有执行，不能算通过，另有 `EMFILE` watcher 错误。
- Desktop：**105 条**，104 通过、1 失败、0 skipped；相对审计 #6 的 165 为 **-60**。Electron DOM harness 子进程退出状态为 null，未取得内部 DOM 断言结果；缺失的 60 条属于该层未展开，不能用 skipped=0 掩盖。没有改动 desktop 测试。
- 原 CLI 用例和偶发 Rust 用例各另跑 1 条，均通过；它们是重复执行，不计新增用例。

状态块字段、`implementation-round: 7/25`、里程碑表与 HEAD 一致、retro 新增行不超过 200 字的脚本检查通过；`git diff --check` 通过。本轮只提交三份文档，原始日志留在 `.autome/output/`。
