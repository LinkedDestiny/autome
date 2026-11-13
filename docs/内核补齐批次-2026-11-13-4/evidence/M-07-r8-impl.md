# M-07 · 实现轮 8

## 范围与修复

普通定位、修测试定位及无快照的 rerun fix 共用 `scheduler/test_fix.rs::regression_set` 和最早定位基线。累计候选与上一快照取并集；基线不是 HEAD 祖先时沿用 merge-base 回退。普通定位继续只保护新增测试块，保留产品辅助代码的修改权限。rerun fix 在会话启动前生成 `source: rerun_fix` 快照，空集或文档读盘失败落为 `Failed(Fix, Protocol)`，不启动修复会话。同步技术设计 §5.6。

上轮审计 M-06 通过，没有要求本轮移植的缺陷判别检查。回归测试先以 `dace47e4` 提交，随后修正夹具：AwaitInfo 不支持直接 rerun，改为显式注入 Failed(Locate, Protocol) 的恢复起点；定位产物仍由真实定位会话产生，rerun 走真实 dispatcher。rebase 情形以真实 Git commit-tree/reset 替换祖先链，保留首次定位文件。

## 命令与结果

- 修复前验收：`cargo test -p automed --test end_to_end -- bug_regression_base_ bug_regression_guard_ bug_cd_`：退出 101，18 条中既有 13 通过、新增 5 失败；两个 rerun 夹具最初被 AwaitInfo 拒绝，不将此作为产品缺陷证据。输出 `.autome/output/m07-red.txt`。
- 修正夹具后，将两个产品源文件临时恢复为 HEAD 原实现，再执行 `cargo test -p automed --test end_to_end -- bug_regression_base_`：退出 101，6 条全部失败，12.62 秒；定位路径触发空保护集，rerun fix 缺少启动前快照，空输入被错误放行。此轮断言立即失败，不依赖超时。输出 `.autome/output/m07-red-confirmed.txt`。原实现注入已还原。
- 修复后里程碑验收：`cargo test -p automed --test end_to_end -- bug_regression_base_ bug_regression_guard_ bug_cd_`：退出 0，19/19 通过，0 忽略。输出 `.autome/output/m07-final.txt`。
- 共享路径：`cargo test -p automed --test end_to_end -- test_fix_e2e_`：退出 0，10/10 通过，0 忽略。输出 `.autome/output/m07-shared.txt`。
- 最后将无快照/空集断言移到启动返回处，使错误实现立即失败；复验六条边界用例退出 0、6/6 通过，见 `.autome/output/m07-final-boundaries.txt`。
- 运行 `cargo fmt --all` 格式化；未运行宿主快检或 `scripts/ci-local.sh`。

用例总数：本轮唯一用例 29（里程碑 19 + 共享路径 10），重复红绿执行不重复计数。同口径基线 23（既有 guard 8 + cd 5 + test_fix_e2e 10），增加 6：设计指定五条及空集/读盘失败合并的一条边界用例。设计原预期 18，本轮实际 19，差 1 来自新增边界用例。上一轮 M-06 报 8 条，本轮较其多 21 条是里程碑范围变化，不能当成同一套测试覆盖增量。

## 标待审之前的四项自审

1. **情形表逐行**：
   - 首次无法复现且没有快照 → reply 后只改文档：`second_locate_without_test_changes` 保留首次测试，修复轮篡改被拒。
   - 首次命令通过 → 第二次只改复现命令：`command_only_relocate` 保留首次快照，篡改被拒。
   - 定位失败恢复 → rerun locate：`rerun_locate_after_failure` 保留首次测试，篡改被拒；失败状态由夹具明确注入。
   - 原基线不再是祖先 → merge-base：`rebased_branch_falls_back_to_merge_base` 仍进入修复并拒绝测试篡改。
   - 无快照 → rerun fix：`rerun_fix_without_snapshot_computes_one` 断言启动前已有快照、source 正确、修复篡改被拒。
2. **失败分支**：空保护集、Bug 文档缺失读盘失败均在 `rerun_fix_rejects_empty_or_unreadable_baseline` 检查 Failed(Fix, Protocol) 且启动次数为 0。已有 `bug_cd_repro_empty_protection...` 保持定位空集先复现再拒绝；guard 用例覆盖删除、重命名、权限改变与内容篡改。授权、网络不可达、超时不适用：本项只涉及本地 Git 基线及快照，不新增这些外部操作。
3. **全部迁移**：无法复现→AwaitInfo→Reply→Locate→ReproCheck→Fix；复现未失败→Locate→ReproCheck→Fix；Failed→rerun locate；Failed→rerun fix（有集启动、空集/读盘失败拒绝）；Fix 篡改→协议失败均由上述用例覆盖。既有 guard/cd 覆盖合法产品修复进入复核与工作区成员；共享十条用例覆盖修测试接受、HEAD 红回修复、基线绿未复现及拒绝分支。
4. **输入域边界**：空集合、单个脚本、首次无快照、已有快照、非祖先基线、多仓库、带空格和中文路径、内联测试段及已有测试文件的新增块均有本次执行的用例。日期、整数最大值不适用：该机制没有新增日期或数值输入域。

结论：M-07 待审。只推进本里程碑；没有运行项目门禁、切换分支或推送。
