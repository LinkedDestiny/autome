# M-07 · 实现 #7

## 范围与修复

只推进 B-05：`write_task_inputs` 保留已有卡片，不重写；将卡片路径加入提交集合的操作移到文件存在判断之外。生产调用统一从 `launch_session` 进入，在冻结协议和启动会话之前传播提交错误。附件仍沿用 first_time 条件，无新依赖。上一轮审计 #6 通过，无退回检查需要逐字迁入。

## 红绿验证

- 新增项目测试 `scheduler::tests::task_card_retry` 两例。使用隔离 World 的真实 Git worktree，在其 Git 目录创建 index.lock，确认首次 git add 报错、卡片已写盘、HEAD 未变；解除锁后以 first_time=false 和不同正文重试。
- 修改生产代码前：`cargo test -p automed --lib scheduler::tests::task_card_retry`，总数 2，1 passed / 1 failed。失败点是 `git show HEAD:plans/retry/retry-card.md`：文件存在但不在 HEAD。日志 `.autome/output/m07-red.txt`。这是设计负向对照「只有新建文件才提交」的实跑证据。
- 修复后同一命令：总数 2，2 passed / 0 failed，相比上一轮该过滤器基线 0 增加 2，分别是失败恢复、已提交卡片幂等。日志 `.autome/output/m07-green.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：M-07 两行情形均通过。写盘成功、git add 失败后，重试保留原字节并提交为 `chore(autome): retry inputs`；已提交卡片再次执行时 HEAD 和文件修改时间、字节均不变。§4 的无正文无附件、缺卡补写、附件共提交由既有 task_card 测试覆盖。
2. **失败分支**：真实 index.lock 导致 git add 失败，断言错误包含 index.lock，解除后成功；既有卡片测试还检查目录被普通文件阻塞时报中文创建失败。授权、网络不可达、超时不适用，本方法没有这些分支；空正文由既有创建参数测试拒绝。
3. **全部迁移**：无新增领域状态。文件生命周期逐项覆盖：缺卡→写盘但暂存失败；已有未提交→重试提交；已有已提交→无重复提交；已提交后文件删除→补写恢复（既有测试）。新幂等用例分别执行 first_time=false/true。
4. **域边界**：新幂等用例循环验证 1 字节与 65536 字节（中文+ASCII）的正文；失败恢复用例包含中文、CRLF、空白与占位符，并使用自定义 plans 文档根。None 无正文由既有测试覆盖；空串与超限由创建接口测试覆盖。日期、前导零等数值域不适用，正文按字节保存。

## 全量门禁结果

- `cargo test --workspace`：总数 990，990 passed / 0 failed / 0 ignored；较上一轮实测 988 增加 2，均来自本轮 task_card_retry，automed lib 580→582。其它套件保持 autome-domain 306、end_to_end 59、prompt_rendering 11、real_cli 7、CLI 单元 17、CLI 集成 8。日志 `.autome/output/m07-workspace.txt`。
- `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo run -p automed -- protocol eval protocol/` 均退出 0；日志分别为 `.autome/output/m07-clippy.txt`、`m07-fmt.txt`、`m07-protocol.txt`。协议静态门 8 项通过、0 项不通过、1 项警告。
- 静态警告为 42 行既有强制条文没有 CHANGELOG 理由记录；协议与基线无差异，本轮不改协议。

## Git 基线说明

当前检出起始只跟踪脚手架，源码及任务文档均未跟踪，用户已有 .gitignore 修改；因此直接 `git diff --quiet f20fe14 -- protocol/ apps/ crates/automed/tests/golden/prompts/intake.md` 返回 1，是索引缺少文件，不能作为内容删除证据。

使用独立临时索引验证相同基线，不改变实际暂存区：`GIT_INDEX_FILE="$PWD/.autome/output/m07-baseline.index" git read-tree f20fe14`，然后以同一 GIT_INDEX_FILE 执行 M-05 的两条 diff 检查。受保护目录与金样 diff --quiet 退出 0；四个集成测试的 `git diff -U0 f20fe14` 删除行数为 0，差异日志 `.autome/output/m07-integration.diff`。这与 M-05 的内容判据一致。只提交本轮涉及的四个文件，不加入其它未跟踪内容或用户修改。
