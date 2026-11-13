# 实现 #18：全部里程碑关闭后的 rebase 集成修复

## 范围与结论

简报、设计表和审计 #16 一致：M-01—M-16 均已完成，没有开放里程碑，也没有审计退回缺陷。本轮执行用户追加的 rebase 指令，不重开或改写已完成里程碑及其最新证据指针。implementation-round 更新为 18/80，next-action 指向独立集成复验。

`git rebase main` 已完成，基线 main 为 fcabc5e1；`git merge-base --is-ancestor main HEAD` 返回 0。没有 merge、fetch、推送、切换工作目录或启动下一会话。原始日志位于 `.autome/output/r18-rebase*.txt`。

## 冲突解决

- `crates/automed/tests/end_to_end.rs`：保留 main 的 kernel guard/adopt 测试与分支的 scope_restore_bug 测试，补回各自函数闭合边界。
- `crates/autome-cli/tests/cli.rs`：保留 main 构建标识、旧内核拒绝与接管测试，以及分支 operator_note 测试和独立 test 属性。
- `apps/web/test/write-gate.test.ts`：保留 project.confirm/remove/env 的新增测试及 task.note 的输入校验，specific 字典保留双方条目。
- `docs/development/technical-design.md`：保留 main 的账本推进约束和分支的重/轻车道准备规则。
- `crates/automed/src/store.rs`：main 的 v8 创建 kernel_mark；Onboarding 重建 sessions 顺延为 v9，SCHEMA_VERSION 和断言同步。这符合 X-6 的版本占用顺延规则，未修改真实账本。设计现行说明与技术文档同步为 schema 9。

## 验收与用例数

均在当前 worktree、项目原有临时账本测试体系运行；以下通过命令退出码均为 0。

| 命令 | 结果 | 原始日志 |
|---|---|---|
| `cargo test -p automed --test end_to_end -- kernel_ scope_restore_bug_` | 13 通过 | `.autome/output/r18-e2e.txt` |
| `cargo test -p autome-cli --test cli -- kernel_ operator_note_` | 10 通过 | `.autome/output/r18-cli.txt` |
| `npm run test --prefix apps/web -- test/write-gate.test.ts` | 54 通过 | `.autome/output/r18-web.txt` |
| `cargo test -p automed onboarding` | 19 通过（库 17、e2e 1、金样 1） | `.autome/output/r18-onboarding.txt` |
| `cargo test -p automed --lib onboarding_session_migration` | 2 通过，已包含在上述 19 个内 | `.autome/output/r18-migration.txt` |

主验收共 **96 个唯一用例，96 通过、0 失败、0 忽略**。rebase 前 ORIG_HEAD 相同范围为 73 个（3 + 1 + 51 + 18）；差 **+23**，其中 main 带入 kernel e2e 10、kernel CLI 9、写网关 3，本轮新增 v8 升级回归 1。上一审计轮仅执行 M-16 的 8 个协议哈希用例，与本轮集成范围不同，不把 96−8 当作新增测试数。迁移单跑和负向对照复跑不重复计入唯一用例。

新增 `onboarding_session_migration_from_v8_preserves_kernel_mark`：真实 v8 sessions 布局升级，检查旧会话字段、kernel_mark、schema 9、外键和 Onboarding 归属约束；原 v7 升级测试复用同组断言。

负向对照原样命令：`cargo test -p automed --lib onboarding_session_migration_from_v8_preserves_kernel_mark`。
- 红：临时将 Onboarding 迁移条件从 `< 9` 退回 `< 8`，退出 101，1 失败，错误为 `no such column: project_id`；日志 `.autome/output/r18-migration-red.txt`。
- 绿：恢复正确条件后同命令退出 0，1 通过；日志 `.autome/output/r18-migration-green.txt`。故障注入已还原。
- 上轮审计通过，无需搬入缺陷判别检查；本轮新增检查已进入项目测试体系。

## 标待审前的四项自审（本轮为集成复验，不改变里程碑状态）

1. **情形表逐行**：冲突涉及的两侧功能均保留并运行：旧内核只读/拒绝/显式接管/恢复；Bug 复核首次越界退修、预算耗尽、连续越界熔断；note 原文与草稿状态；project/env/note 网关校验；Onboarding 单仓库/工作区完成/跳过及会话隔离。对应上述命令均通过。
2. **失败分支**：kernel 写拒绝、非法 note 参数、Onboarding 索引锁提交失败保持状态及重试、缺失/未知项目与非法 task_id 更新均通过；迁移漏跑负向对照红→绿。超时、外部服务不可达不适用：本轮不改这些分支。
3. **全部迁移**：本轮改变的数据库路径为 v7→v8→v9、v8→v9，均实际检查保留旧会话与 kernel_mark；当前 schema 打开由各临时 Store 用例覆盖。Onboarding 完成/跳过/失败留原态/重试由 19 个验收用例覆盖。
4. **域边界**：迁移版本边界 7、8、9；NULL task_id 只准 Onboarding、缺失/未知 project_id 拒绝；写网关空值、类型错误、8000 字符与超限边界通过。日期/闰日不适用：本轮无日期输入域变化。

仅格式化修改的 Rust 文件；`git diff --check` 工作区通过。全量门禁和宿主快检按用户要求未运行，由宿主执行。跨 ORIG_HEAD 的 diff-check 仅报 main 带入的两个任务协议 contract.toml 末尾空行，未改动这些外部任务文档。H-01 人工模型验收与既有 D-1 eval 缺失仍按原审计记录处理。
