# 复盘轮拿到本任务指标（含门禁数据），门禁中途装依赖后通过记录不再丢

## 背景

T-99 合并后核对出两处缺陷，都有账本与日志为证，都不是某个任务特有的。

### 一、复盘轮从来拿不到指标

`protocol/prompts/retro.md` 写着「下面是 Autome 记录的本任务指标。对着这些数字写，不要对着印象写。」，占位符 `{task_metrics}` 由 `launcher::render_task_metrics` 填。但是：

- 复盘轮启动时读的是 `ctx.store.task_metrics(&task.id)`（`crates/automed/src/scheduler.rs` 约 3722 行）。
- 指标只在进入 `Cleanup`、`Failed`、`Cancelled`、`Done` 时才写（`write_task_metrics`，同文件约 2903–2920 行的 `measure_now`）。
- Loop 的顺序是 复盘 → rebase → 合并前门禁 → 合并 → 清理，所以复盘轮启动时指标一定是空的，提示词里永远是「（本任务没有记录到指标。照常复盘，但不要编造数字。）」。

实证：T-99 的 `session.started`（复盘 #1）在 2026-11-16T15:08:15Z，`task.metrics` 在 15:33:51Z（合并之后）。T-98、T-99 的 `lessons.md` 第一段都是「本任务没有记录到 Autome 指标」，T-98 的七轮复盘全靠数证据文件。

另外，现有指标（`TaskMetrics::METRIC_NAMES` 共 12 项）里没有门禁数据。最近两个任务最贵的恰恰是门禁：T-98 合并前门禁 7 次共 7412 秒、每轮快检 29 次里 8 次失败；T-99 每轮快检 12 次里 4 次失败。这些数字现在只能手工查 `gate.result` 事件，复盘轮看不到，`predicted_impact.metric` 也引用不了。

### 二、门禁中途装依赖后，后面步骤的通过记录全部丢失

`scripts/ci/inputs.mjs` 的 `toolchain()` 把 `apps/web/node_modules/.package-lock.json` 与 `apps/desktop/node_modules/.package-lock.json` 的哈希算进每个步骤的键。`prepare` 模式在门禁开始时一次算好所有键；`execute_step`（`scripts/ci/lib.sh`）在步骤跑完后重算键，对不上就写「输入已变化或无法复核，不记录通过」并报 `partial`。

新 worktree 里没有 `apps/desktop/node_modules`。任务的第一次合并前门禁跑到 `Desktop install` 时执行 `npm ci`，该文件从 `absent` 变成有哈希，于是它之后的每个步骤重算出来的键都变了：

- T-99 合并前门禁（`.autome/output/gates/T-99/merge_gate-f43c3a1dbc63.report`）：`Desktop`、`Contract`、`Scripts`、`Web test`、`Web e2e` 五步全是 `partial	-`，`gate.result.passes` 里只有 `Test` 一条。
- T-96 合并前门禁（`merge_gate-a70c08f060f1.report`）：`Desktop`、`Contract`、`Scripts`、`Web build`、`Web test`、`Web e2e` 六步 `partial`。

后果是成本，不是正确性：门禁本身通过，但这些步骤的通过记录没有保存，合并前门禁失败后重跑时不能沿用（T-98 的 7 次合并前门禁里前面几次就是这样）。同理，任务第一次快检里 `Web install` 之后的步骤也可能受影响，设计轮核对。

## 要做什么

一次设计、一次评审，按下面两组里程碑实现。

### A 复盘轮拿到指标

