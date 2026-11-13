# M-03 · 实现轮 4/25

## 范围与实现
- 只推进最小开放里程碑 M-03，落实 D6。M-01、M-02 的上一轮审计均通过，没有需要逐字移植的退回检查。
- 需求 MergeGate 的 CoreStepDone Conflict 回 Implement；Bug 同一事件回 Fix，注入原始冲突文件列表。节点不匹配仍拒绝，正常门禁结果仍经 GateDone。
- 调度器把 Rebase 与 MergeGate 冲突计数合用；归一化文件集后连续第三次停止。需求沿用 ImplBudget，Bug 沿用 start_fix 的 FixBudget，并让 Bug 修复动作同样受连续冲突熔断约束。失败位置记录实际冲突节点。
- 出队对齐、授权账本、队列和解锁机制复用 M-01/M-02；本轮用真实 Git 冲突验证它们与新转换的接缝。
- 本地 main 历史核对：T-51 的 bf399dbf 已存在；未发现 T-53 合并记录。本轮不修改 FIFO 排序。

## 验收与回归
- 红：新增两个 domain merge_conflict_ 用例后、生产代码修改前执行 `cargo test -p autome-domain merge_conflict_`，0 通过、2 失败，失败为 Conflict 落入通用 Failed；日志 `.autome/output/m03-domain-red.txt`。
- 绿：同命令 2 通过；日志 `.autome/output/m03-domain-green.txt`。
- `cargo test -p automed --lib merge_conflict_`：2 通过；日志 `.autome/output/m03-lib.txt`。包含真实出队冲突让位、计数 1/2/3、零预算，以及 Bug 文件集归一化、第三次熔断和零修复预算。
- 直接相关回归：`cargo test -p autome-domain rebase_` 13 项；`cargo test -p automed --lib rebase_retry_` 2 项；`cargo test -p automed --lib rebase_conflict_streak_` 1 项，全部通过。日志分别为 `.autome/output/m03-related-domain.txt`、`m03-related-retry.txt`、`m03-related-streak.txt`。
- 最终按契约原样串行执行 `cargo test -p autome-domain merge_conflict_ && cargo test -p automed --lib merge_conflict_ && cargo test -p automed --test end_to_end merge_conflict_`，整链退出码 0；分别 2、2、2 项，**验收用例总数 6，全部通过，0 失败、0 忽略**。日志 `.autome/output/m03-accept-domain.txt`、`m03-accept-lib.txt`、`m03-accept-e2e.txt`。
- 与上一轮 M-02 的 17 项验收基线相比 **-11**，来自本轮改验 M-03 的 6 项，不是删除测试。与本轮起点相同 merge_conflict_ 筛选口径 0 项相比 **+6**，全为新增；比原设计预期 5 项 **+1**，新增的是 Bug 连续冲突与零预算宿主用例，契约同步为 6 项。
- 加上直接相关回归 16 项，本轮最终共 **22 个不同用例通过**；与上一轮记录的 53 个不同用例相比 **-31**：验收范围 17→6（-11），直接相关回归范围 36→16（-20）。没有把重复运行、空筛选测试程序或红阶段累计为额外用例。
- 真实三任务验收：A 占位时 B/C 排队，main 推进后停止 A；B 出队冲突进入实现轮且未启动门禁，C 先开门禁并合并；B 完成真实 rebase 冲突修复、审计和复盘后仅启动一次门禁并自动合并。C 的 gate.started seq 小于 B 修复后的 gate.started seq，B 的 merge.direct 恰好 1 条。
- Bug 验收：出队冲突后启动的是 Bug Fix 会话，注入 README.md，门禁启动数未增加，锁释放、用户授权保留；真实修复和复核后自动合并，merge.direct 恰好 1 条。
- `git diff --check` 通过。没有上一轮审计发现的实现缺陷，故逐字移植审计检查不适用；本轮自建 domain 红/绿已记录。
- 未运行 `sh scripts/ci-local.sh` 或宿主快检组合；本轮仅运行里程碑验收与直接相关子集，并使用 rustfmt 格式化修改文件。

## 标待审前自审清单
1. **情形表逐行**：D6 需求冲突 → Implement、Bug 冲突 → Fix，domain 分别验证注入文件保持原样；D1 冲突分支不写 gate.started、释放锁且下一任务开跑由 lib 真实 Git 用例验证；授权在修复路径和熔断后仍存在。两项端到端闭环均通过，需求用例验证后续任务先运行，Bug 用例验证实际启动 Fix 会话。
2. **失败分支**：需求同文件集第三次 → RebaseConflict；预算 0 → ImplBudget；Bug 第三次 → RebaseConflict、修复预算 0 → FixBudget，均实跑。MergeGate 的 CoreStepDone Ok/Behind 拒绝、Failed/Blocked 保持失败分支，domain 实跑。拒绝授权、超时、外部服务不可达不适用：D6 无新增此类输入或 I/O；本轮不改门禁执行器或授权校验。
3. **全部迁移**：需求 MergeGate+Conflict→Implement；Bug MergeGate+Conflict→Fix；两类熔断/预算耗尽→Failed；授权有/无分别验证；Bug 不匹配的 CoreStepDone 节点拒绝。需求完整链路 Implement→Audit→Retro→Rebase→MergeGate→Merging→MainCheck→Cleanup→Done、Bug Fix→FixGate→FixReview→Rebase→MergeGate→Merging→Cleanup→Done 均由端到端实际驱动通过。
4. **输入域边界**：文件集空、单元素、多元素由 domain 验证；乱序/重复文件集计数归一化由 Bug lib 验证；次数 1、2、3 与预算 0/充足由 lib 验证；更换文件集及重跑重置由既有 rebase_conflict_streak_ 回归验证。日期、闰日、跨年不适用：D6 没有日期输入域。

## 测试夹具修正
- 首次新增 lib 用例引用了不存在的 Git 状态辅助函数；删除该调用，保留真实文件内容、主线 SHA、门禁启动次数和后续任务运行断言。
- 首轮端到端脚本续跑序号被 `step_for` 重置为 1，显式设为 8 后修复；会话列表倒序，改查 running_session，避免误取定位轮。
- 进一步取证发现修复会话 exit 141：模拟 CLI 的 stdout 是事件流，直接输出 Git rebase/merge 进度会触发 SIGPIPE，导致修复脚本未完整写回文档（出现回归测试保护/复核字段错误或复盘重试）。最终脚本将 Git 进度重定向，仍真实执行 rebase、解决 README 冲突、继续 rebase。未修改或绕过产品的回归测试保护、文档校验、预算检查与状态机。
- 原始失败日志保留在 `.autome/output/m03-e2e.txt`、`m03-e2e-green.txt`、`m03-e2e-repair.txt`、`m03-e2e-final.txt`；最终验收使用独立 `m03-accept-*` 日志。
