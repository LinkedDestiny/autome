# 实现 #4：完成默认分支 rebase 与回归

## 范围与结论

本轮简报与审计一致：M-01、M-02、M-03 均已由审计关闭，没有开放里程碑。本轮只执行用户追加的 rebase 修复，不虚构里程碑、不重开已通过项，也不把任何里程碑由实现轮标成已完成。状态轮次更新为 `4/15`。

- 默认分支：`origin/HEAD -> origin/main`；执行 `git rebase main`，本地 main 为 `502e656b5432b89bc9a3a3669411f48bde69f35b`。未 fetch、未推送。
- 唯一冲突：重放 `81dea34` 时，`crates/automed/tests/end_to_end.rs` 文件尾两侧都新增测试。保留 main 一侧全部内容，补齐末尾函数闭合，再保留本任务的附件改建端到端用例。
- `git add crates/automed/tests/end_to_end.rs` 后 `GIT_EDITOR=true git rebase --continue` 成功完成全部 25 步。重放后 HEAD 为 `d1b298c`。
- 对 main 的该文件差异为 61 行新增、0 行删除，只有改建测试；`git merge-base --is-ancestor main HEAD` 返回 0，无冲突标记。修复已包含在重放后的内核实现提交 `91d3364` 中。
- 上一审计结论是通过，无退回缺陷或需原样搬入的红绿检查。

## 验收与计数

- `cargo test -p automed task_convert`：10/10 通过（9 单元 + 1 附件端到端），与上一轮专项总数 10 相比差 0。
- 宿主 `sh scripts/ci-local.sh`：fmt、Clippy 通过；`cargo test --workspace --all-targets -- --nocapture` 的 9 个目标均打印成功汇总，1221 passed / 0 failed / 1 ignored，总数 1222。脚本随后以 143 结束，未打印整门禁成功，因此不把脚本报告为通过。另跑 `cargo test --workspace --doc` 成功，两个文档目标各 0 例；合计 11 个 Rust 目标。
- Rust 与上一轮完整基线 1207（1206 passed + 1 ignored）相比总数 +15：main 新增 CLI 1、driver_lock 1、scheduler 2、端到端 11；没有删除测试。本轮没有新增产品测试，冲突解决保留双方全部测试。端到端全量 113/113 通过。
- `npm ci --prefix apps/web` 成功；网关专项 36/36，与上一轮差 0。
- `npm test --prefix apps/web` 首次因 EMFILE 后长期无进展主动中止，退出 130；补跑完整汇总为 43 文件、365 例（334 passed / 12 failed / 19 skipped）。比上一轮 33 文件、302 例增加 10 文件、63 例，来自 main 合入的 Web 功能/测试（相对旧 HEAD 的测试变更涉及 16 文件）；本轮未编辑 Web 测试。失败的 12 例均报 `listen EPERM: operation not permitted 127.0.0.1`，19 例因启动钩子失败跳过，另见 EMFILE。不是全绿证据，需宿主补验。
- 单独补跑 Web lint、typecheck、build 均通过，Contract 的文件与 READ_METHODS 检查通过。
- 桌面端 `npm test --prefix apps/desktop`：总计 105，104 passed / 1 failed；失败是 Electron DOM harness 的子进程 `SIGABRT`，无更具体 stderr，不能据此断言产品缺陷或通过。上一审计没有给出桌面端计数，无法比较实跑基线；旧 HEAD 到当前桌面测试无用例增删，仅已有用例内容更新，本轮未修改桌面代码。
- 本轮结论：rebase 完成且改建核心/CLI/Rust 回归通过；完整宿主门禁未通过，Web 本地监听权限与桌面 Electron 启动仍需宿主复验。保持前轮审计关闭状态，不伪造新的待审里程碑。H-01 仍待人工确认。


## 四项自审

1. **情形表逐行**：本轮冲突两侧的内容均完整保留，main 的调度等待与评审范围回归测试无删除，本任务的 `bug_loop_not_a_bug_task_convert_rebuilds_it_as_a_feature_with_its_attachments` 保留；产品情形由现有验收覆盖，结果见上节。
2. **失败分支**：未改动实现。现有 `task_convert` 套件覆盖错误参数、错误状态/任务类型、附件缺失、停会话失败与数据库提交失败；本轮运行结果见上节。不存在新的外部服务或授权失败分支。
3. **全部迁移**：未增加迁移；现有测试覆盖 not_a_bug → cancelled、新需求 draft/queued、非法状态拒绝及重复改建拒绝。本轮冲突解决未删改这些断言。
4. **输入域边界**：网关 36/36 通过，其中改建测试包含未传/true/false、512/513 字符、空串、缺字段和错误类型。日期、闰日、跨年不适用：本接口没有日期输入。

## 原始输出

均存放 `.autome/output/impl4-*`：rebase.txt、rebase-continue.txt、e2e-diff.txt、upstream-rust.diff、ci.txt、npm-ci.txt、gateway.txt、core.txt、web.txt、web-retry.txt、desktop.txt、lint.txt、typecheck.txt、build.txt、doc-tests.txt。不把原始输出复制进任务目录。
