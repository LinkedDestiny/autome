# 内核提交会删除大量文件时不拦截，兜底提交收了什么也看不到

status: 已定位
fix-round: 1/3
复现命令: cd crates/automed && cargo test --offline --lib commit_safety -- --nocapture
根因: commit_paths 在暂存后直接提交整个索引，未检查相对 HEAD 的删除数；sweep_one 吞掉提交错误且成功时只记录 SHA，没有新增文件任务事件。
复核: 待复核
repos: crates/automed, crates/autome-cli

## 现象

任务卡为本轮需求依据，无补充信息文件。定位基线为 `71c1c411cc1845ce5918664340d6f3c4cfc61a0b`。

会话正常退出后，兜底提交会把删除 60 个已跟踪文件的快照直接提交。只指定单个输入文件的提交也会夹带索引里已暂存的 60 个删除。新建源文件能够被兜底提交，但任务事件中没有该文件记录。

任务卡记载的 T-13、515 个删除、29 次提交是用户提供的历史资料；本轮未获取该历史分支进行独立核验。本轮使用独立临时仓库验证相同提交缺口，不重演钩子污染当前任务索引。

## 复现

从当前 worktree 根目录执行状态块命令；命令自行进入 Cargo 工作区成员 `crates/automed`。使用现有真实 Git worktree、内存 Store、dry 调度上下文及退出码为 0 的假会话，不调用模型、不访问网络、不依赖 sleep。Git 缺失会显式失败，不跳过测试。

新增四个测试，均以 `commit_safety` 过滤：

1. `sweep_rejects_sixty_deletions_and_fails_task`：基线跟踪 60 个文件，删除后调用真实 `reap_session`；要求 HEAD 不变、任务 Failed、原因包含「本轮会删除 60 个文件」及路径示例。
2. `narrow_commit_rejects_pre_staged_deletions`：先暂存 60 个删除，随后只要求提交 `reply.md`；要求返回错误且 HEAD 不变。捕获 inputs / 补充信息等窄路径调用仍提交整个索引的风险。
3. `sweep_reports_new_source_in_task_events`：验证 `feature.txt` 确实在 HEAD 中，并要求同一任务的持久化事件包含该路径；不预设未来事件名或字段名。
4. `sweep_allows_fifty_and_keeps_new_source`：合法对照，恰好删除 50 个文件应成功，新增 `feature.txt` 内容必须保留。防止把阈值写成大于等于 50，或收窄兜底为仅已跟踪文件。

测试仅增加在 `git.rs` 和 `scheduler.rs` 的现有测试模块，未改产品代码。验证记录：

- 最终夹具连续执行两次，结果一致：3 failed、1 passed、0 ignored，退出码 101；失败来自目标断言，不是编译、超时或命令缺失。
- 窄路径提交实际返回 `Ok(Some(SHA))`；60 文件兜底用例观察到 HEAD 改变；事件用例确认源文件已提交但任务事件不含其路径。
- 日志：`.autome/output/commit-safety-run2.log`、`.autome/output/commit-safety-run3.log`。
- 首次调试日志为 `.autome/output/commit-safety-run1.log`：合法对照额外触发夹具缺少实现证据文件的协议错误，已补齐合法证据后重跑；不以该次结果作为最终复现证据。
- `rustfmt --check` 和 `git diff --check` 通过。未运行全仓库测试或发布门禁；本轮预期保留失败回归测试。

## 根因分析

- `crates/automed/src/git.rs:459`：`commit_paths` 执行 `git add -- <paths>` 后，仅用 `diff --cached --quiet` 判断是否有改动，随即执行 `git commit`。没有相对 HEAD 的删除统计和阈值判断。`paths` 仅限制此次 add，不限制 commit 消费的整个索引，因此不能用窄路径列表证明提交安全。
- `crates/automed/src/scheduler.rs:859`：`sweep_one` 调用 `commit_paths(worktree, ["."], ...)`；错误分支只 `tracing::warn!`，随后返回 `Ok(())`。即使底层增加拒绝逻辑，也需要让任务状态正确失败，不能只打印警告后继续读会话结果。
- 同一函数成功分支仅记录任务 ID 和 SHA，没有统计新增跟踪文件，也没有向 Store 写入事件。`crates/autome-cli/src/main.rs:776` 起通过 `events.since` 获取事件，交给 `render::event_line` 输出；普通 tracing 日志不是这条展示通路。
- inputs、协议固定、简报和补充信息等内核提交也使用同一提交函数。删除防护必须针对最终暂存区快照；新增文件审计则应保留原有兜底范围，不能通过排除未跟踪文件“解决”。

本轮动态覆盖提交函数和会话收尾；未运行真实 `autome run`、未验证新增列表超过 20 条时的截断展示、未逐一实跑 inputs / 补充信息的完整调度入口。上述验收仍应由修复轮完成，不能把本轮测试当作全部产品验收。

## 修复说明

本轮为修复第 1/3 轮，保留原复现命令和定位轮全部回归测试。

- `commit_paths` 在暂存后检查整个索引相对 HEAD 的删除路径，使用 NUL 分隔且关闭重命名折叠；超过 50 个时拒绝提交，错误包含总数与前 5 个路径。恰好 50 个仍允许提交。
- 兜底提交错误向上传递，并在会话收尾将任务标记为 Failed、持久化失败事件；简报提交也不再吞掉错误。inputs、协议和补充信息入口沿用现有错误传播。
- 兜底范围仍为全部未忽略改动。成功后从实际提交读取新增路径，写入 `task.swept` 事件，包含 SHA、仓库、总数及最多 20 个路径；CLI 打印一行，路径中的换行由 JSON 转义。
- 仅追加两个测试，覆盖 21 个新文件的提交与事件截断，以及 CLI 单行展示；未修改、删除或跳过定位轮测试。

验证结果：

- 修复前原命令：3 failed、1 passed，符合定位结果，日志 `.autome/output/fix-before.log`。
- 修复后原命令：5 passed、0 failed、0 ignored（原有 4 项加新增截断测试），日志 `.autome/output/fix-after.log`。
- 最终代码的格式检查、Clippy（`-D warnings`）与 CLI 新增单行展示测试通过，日志 `.autome/output/fix-final-checks.log`；`git diff --check` 通过。逐字比对确认定位轮 4 个测试和共用夹具均未修改。
- 配置门禁 `sh scripts/ci-local.sh` 已完整执行（706 秒），总体未通过。Format、Clippy、工作区测试、独立 End to end（81 项）、Adapter flags、Desktop、Contract、Web install/lint/typecheck/build 均通过；Web test 为 4 个文件失败、29 个文件通过，11 failed、271 passed、19 skipped。
- Web 失败日志包含 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`；当前沙箱禁止监听且不允许提权，未修改或跳过相关测试。需要宿主在允许本机监听和文件监听的环境重跑配置门禁。完整日志 `.autome/output/fix-ci.log`，错误摘要 `.autome/output/fix-ci-errors.log`。
- 未实跑真实模型会话驱动的 `autome run`，未逐一动态注入 inputs / 补充信息入口的删除故障；不将单元测试等同于这些完整入口验收。

## 复核意见

待后续独立复核。