- A-1 复盘轮启动前先算一次本任务指标，交给 `{task_metrics}`。合并后 / 失败 / 取消时的最终计量保留，最终值覆盖复盘时的值。手动 `autome` 触发的复盘（`task_retro`）同样拿到指标。
- A-2 指标增加门禁数据，至少包含：合并前门禁次数与失败次数、每轮快检次数与失败次数、门禁总耗时（秒）。数据取自账本 `gate.started` / `gate.result` 事件；怎样命名、是否拆成快检与合并前两组由设计轮定。新指标进 `METRIC_NAMES`，复盘提示词的表格里显示，`predicted_impact.metric` 可以引用。
- A-3 已经读取指标的地方（`crates/automed/src/version_page.rs` 的协议页数据、`dispatch_protocol.rs`、`curation.rs`）兼容新字段：旧任务没有这些字段时按缺失处理，不当成 0。面板目前不展示任务指标，本任务不新增展示。

### B 通过记录不因门禁内装依赖而丢

- B-1 门禁在本次运行里自己执行了依赖安装（`Desktop install`、`Web install`）之后，后面通过的步骤照常记录通过，下次输入没变时可以沿用。修法由设计轮定（例如：先装依赖再算键；或键改用已在 `GLOBAL_INPUTS` 里的 `package-lock.json`，不再读已安装目录的标记），但必须保住下面两条现有保证。
- B-2 保证一：依赖没装或装的与锁文件不一致时，不能沿用按「已装好」算出的通过记录。
- B-3 保证二：步骤运行期间输入真的被改了（源码、锁文件），仍然不记录通过。
- B-4 所有步骤都能沿用时不为了算键去装依赖（T-97 的「全沿用时不跑准备步骤」不回退）。

## 不做什么

- 不改协议文本、不新增协议版本（`retro.md` 提示词的占位符已经在，只是没填上）。
- 不升账本结构版本：`tasks.metrics` 是 JSON 列，新字段带默认值即可；设计轮确认，若确实需要升级，在设计里写明迁移与理由。
- 不动 `.autome/`。
- T-99 复盘的 L-01…L-04（实现轮跑的检查比宿主窄、审计放行计时类失败、负向对照没有宿主执行路径、★ 命令定稿前没执行过）不在本任务里做，先看 `quick_session` 配置生效后的数据。

## 验收标准

- 验-1 夹具任务走到复盘轮：复盘会话的提示词里是指标表（设计轮、实现轮、reopen、门禁各项都有数），不是「没有记录到指标」。负向对照：把复盘前的计量去掉，测试变红。
- 验-2 夹具任务有 2 次合并前门禁（1 败 1 过）与 3 次快检（1 败）：指标里的次数、失败数与账本 `gate.result` 事件一致；合并后的最终指标包含复盘之后发生的门禁。
- 验-3 旧任务（指标里没有门禁字段）：读取指标的各处不报错，新指标按缺失处理、不参与均值；用一份不含新字段的旧指标 JSON 做测试。
- 验-4 夹具仓库没有 `node_modules`：完整门禁第一次运行后，安装步骤之后的每个通过步骤在报告里都是 `ran` 且带键；不改任何输入再跑一次，这些步骤全部「沿用」。负向对照：恢复现在的算键时机，该测试变红。
- 验-5 B-2、B-3 各有一条测试：已装依赖与锁文件不一致时不沿用；步骤运行期间改了输入时报 `partial`。
- 验-6 `sh scripts/ci-local.sh` 全部通过。

## 参考

- `crates/automed/src/scheduler.rs`（`write_task_metrics`、`measure_now`、复盘轮取 `task_metrics` 处）、`crates/automed/src/launcher.rs`（`render_task_metrics`）、`crates/automed/src/task_metrics.rs`、`crates/autome-domain/src/metrics.rs`（`TaskMetrics`、`METRIC_NAMES`）、`protocol/prompts/retro.md`
- `scripts/ci/inputs.mjs`（`toolchain`、`calculate`、`prepare`）、`scripts/ci/lib.sh`（`execute_step`、`execute_prep`）、`scripts/ci/install-node.sh`、`scripts/ci/steps.sh`、`scripts/test/ci-reuse.test.mjs`
- T-97 设计文档（步骤沿用与键的组成）、`docs/合并前门禁一次通过/lessons.md`、`docs/面板收尾批次-用上新的内核数据-补齐备注/lessons.md`
