# M-04 · 实现 #4

结论：验收通过，提交待审。上一轮 M-03 审计通过，没有退回缺陷或必须原样移植的缺陷检查。

## 实现与验收

仅在 `crates/automed/tests/end_to_end.rs` 新增共享准备函数及 E4、E5；复用 Workspace、真实 Git/worktree/包装脚本与 RPC，模型使用现有替身。M-02 的生产实现已满足本里程碑，无须改动生产代码或旧测试。

- `cargo test -p automed --test end_to_end > .autome/output/m04-e2e.txt 2>&1`：退出 0，55 个用例全部通过，0 失败、0 忽略。上一轮审计基线 53，本轮 +2，分别是 E4 `a_workspace_with_one_member_behind_rebases_every_member_and_merges_them_all` 和 E5 `a_workspace_merge_left_half_done_finishes_without_a_second_merge_commit`；没有删除、改写既有用例。
- `cargo fmt --all -- --check > .autome/output/m04-fmt-check.txt 2>&1`、`git diff --check`：均退出 0。
- 全 workspace / clippy 收口按设计留在 M-05；本轮没有声称执行宿主门禁或桌面人工验收。

## 标待审之前的自审清单

1. **情形表逐行**：方案 §3 工作区与部分合并四行均核对。①只因落后被拒：E4 只有 backend main 前进，一次 RPC 合并完成；两仓均出现在 merge.rebased，所有 merge.repo 都在其后，且均有 commit。②非落后拒绝允许部分合并：E5 docs 未跟踪的同路径用户文档阻挡合并，backend 已落地、任务回 await_merge、用户文档字节不变、无 merge.rebased。阶段二竞态导致 Behind 的共用自动路径由全量中的 E2 检查；不声称新增了多仓竞态注入。③所有成员 rebase：E4 事件包含两仓 head，均为最终 main 祖先；E5 已合并 backend 在自动路径中可继续。④已合并仓跳过：E5 backend main SHA 前后相同，两仓各只有一个精确匹配任务合并消息的 merge commit，docs 文档和推进 main 的文件均保留。
2. **失败分支**：E5 实跑 docs 冲突脏文件拒绝，验证返回 await_merge 且不改用户文件；清除阻挡后让 docs 落后，第二次按合并自动恢复并完成。全量保留执行 M-03 自动 rebase 后门禁失败回实现，以及原有脏工作树阻挡用例。拒绝授权、读盘失败、外部服务不可达、空输入、超时不是 M-04 设计新增或点名的失败分支，不适用；本轮不新增这些行为。
3. **全部迁移**：M-04 涉及的两条工作区路径均实跑：E4 await_merge → merging → 排队/rebase → merging → cleanup → done；E5 第一次 await_merge → merging → await_merge，第二次 → merging → 排队/rebase → merging → cleanup → done。E4 断言自动 rebase 后没有再次 await_merge；E5 断言第一阶段确实等待、第二阶段 done。无门禁路径为本轮两例配置；带门禁成功/失败分支由本次全量 E1/E3 覆盖。全域暂停/停止等转换不属于本里程碑新增状态机，本轮未单跑 domain。
4. **输入域边界**：E4 两个参与仓中恰好一个落后，一个未落后；E5 恰好一个已并入、一个被拒，再从拒绝转为落后。这是本里程碑最小多仓与部分合并边界。零落后由现有工作区正常合并用例覆盖，单仓由全量既有自动合并用例覆盖。日期、数值、前导零等输入域不适用，M-04 未引入此类解析。

## 负向对照与恢复

将 `scheduler.rs` 唯一的 `if !behind.is_empty() && !other_block {` 临时替换为 `if false && !behind.is_empty() && !other_block {`，禁用阶段一提前返回，阶段二真实执行 Git 合并。

- 命令：`cargo test -p automed --test end_to_end a_workspace_with_one_member_behind_rebases_every_member_and_merges_them_all > .autome/output/m04-e4-negative.txt 2>&1`。
- 红：退出 101，1 个用例失败，54 个被过滤。merge.repo 数为 3 而非 2（落后拒绝、docs 提前合并、backend 后续合并），被 E4 截获。
- 原样恢复 scheduler.rs 后，同命令输出到 `.autome/output/m04-e4-restored.txt`：退出 0，1 个通过，54 个过滤。`git diff` 确认生产代码无残留变异。
- 此处红绿是本轮主动验证鉴别力，不是上一轮退回缺陷。E5 移除已合并跳过的真实 Git 变异及 B-03 结论严格留给 M-06，不在本轮提前标完成，也不声称该变异必然红。
