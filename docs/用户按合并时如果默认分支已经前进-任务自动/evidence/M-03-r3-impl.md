# M-03 · 实现 #3

## 范围与结论

最小开放里程碑为 M-03；仅新增 S6、E3 两个项目测试，前两轮的产品实现已满足本里程碑，无需修改生产逻辑或已有测试。上轮审计结论为 M-02 通过，无退回缺陷，因此没有需要逐字搬入的审计缺陷检查。实现轮预算为 3/30。

## 命令与结果

- `cargo test -p automed --lib > .autome/output/m03-lib.txt 2>&1`：555 个用例全部通过，失败 0、忽略 0；较上一轮 554 增加 1，即 S6。
- `cargo test -p automed --test end_to_end > .autome/output/m03-e2e.txt 2>&1`：53 个用例全部通过，失败 0、忽略 0；较上一轮 52 增加 1，即 E3。
- 两套合计 608 个，上一轮 606 个，净增 2，无删减。S6 使用真实 Git/rebase，E3 使用现有真实包装脚本、Git/worktree、调度器及 RPC，只有模型进程是替身。
- 首次单跑：`cargo test -p automed --lib a_conflicting_requested_rebase`、`cargo test -p automed --test end_to_end a_gate_failing_after`，各 1 个通过；日志分别为 `.autome/output/m03-s6.txt`、`m03-e3.txt`。

- `cargo clippy -p automed --all-targets -- -D warnings`、`cargo fmt --all -- --check` 与 `git diff --check` 均通过；日志为 `.autome/output/m03-clippy.txt`、`m03-fmt-check.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：M-03 契约的 S6（授权 rebase 冲突）通过：从真实落后合并进入排队，取得槽位后 rebase 产生 README.md 冲突，退回 Implement；session.started 注入严格等于 RebaseConflict{files:[README.md]}，最后 merge.auto.active=false，没有 merge.rebased，主分支 SHA 不变，任务文件恢复到冲突前内容。E3（自动 rebase 后门禁失败）通过：A 仅新增 from-a、B 仅新增 from-b，各自初始门禁通过；A 合并后 B 单次按合并，重放到 A 之上，组合门禁失败；失败事件 SHA 严格等于 merge.rebased.head，随后真实实现替身删除 from-b 并提交，恢复到 await_merge，B 没有 merge_commit，主分支保持 A 合并后的 SHA。
2. **失败分支**：设计点名的冲突与门禁失败分别由 S6/E3 实跑；S6 同时验证 rebase abort 后文件恢复。一般 Git 操作失败（空 ref、缺失仓目录）由全量中的 S1 检查返回 Failed 且主分支不变。拒绝授权、远端服务不可达、网络超时不适用：本里程碑只运行本地 Git 与本地门禁，无外部服务或新授权接口；没有新增门禁超时规则。
3. **全部迁移**：S6 实跑 AwaitMerge→Merging→Requeued(Rebase)→Active(Rebase)→Implement，并检查真授权清为假。E3 实跑 AwaitMerge→Merging→Rebase→MergeGate(失败)→Implement→Gate→Audit→Retro→Rebase→MergeGate→AwaitMerge；失败后实现事件及后续两个成功门禁均有顺序断言，B 的替身执行历史严格为 1…10（第 8 轮修复、第 9 轮审计、第 10 轮复盘），没有继续合并。成功自动路径及无授权普通路径由全量既有 S2/S3/E1 覆盖；本轮没有新增状态或通道。
4. **输入域边界**：本里程碑没有数值、日期、字符串格式的新输入域，前导零、闰日、最大值不适用。组合门禁的两个单元素正向对照（仅 from-a、仅 from-b）及组合失败、修复后再次单元素通过由 E3 覆盖；单冲突文件边界由 S6 精确断言 files 的单元素数组覆盖。空 ref 与缺失仓路径由 S1 覆盖。没有用无改动或跳过门禁代替组合失败。

## 负向对照与恢复

所有变异仅临时修改 `crates/autome-domain/src/task.rs`，原版备份为 `.autome/output/m03-task-original.rs`，每次测试后用该文件恢复；最终该文件无 diff。

- 变异一：把授权延续表达式 `(requested && in_merge_stretch(&transition.next))` 替换为 `requested`。运行 `cargo test -p automed --lib a_conflicting_requested_rebase > .autome/output/m03-mut-auth.txt 2>&1`，退出 101，1 个失败：实际 merge.auto.active=true，期望 false。
- 变异二：把 MergeGate 失败分支的 `Ok(back_to_impl(head, command, tail))` 替换为 `Ok(Transition::active(Node::AwaitMerge, Action::None))`（仅用 let 消除参数未使用告警）。运行 `cargo test -p automed --test end_to_end a_gate_failing_after > .autome/output/m03-mut-gate.txt 2>&1`，退出 101，1 个失败：失败门禁之后不存在 implement 事件，修复路径断言拦住该错误。
- 还原后分别以相同测试过滤器复跑，S6 1 个通过、E3 1 个通过；日志 `.autome/output/m03-s6-restored.txt`、`.autome/output/m03-e3-restored.txt`。这些是主动鉴别力检查，不是声称原版存在缺陷。

## 交付边界

本轮仅推进 M-03，标待审，其他里程碑状态不改。未运行 workspace 全量和宿主 `scripts/ci-local.sh`；后者由 Autome 调度。未执行桌面人工验收。原始输出和临时脚本仅保存在 `.autome/output/`。
