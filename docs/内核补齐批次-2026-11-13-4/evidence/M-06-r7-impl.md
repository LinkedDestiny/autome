# M-06 · 实现 #7

## 修复与回归来源

根因是 `bug_loop::apply_for` 未接住 Failed / Stopped 的 Reply，落入需求转换表拒绝。复用 `refs/keep/t75-fix` 的域转换与调度器测试，手工合入现有分支；没有搬旧任务文档。保留修复轮历史，上限至少为已用轮数加配置 fix_rounds；溢出拒绝。其它状态给出中文恢复命令，帮助和技术设计同步。

回归测试独立提交 `0e9fffb8`，不含产品修复。CLI 夹具从 `refs/keep/t75-test` 重写：停止路径为 1 失败、2 rerun 后停止、3 reply；每次准备脚本，等待第 2 次 prompt 出现后才 stop，检查实际 reply 会话的 prompt。全部使用原有临时 Sandbox / World，不接触真实账本。上轮审计 M-05 通过，无退回缺陷检查需要搬入。

## 先红、临时绿、提交、正式绿

各段命令在修复前与修复后逐字相同：

| 命令 | 修复前 | 正式修复后 | 原始日志（`.autome/output/` 下） |
|---|---|---|---|
| `cargo test --offline -p autome-cli --test cli bug_loop_reply_` | 3 个：1 过 2 败，退出 101 | 3 过，退出 0 | `m06-red-cli.txt` / `m06-green-cli.txt` |
| `cargo test -p autome-domain bug_loop_reply_` | 4 个：2 过 2 败，退出 101 | 4 过，退出 0 | `m06-red-domain.txt` / `m06-green-domain.txt` |
| `cargo test -p automed --lib bug_reply_after_interruption` | 1 个：0 过 1 败，退出 101 | 1 过，退出 0 | `m06-red-core.txt` / `m06-green-core.txt` |
| `cargo test -p autome-cli bug_loop_usage_no_longer_says_reply_equals_reject` | 既有帮助断言按 B-E5 更新 | 1 过，退出 0 | `m06-green-help.txt` |

CLI 与调度器红态均报「Bug 节点不使用需求转换表」；域预算测试同样被拒，状态提示测试缺少下一步命令。编写 CLI 测试初期发现 Duration 未限定命名空间并修正，编译错误不计作缺陷复现。

提交回归前临时叠加域修复，前三段分别 3 / 4 / 1 通过（`m06-trial-{cli,domain,core}.txt`）；撤掉产品修复后域测试再次红。`cargo fmt --all -- --check` 退出 0（`m06-regression-fmt-check.txt`），再提交纯测试。正式验收四段以 `&&` 串联，整体退出 0。

## 用例数与基线

本里程碑验收总数 **9**，0 失败、0 忽略。上轮代码在同一筛选下有 4 个（CLI 1、域 2、调度器 0、帮助 1），本轮 **+5**：CLI 新增 2、域新增 2、调度器新增 1。设计原预估 7 漏计两个既有 `bug_loop_reply_` 域测试，现修正为 9，未删除或跳过用例。

直接相关子集：`cargo test -p autome-domain bug_loop_` 32 过，`cargo test -p automed --lib bug_loop_reply_` 2 过，退出均 0（`m06-related-domain.txt`、`m06-related-dispatch.txt`）。与验收去重总数 **39**，同范围上轮基线 34，差 **+5**。上一轮 M-05 审计报告的 66 个属于另一验收范围；本轮 39 比它少 27，来自切换到 B 的直接相关子集，不代表项目删掉测试。未跑全量门禁、clippy、Web lint/typecheck，留宿主快检与合并门禁执行。

## 标待审前自审清单

1. **情形表逐行**：Failed / Stopped 各由 CLI 验证 reply 原文仅写一次、干净提交、定位 prompt 引用 replies、最终 AwaitInfo；调度器对两者分别验证满槽 Requeued(Locate)、释放槽位后 Locate、预算 3→6、已用轮次仍为 3。Draft / Queued / Requeued / Paused / Done / Cancelled 及全部 20 个非等待活动节点的 Reply 均验证拒绝及下一步命令、不含转换表措辞。AwaitInfo / NotABug 原有域测试通过且预算不变；需求 reply 等同 reject 的 RPC 回归通过。帮助测试通过。
2. **失败分支**：空串与纯空白在 Failed / Stopped 下均拒绝；已用轮次 u32::MAX 时加法溢出明确拒绝。所有不允许 Reply 的状态均执行拒绝检查。本里程碑未新增授权、读盘、超时或外部服务分支，这些项不适用；现有基础设施路径未改。
3. **全部迁移**：Failed→Locate、Stopped→Locate 的直达路径由两个 CLI 用例覆盖；两者在满槽下 →Requeued(Locate)→Locate 由调度器矩阵覆盖；AwaitInfo / NotABug→Locate 原有测试覆盖；所有拒绝状态无转换。域测试验证恢复定位后的 ReproCheck→Fix 不立即预算失败，且较大的原预算不缩小。合并尾部共享测试仅调整 Reply 的预期中文错误，原有拒绝语义不变，32 个域子集全过。
4. **域边界**：空串、空白、中文、双空格、换行、字面 `{request}` 原样保存与只写一次；预算 None、已用 0、用尽 3、已有较高预算 20、加法恰好达到 u32::MAX、超过 u32::MAX 均执行。日期／前导零不适用，输入没有日期或数字字符串解析。

## 负向对照与收尾

- 临时把恢复预算改成优先沿用原 budget_n，`cargo test -p autome-domain bug_loop_reply_restores_a_fix_window_without_losing_history` 退出 101：上限仍为 3，预期 6（`m06-negative-budget.txt`）。
- 临时把 reply 会话号固定为 2、不准备 3.sh，`cargo test --offline -p autome-cli --test cli bug_loop_reply_after_stop` 退出 101，夹具不能到达 AwaitInfo（`m06-negative-fixture.txt`），能抓住 B-T2 的旧错误。
- 两次注入均已还原。重新跑 CLI `bug_loop_reply_` 与域 `bug_loop_reply_`，3 / 4 个全绿（`m06-final-cli.txt`、`m06-final-domain.txt`）。重复执行不计入用例总数。

结论：M-06 验收及四项自审通过，标为待审，交独立审计；不推进 M-07。
