# 实现 #8：合并前门禁字段契约兼容修复

## 范围与定位

本轮按追加指令修复提交 `2b7669a45a10` 的宿主门禁失败，不推进新里程碑。全部里程碑原已由审计关闭，原状态保留；implementation-round 为 8/25。本证据归档 M-04（D7 读路径字段与 R5 消费者兼容）。上一轮审计无退回检查，失败检查本来就在项目测试体系内，无需另搬临时脚本。

读取宿主原始日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-70/merge_gate-2b7669a45a10.log`，失败摘录存 `.autome/output/r8-host-failures.txt`：

- Rust end_to_end：174 例，173 通过、1 失败。`bug_loop_task_get_reports_the_fix_round_for_bugs_only` 的严格键集合缺少 D7 新增的 `merge`。
- Web：535 例，533 通过、2 失败，均在 `notifier-backfill-real.test.ts`。首次启动的 dashboard 替身缺少 D2/D7 的 `config_warnings`；该断言提前退出，没执行 notifier.pull，下一例取得空通知基线，重启补发后便发生连带失败。

## 修复与红绿记录

生产返回值符合设计，不删除字段、不放宽严格键集合、不修改通知器。Rust 基线加入 merge，并对 feature/bug 草稿均断言完整默认 merge 对象；Web 共享 dashboardDouble 加入 config_warnings 空数组，真实账本断言正常配置下警告为空。通知数量、游标、去重和消息断言保留。

修改前原样执行已有检查：

- `cargo test -p automed --test end_to_end bug_loop_task_get_reports_the_fix_round_for_bugs_only`：1 例，0 通过、1 失败；与宿主相同的键集合断言。`.autome/output/r8-rust-red.txt`。cargo 报测试失败（101）；本次包装命令末尾 tail 的返回码为 0，不作为测试通过证据。
- `npm run test --prefix apps/web -- test/notifier-backfill-real.test.ts`：4 例，2 通过、2 失败；两项失败与宿主一致。`.autome/output/r8-web-red.txt`。本次包装命令末尾 tail 的返回码为 0，不作为测试通过证据。

修改后执行（包装命令显式保留测试退出码）：

| 命令 | 结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed --test end_to_end bug_loop_task_get_reports_the_fix_round_for_bugs_only` | 1 例通过，退出 0 | `.autome/output/r8-rust-green.txt` |
| `cargo test -p automed --lib merge_view_` | 3 例通过，退出 0 | `.autome/output/r8-merge-view.txt` |
| `npm run test --prefix apps/web -- test/notifier-backfill-real.test.ts test/notifier.test.ts test/notifier-real.test.ts` | 23 例通过，退出 0 | `.autome/output/r8-web-green.txt` |

## 标待审前的自审清单

本轮不更改里程碑状态，仍按四项执行直接相关检查：

1. **情形表逐行**：D7 feature/bug 均携带 merge，草稿默认未授权、不持锁、不等待、不过期；AwaitMerge 落后显示计数，MergeGate/Implement/Merging 不显示 stale；取消终态 merge 为 null；单仓库/多仓库计数与降级；dashboard 持锁、等待、项目授权和无门禁警告。Rust 上述 4 例覆盖。Web 正常配置警告空数组和严格字段集合由真实账本首例覆盖。
2. **失败分支**：merge_view_ 覆盖分支或成员仓库缺失时 stale 为 null；Rust 原有 wrong-state reply 拒绝与无事件断言保留。notifier.test.ts 覆盖游标写失败回滚、离线/超时保留游标后恢复、带 code 拒绝、首次补发失败/关闭。新授权拒绝、门禁冲突与合并锁释放本轮未修改，不适用本轮字段基线修复，未声称重跑这些全量分支。
3. **全部迁移**：无生产状态机改动。受影响通知路径检查首次空库→补发、重启同状态→不重发、离开→再进入→新增、旧库→升级播种、换账本→重置补发、关库期间新增失败→恢复补齐；merge_view_ 检查授权 user/behind/撤销及终态字段。完整合并队列迁移本轮不适用（仅测试契约修复）。
4. **域边界**：feature/bug 两种类型、fix_round 只存在于 bug 且初值 0、无授权字段 false/null、无警告空数组、空账本、单个等待任务、相同状态不同 seq、多仓库不同默认分支的落后计数。日期/前导零/数值最大值不适用：没有对应解析或输入域改动。

## 用例总数与基线差

本轮绿色验收共 27 个不同用例（Rust 1 + 3，Web 23），0 失败、0 跳过，无重复计数。宿主失败对应的同口径子集为 5 例（Rust 1 + backfill 4），红轮 2 通过/3 失败，绿轮 5 通过，总数差 0。另执行 22 个既有相关用例（merge_view_ 3、通知单测 17、真实通知集成 2）。没有新增或删除测试函数。

上一实现轮为 192 个不同用例，本轮为 27，差 -165，源于按用户要求改为字段契约与通知器相关子集，而非上一轮的 rebase/merge/快检/费用/格式兼容范围；不能把该差解释为删除或跳过测试。与上轮交集为 merge_view_ 的 3 例，本轮另选 24 例，上轮其它 189 例本轮未跑。

## 范围限制


未跑全量门禁、项目快检或全量 Rust/Web 测试；最终快检与全量门禁由 Autome 对提交重跑。人工 H-01/H-02 本轮未取证。没有生产代码、协议、配置或其它 worktree 改动，不启动下一会话、不推送。
