# 任务：复盘轮拿到本任务指标（含门禁数据），门禁中途装依赖后通过记录不再丢

本任务的规则见 `docs/复盘轮拿到本任务指标-含门禁数据/protocol/loop-protocol.md` 与 `docs/复盘轮拿到本任务指标-含门禁数据/protocol/session-protocol.md`。

- 设计文档：`docs/复盘轮拿到本任务指标-含门禁数据/复盘轮拿到本任务指标-含门禁数据.md`
- 任务卡（用户写的，第一输入）：`docs/复盘轮拿到本任务指标-含门禁数据/复盘轮拿到本任务指标-含门禁数据-card.md`
- 任务整理日期：2026-11-17；任务编号 T-100
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）。整理时的基线提交是 `a326d2ff`。往前依次是本任务的输入提交
  `15abe608`、T-99 合并后应用配置的提交 `a0e1e005`、T-99 的合并提交 `ca06307c`
- 本任务固定的协议版本：`protocol/v9`。账本 schema 现为 9（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）

## 1. 需求原文

> 复盘轮拿到本任务指标（含门禁数据），门禁中途装依赖后通过记录不再丢

任务卡正文比这句话具体，两者不一致时以任务卡为准。各节内容如下：

- §2–§5 照任务卡整理，A-1…B-4、验-1…验-6 沿用任务卡编号。
- §6–§7 是整理轮从调研得出的约束与待定问题。
- §8 是代码现状，§9 是前几个任务留下的教训，§10 是项目背景。

## 2. 背景与目标

T-99 合并后核对出两处缺陷。两处都有账本与日志为证，都不是哪个任务特有的。

**一、复盘轮从来拿不到指标。** 复盘提示词 `protocol/prompts/retro.md` 写着「下面是 Autome 记录的本任务指标。对着这些数字写，
不要对着印象写。」，占位符 `{task_metrics}` 由 `launcher::render_task_metrics` 填。但是：

- 复盘轮启动时读的是 `ctx.store.task_metrics(&task.id)`（`crates/automed/src/scheduler.rs` 约第 3722 行）。
- 指标只在以下几种转移时才写（`write_task_metrics`，同文件约第 2909–2920 行的 `measure_now`）：
  - 进入 `Cleanup`；
  - 进入 `Failed` 或 `Cancelled`；
  - 进入 `Done` 且尚无指标。
- Loop 的顺序是 复盘 → rebase → 合并前门禁 → 合并 → 清理，所以复盘轮启动时指标一定是空的。提示词里永远是
  「（本任务没有记录到指标。照常复盘，但不要编造数字。）」。

实证：

- T-99 复盘 #1 的 `session.started` 在 2026-11-16T15:08:15Z，`task.metrics` 在 15:33:51Z（合并之后）。
- T-98、T-99 的 `lessons.md` 第一段都是「本任务没有记录到 Autome 指标」。T-98 的七轮复盘全靠数证据文件。

另外，现有指标（`TaskMetrics::METRIC_NAMES` 共 12 项）里没有门禁数据。最近两个任务最贵的恰恰是门禁：

- T-98：合并前门禁 7 次，共 7412 秒；每轮快检 29 次，其中 8 次失败。
- T-99：每轮快检 12 次，其中 4 次失败。

这些数字现在只能手工查 `gate.result` 事件。复盘轮看不到它们，`predicted_impact.metric` 也引用不了。

**二、门禁中途装依赖后，后面步骤的通过记录全部丢失。** 机制如下：

- `scripts/ci/inputs.mjs` 的 `toolchain()` 把两份已安装目录的标记文件算进每个步骤的键：
  `apps/web/node_modules/.package-lock.json` 与 `apps/desktop/node_modules/.package-lock.json`。
- `prepare` 模式在门禁开始时一次算好所有键。
- `execute_step`（`scripts/ci/lib.sh`）在步骤跑完后重算键。对不上就写「输入已变化或无法复核，不记录通过」，并报 `partial`。

新 worktree 里没有 `apps/desktop/node_modules`。任务的第一次合并前门禁跑到 `Desktop install` 时执行 `npm ci`，
上面那个标记文件从 `absent` 变成有哈希。于是它之后每个步骤重算出来的键都变了：

- T-99 合并前门禁（`.autome/output/gates/T-99/merge_gate-f43c3a1dbc63.report`，在主检出里）：
  - `Desktop`、`Contract`、`Scripts`、`Web test`、`Web e2e` 五步全是 `partial	-`；
  - `gate.result.passes` 里只有 `Test` 一条。
  - 整理轮已只读核对过这份报告，与任务卡一致。
- T-96 合并前门禁（`merge_gate-a70c08f060f1.report`）：`Desktop`、`Contract`、`Scripts`、`Web build`、`Web test`、`Web e2e`
  六步 `partial`。

后果是成本，不是正确性。门禁本身通过了，但这些步骤的通过记录没有保存，合并前门禁失败后重跑时不能沿用（T-98 的 7 次合并前
门禁里前面几次就是这样）。同理，任务第一次快检里 `Web install` 之后的步骤也可能受影响，由设计轮核对。

**目标。** 达到下面两点，并且两点都由内核或门禁脚本机械保证、有测试钉住：

- 复盘轮启动时拿到本任务当时的指标，其中包括门禁次数、失败次数与耗时。
- 门禁本次运行里自己装了依赖之后，后面通过的步骤照常记录通过，下次输入没变时可以沿用。现有的两条正确性保证不放松。

## 3. 任务要求（任务卡「要做什么」逐条照录；设计文档的里程碑必须全部覆盖）

**总要求（任务卡）。** 一次设计、一次评审，按下面 A、B 两组里程碑实现。

### 3.1 A 复盘轮拿到指标

- **A-1** 复盘轮启动前先算一次本任务指标，交给 `{task_metrics}`。合并后 / 失败 / 取消时的最终计量保留，最终值覆盖复盘时的值。
  手动 `autome` 触发的复盘（`task_retro`）同样拿到指标。
- **A-2** 指标增加门禁数据，至少包含：
  - 合并前门禁次数与失败次数；
  - 每轮快检次数与失败次数；
  - 门禁总耗时（秒）。

  数据取自账本 `gate.started` / `gate.result` 事件。怎样命名、是否拆成快检与合并前两组由设计轮定。新指标要做到三件事：
  进 `METRIC_NAMES`；在复盘提示词的表格里显示；`predicted_impact.metric` 可以引用。
- **A-3** 已经读取指标的地方兼容新字段：`crates/automed/src/version_page.rs` 的协议页数据、`dispatch_protocol.rs`、
  `curation.rs`。旧任务没有这些字段时按缺失处理，不当成 0。面板目前不展示任务指标，本任务不新增展示。

### 3.2 B 通过记录不因门禁内装依赖而丢

- **B-1** 门禁在本次运行里自己执行了依赖安装（`Desktop install`、`Web install`）之后，后面通过的步骤照常记录通过，下次输入没变时
  可以沿用。修法由设计轮定。任务卡举了两个例子：先装依赖再算键；或者键改用已在 `GLOBAL_INPUTS` 里的 `package-lock.json`，
  不再读已安装目录的标记。不管选哪种，都必须保住下面两条现有保证。
- **B-2** 保证一：依赖没装，或者装的与锁文件不一致时，不能沿用按「已装好」算出的通过记录。
- **B-3** 保证二：步骤运行期间输入真的被改了（源码、锁文件），仍然不记录通过。
- **B-4** 所有步骤都能沿用时不为了算键去装依赖（T-97 的「全沿用时不跑准备步骤」不回退）。

## 4. 验收标准（任务卡原文）

- **验-1** 夹具任务走到复盘轮：复盘会话的提示词里是指标表（设计轮、实现轮、reopen、门禁各项都有数），不是「没有记录到指标」。
  负向对照：把复盘前的计量去掉，测试变红。
- **验-2** 夹具任务有 2 次合并前门禁（1 败 1 过）与 3 次快检（1 败）：指标里的次数、失败数与账本 `gate.result` 事件一致；合并后
  的最终指标包含复盘之后发生的门禁。
- **验-3** 旧任务（指标里没有门禁字段）：读取指标的各处不报错，新指标按缺失处理、不参与均值；用一份不含新字段的旧指标 JSON 做
  测试。
- **验-4** 夹具仓库没有 `node_modules`：完整门禁第一次运行后，安装步骤之后的每个通过步骤在报告里都是 `ran` 且带键；不改任何输入
  再跑一次，这些步骤全部「沿用」。负向对照：恢复现在的算键时机，该测试变红。
- **验-5** B-2、B-3 各有一条测试：已装依赖与锁文件不一致时不沿用；步骤运行期间改了输入时报 `partial`。
- **验-6** `sh scripts/ci-local.sh` 全部通过。

## 5. 范围

**在范围内（按任务卡「参考」与调研得出，具体落点以设计为准）。**

| 项 | 可能改的地方 |
|---|---|
| A-1 | `crates/automed/src/scheduler.rs`：`write_task_metrics`、`measure_now`、`start_session` 里给复盘轮取 `task_metrics` 处（约第 3722 行）、`start_retro`（约第 5716 行）；`crates/automed/src/store.rs`：`task_metrics` / `set_task_metrics` / `tasks_with_metrics` |
| A-2 | `crates/autome-domain/src/metrics.rs`（`TaskMetrics`、`METRIC_NAMES`、`value()`）；`crates/automed/src/task_metrics.rs`（`Counts`、`aggregate`）；`crates/automed/src/launcher.rs` 的 `render_task_metrics`；读事件可能要在 `store.rs` 加查询（`events` 表有 `at` 列） |
| A-3 | `crates/automed/src/{version_page.rs,dispatch_protocol.rs,curation.rs,dispatch_curation.rs}`；`crates/automed/tests/prompt_rendering.rs` 与 `crates/automed/tests/golden/`（`{metric_vocabulary}` 会随 `METRIC_NAMES` 变）；`crates/automed/tests/end_to_end.rs` |
| B | `scripts/ci/inputs.mjs`（`toolchain`、`calculate`、`prepare`、`quick-prepare`）、`scripts/ci/lib.sh`（`execute_step`、`execute_prep`）、`scripts/ci/install-node.sh`、`scripts/ci/steps.sh`；测试 `scripts/test/ci-reuse.test.mjs`、`ci-install.test.mjs`、`ci-quick.test.mjs` |
| 文档 | `README.md` 的 Host gate 一节（步骤键的组成若变）；`docs/development/technical-design.md` 是否同步由设计轮定 |

**不做（任务卡「不做什么」，逐条照录）。**

- 不改协议文本、不新增协议版本（`retro.md` 提示词的占位符已经在，只是没填上）。
- 不升账本结构版本：`tasks.metrics` 是 JSON 列，新字段带默认值即可；设计轮确认，若确实需要升级，在设计里写明迁移与理由。
- 不动 `.autome/`。
- T-99 复盘的 L-01…L-04 不在本任务里做，先看 `quick_session` 配置生效后的数据。这四条是：
  - L-01 实现轮跑的检查比宿主窄；
  - L-02 审计放行计时类失败；
  - L-03 负向对照没有宿主执行路径；
  - L-04 ★ 命令定稿前没执行过。

**整理轮补充的边界。** 这些边界从调研得出，不扩大范围。标「整理轮理解」的是解释，设计轮可以据证据修正。

- 「不改协议文本」指两处都不改：
  - 仓库根 `protocol/`，内核用 `include_str!` 编译进去的 seed；
  - 本任务的副本 `docs/复盘轮拿到本任务指标-含门禁数据/protocol/`。

  `launcher.rs` 里的 `render_task_metrics` 输出的表格是代码，在范围内。`{metric_vocabulary}` 由 `METRIC_NAMES` 代入，同样
  在范围内。
- 整理轮理解：「面板目前不展示任务指标，本任务不新增展示」指 `apps/web` 不新增界面，但已有的读取方要保持正确（见约束 4）。
- launchd 里的 `com.autome.loop`（驱动本仓库 Loop）在仓库之外，会话不得读写、装卸、kickstart。

## 6. 硬性约束

1. **分组与顺序（总要求）。** 里程碑按 A、B 分组，每组至少一个。实现轮只推进最小编号的开放里程碑，编号顺序即执行顺序。前面
   里程碑的验收不得依赖后面才有的东西。每个里程碑都有协议要求的三行验收契约：验收命令、预期、负向对照。
2. **机械保证。** 每一条都由内核代码或门禁脚本保证，并有测试钉住。不得以「在提示词里要求会话做 X」作为某条要求的实现。
3. **不改协议、不升 schema、不动 `.autome/`**（见 §5「不做」）。
   - 新字段在 `TaskMetrics` 上用 `#[serde(default, skip_serializing_if = …)]` 一类方式，使旧 JSON 能读。
   - 「按缺失处理」意味着新字段在 `value()` 里对旧任务返回 `None`，不是 `Some(0.0)`。`SessionMetrics` 已有同样的
     「零是断言，缺失不是」写法可参照。
   - 账本 schema 保持 9。若设计坚持要升，写明理由与迁移。
4. **在途任务的指标不得污染已终结任务的统计（A-1 带来的新风险，整理轮调研）。** 下面这些地方都靠
   「有没有 `tasks.metrics`」来认定任务是否已终结：
   - `store.rs` 的 `tasks_with_metrics` 只按 `metrics` 非空筛，不看任务状态，注释却写「Every terminal task」。
     它被协议版本页（`dispatch_protocol::versions` → `version_page::rows`）与移除实验的基线（`dispatch_curation.rs` 约第
     359 行，「这个项目还没有任何带指标的已终结任务」）读取。
   - `scheduler.rs` 的 `measure_now` 有一支 `transition.next == TaskState::Done && task.metrics.is_none()`：已有指标时进入
     `Done` 不再计量。
   - `dispatch.rs` 约第 2151 行的注释写着「`task.metrics` 是最终汇总，只在任务结束后存在」。任务列表约第 729 行把
     `t.metrics` 原样回给面板。
   - `apps/web/app/components/task-stage-model.ts` 在没有状态块时用 `task.metrics` 事件的 `design_rounds_used` / `impl_rounds_used`
     画轮次。

   如果复盘前的计量写进同一列、发同一种事件，上面这些都会把在途任务当成已终结，或者把复盘时的值当成最终值。设计必须给出
   处理方式，并有测试钉住「在途任务不进入版本页均值与移除实验基线」「最终计量仍然发生并覆盖」。可选的做法包括：复盘时只算
   不存，直接交给提示词；分开存；读取方按状态过滤。
5. **门禁数据口径要和账本对得上（A-2、验-2）。**
   - `gate.started` 载荷：`node`、`sha`、`command`、`kind`（`quick` / `full` / `repro`）、`lane`、`log`。
   - `gate.result` 载荷：`node`、`sha`、`ok`、`exit_code`、`kind`、`command`、`log`、`tail`，可能还有 `digest`、`cases`、
     `steps`、`passes`、`fingerprint`、`same_failures`、`tripped`。
   - **事件里没有耗时字段**，耗时只能从 `events.at` 算（`gate.started` 到对应 `gate.result`）。
   - 另有 `gate.reused`（沿用了上一次结果，没有真跑）与 `gate.skipped` 两种事件。
   - 同因熔断时，`record_gate_result` 可能回放上一条结果而不新写事件。

   设计须写明以下口径，每一项都要有测试：
   - 「合并前门禁」与「每轮快检」各按哪些 `node` / `kind` 认定。Bug Loop 的 `FixGate`、`ReproCheck`，主线核对 `MainCheck`
     算不算、算到哪一组。
   - `gate.reused` / `gate.skipped` 算不算一次。
   - 没有配对 `gate.result` 的 `gate.started`（门禁还在跑、作业消失、超时）怎么计。
   - 总耗时是否含排队时间。门禁按机器排队，`gate.started` 是作业真正起跑时写的，设计轮复核。
6. **不接触真实账本与真实安装。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`）驱动自己的 Loop，账本在
   `~/.autome`。
   - 任何测试与手工验证都把 `AUTOME_HOME`（起面板时还有 `HOME`）指到临时目录。
   - 拉真实内核的测试**显式**指定 `<检出>/target/debug/automed` 或 `target/debug/autome`。缺省查找会落到 PATH 上已安装的
     另一个版本。
   - 验-1、验-2 的「夹具任务」在测试自己建的临时仓库与临时账本里构造。不得把本 worktree、主检出或 `.worktree/` 下任何目录
     注册进测试账本。
   - 查真实账本取数据只许只读打开，例如 `sqlite3 -readonly ~/.autome/state/automed.sqlite3`，原始摘录写进 `.autome/output/`。
7. **门禁脚本的两条保证与沿用语义不放松（B-2、B-3、B-4）。** T-97 的沿用以内容寻址的键为准。键由以下几部分组成：
   - `ci-local-key 1`；
   - 步骤名、命令；
   - 工具链哈希；
   - 未声明路径哈希；
   - 声明输入与 `GLOBAL_INPUTS` 的 tree 条目。

   改键的组成时要说明 `ci-local-key` 版本号是否要变，以及旧通过记录在新键下的命运（不得错误命中）。
   - `install-node.sh` 已经在 `node_modules/.autome-lock.sha256` 里记了锁文件指纹，可以作为「已装的与锁文件一致」的判据之一。
     设计轮自行判断够不够：`npm ci` 失败后标记不写，手工 `npm install` 不写标记。
   - `node_modules` 被 `.gitignore` 忽略，不在快照 tree 里。
   - 测试用 PATH 上的假 `npm`，写法见 `ci-install.test.mjs`。不得在测试里真的联网 `npm ci`。
8. **快检路径同样核对。** `inputs.mjs` 的 `quick-prepare` 也在开头一次算好键，然后才可能执行 `Web install`，`Web install` 只在
   `Web lint` / `Web typecheck` 不能沿用时才执行；浏览器阶段还会再 `prepare('Web install')`、`prepare('Build core')`。
   B-1 的修法要同时覆盖完整门禁（`ci-local.sh` → `prepare`）与快检（`ci-quick.sh` → `quick-prepare`），或者在设计里写明快检为什么
   不受影响，并给出证据。
9. **门禁步骤与测试分组的钉子。**
   - `scripts/ci/steps.sh` 的每个 `step` / `prep` 都有测试引用。
   - `apps/web/test/ci-config.test.ts` 钉住 Web 各步骤写法与 `.github/workflows/ci.yml`。
   - `scripts/test/ci-*.test.mjs` 钉住快检与沿用行为。

   改这些文件的里程碑同轮同步对应测试。
10. **向后兼容。** 已经在跑的其它任务分支仍用自己分支里的旧脚本。本任务分支自己的门禁由当前配置的命令驱动，这条命令是
    `sh scripts/ci-local.sh` / `scripts/ci-quick.sh`。改了键的组成后，第一次门禁不能沿用旧记录，这可以接受，但不得错误沿用。
11. **断言子进程文本输出时固定颜色环境**：spawn 时显式设 `NO_COLOR` / `FORCE_COLOR`。
12. **脚本约定。** `scripts/*.sh`、`scripts/ci/*.sh` 为 `/bin/sh`（macOS bash 3.2）而写：不用数组、`[[`、`local`；中文紧跟变量时
    写 `${var}`。
13. **依赖。** 预计不需要新依赖。要加须说明理由；改 `package.json` 必须同轮提交 `package-lock.json`。
14. **文案语言沿用现状。** 下面这些用中文：
    - 注入给会话的文字，包括指标表；
    - CLI 输出、报错、门禁日志；
    - 设计文档与 `docs/development/technical-design.md`。

    代码注释与 `README.md` 用英文。
15. **设计文档体量。** 协议警告线 ≤ 80KB，单轮增长 ≤ 10KB。证据写进 `evidence/`，`## 里程碑` 一节只放那一张表。
16. **会话边界（AGENTS.md）。**
    - 只在本 worktree 内工作。不切分支、不动 `.worktree/` 下其它目录、不推送远端。
    - 不修改 `.autome/`。`.autome/output/` 是输出目录，可以写。
    - 超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录。
    - 完成本轮后结束会话，不自行启动下一个会话。

## 7. 待设计轮回答的问题

这里只列问题与相关事实，不预设答案。设计文档「方案」一节须逐条给出结论与依据。没有依据支持改动任务卡字面的，按任务卡字面做。

### A

- **A-Q1 复盘前计量放在哪。** 有两个入口：
  - 自动复盘经 `advance` → `perform` → `start_session`；
  - 手动复盘经 `dispatch.rs` 的 `task_retro` → `scheduler::start_retro` → `start_session`。

  两者都在 `start_session` 里按 `kind.role() == Some(Role::Retro)` 取 `task_metrics`。可选的落点：
  - 在 `start_session` 取值处现算；
  - 在 `measure_now` 里加「进入 `Node::Retro`」；
  - 其它。

  需说明以下几点：
  - 复盘可能跑多次：T-98 每次合并前门禁失败后都再复盘，共七轮。每次是否重算。
  - 手动复盘的任务已是 `Failed` / `Cancelled` / 暂停，已有最终值，是否重算。
  - `write_task_metrics` 读的状态块与证据文件在复盘时都在 worktree 里。
- **A-Q2 复盘前的值存不存、存在哪（约束 4）。** 有三种选择：只交给提示词不落账本；写进同一列并靠后续覆盖；分开存。每种都要说明
  在途期间对四类读取方的影响：版本页、移除实验基线、`measure_now` 的 `Done` 分支、面板读 `task.metrics` 事件。
- **A-Q3 门禁指标的名称与分组（A-2）。** 需要定：
  - 字段名；
  - 是否拆快检 / 合并前；
  - 总耗时含哪些门禁；
  - 是否同时给「失败次数」与「次数」两个独立指标（`predicted_impact` 一次只引用一个名字）。

  还要说明这些指标是否进 `version_page::WARNING_METRICS`（现为三项）。任务卡没要求，不改就写明不改。
- **A-Q4 口径细节（约束 5）。** 包括：`node` / `kind` 的归组；`gate.reused` / `gate.skipped`；未配对的 `gate.started`；同因熔断
  回放；Bug 任务的门禁。
- **A-Q5 表格呈现（A-2、验-1）。** 新指标在 `render_task_metrics` 表格里的行名与格式，例如「合并前门禁 2 次（失败 1）」或分行。
  旧任务、没跑过门禁的任务显示什么：缺失与 0 要区分。
- **A-Q6 现有怪处是否顺带处理。** `METRIC_NAMES` 含 `mean_request_input`，但 `TaskMetrics::value()` 对它返回 `None`
  （它是会话级字段，`TaskMetrics` 上没有）。这不在任务卡要求内。若新指标的实现牵涉 `value()`，设计轮决定是保持现状，还是写进
  Backlog。

### B

- **B-Q1 修法选型。** 任务卡给了两种：
  - (a) 先装依赖再算键。会碰 B-4：全沿用时不能为算键而装。
  - (b) 键改用 `GLOBAL_INPUTS` 里已有的 `package-lock.json`。此时要另外保证 B-2，即已装的与锁文件一致。可选的判据有：
    步骤执行前核对 `.autome-lock.sha256`；`npm ls`；其它。

  也可以有 (c) 其它方案。设计须逐条论证 B-2、B-3、B-4 在所选方案下怎样成立。
- **B-Q2 「已装依赖与锁文件不一致」的判定点。** 沿用（不执行）的步骤不看 `node_modules`，所以判定点有两个候选：
  - 沿用决策之前；
  - 执行步骤之前。

  要回答：不一致时是不沿用，还是先装再执行。还要回答 `Desktop install` / `Web install` 本身失败时，后续步骤怎么记。
  现行行为是 prep 失败仍跑后续检查，见 `ci-reuse.test.mjs` 第 17 例。
- **B-Q3 快检是否受影响（约束 8）。** 用 T-99 的快检报告核对：主检出 `.autome/output/gates/T-99/gate-*.report`，只读。新 worktree
  第一次快检里，`Web install` 之后的步骤是否也是 `partial`。
- **B-Q4 键版本（约束 7）。** 改组成后 `ci-local-key 1` 是否升到 `2`，与 `gate.rs` 的 `passes` 读取有无耦合。

## 8. 代码现状（整理轮调研，基线 `a326d2ff`；行号为约数，设计轮引用前自行复核）

**指标（内核）。**

- `crates/autome-domain/src/metrics.rs`：
  - `TaskMetrics` 的字段有 `protocol_ref`、`rules_hash`（都是 Option）、`design_rounds_used/limit`、`impl_rounds_used`、
    `budget_n`、`milestones`、`reopen_total`、`reopen_by_domain`、`impl_defects`、`verification_gaps`、`protocol_failures`、
    `closed_then_contradicted`、`manual_items_open`、`total_cost_usd`（Option）、`total_tokens`、`total_turns`。非 Option 的
    字段没有 `serde(default)`。
  - `METRIC_NAMES: [&str; 12]`，`is_metric`，`value(name) -> Option<f64>`（`total_cost_usd` 缺失时返回 `None`）。
  - 测试 `every_metric_name_can_actually_be_read_back`。
- `crates/automed/src/task_metrics.rs`：`count_verdicts`（读 `evidence/*-audit.md`）、`manual_items_open`、`newly_contradicted`、
  `snapshot`、`Counts`、`aggregate(status, budget_n, protocol_ref, counts)`。
- `crates/automed/src/scheduler.rs`：
  - `measure_now`（约第 2909 行）、`write_task_metrics`（约第 2996 行）。它会 `set_task_metrics`，并发 `task.metrics` 事件。
  - `start_session` 里复盘取值（约第 3722 行）。
  - `start_retro`（约第 5716 行）：任务须已停下且 worktree 还在，否则拒绝。
- `crates/automed/src/launcher.rs`：
  - `render_task_metrics`（约第 859 行）：缺值时输出「（本任务没有记录到指标。照常复盘，但不要编造数字。）」，有值时输出
    「| 指标 | 值 |」表格。
  - `{metric_vocabulary}` = `METRIC_NAMES.join(" / ")`（约第 849 行）。
  - `check_no_placeholders_left`。
- `crates/automed/src/store.rs`：`task_metrics`、`set_task_metrics`、`tasks_with_metrics`（约第 1564–1600 行）；`count_events`、
  `task_events(task, kind)`（按 seq 取载荷，不带 `at`）、`last_event*`；`events` 表列为 `seq, event_id, kind, subject_id, payload, at`。
- 读取方：
  - `version_page.rs`：`rows` 对 `METRIC_NAMES` 逐个取 `value()`，`None` 不参与均值；还有 `WARNING_METRICS`、`realized`。
  - `dispatch_protocol.rs::versions`：回包含 `metric_names`。
  - `curation.rs::metric_for`：领域 → 指标，测试要求返回值 `is_metric`。
  - `dispatch_curation.rs`：移除实验基线。
  - `dispatch.rs`：任务回包的 `metrics`、`so_far`。
  - `apps/web/app/components/task-stage-model.ts`：读 `task.metrics` 事件。
- 测试：
  - `crates/automed/tests/prompt_rendering.rs` 约第 272 行断言复盘提示词含「没有记录到指标」。这是现状的钉子，A-1 落地后这条
    断言要改写。
  - `crates/automed/tests/golden/` 是提示词快照。差异核对后用 `AUTOME_UPDATE_GOLDEN=1` 接受。
  - `crates/automed/tests/end_to_end.rs` 约第 1612 行：合并后取 `task_metrics` 断言里程碑数等。

**门禁事件（内核）。**

- `scheduler.rs` 的 `run_gate_attempt`（约第 4950 行）起作业后写 `gate.started`，结束时组 `result`，经 `record_gate_result`
  （约第 5115 行）写 `gate.result`。`gate.reused` 在约第 2763、4776、4816 行，`gate.skipped` 在约第 4680、4767、4891 行。
- `dispatch.rs` 约第 4447–4511 行也写 `gate.started` / `gate.result`，用于主线核对等，主体可能不是任务。
- `gate_queue.rs`：轻 / 重两条车道，按机器排队，起跑时才写 `gate.started`。

**门禁脚本。**

- `scripts/ci/steps.sh`：
  - `GLOBAL_INPUTS` 含 `apps/web/package.json apps/web/package-lock.json apps/desktop/package.json apps/desktop/package-lock.json`。
  - 步骤顺序：Format、Clippy、Test、`prep Desktop install`、`prep Build core`、Desktop、Contract、Scripts、`prep Web install`、
    Web lint、Web typecheck、Web build、Web test、Web e2e。
- `scripts/ci/install-node.sh`：锁文件指纹与 `node_modules/.autome-lock.sha256` 不同，或者没有 `node_modules`，就 `npm ci`
  并写标记。
- `scripts/ci/inputs.mjs`：
  - `toolchain()` 哈希工具版本、`RUSTFLAGS`、`NODE_OPTIONS`，以及两份 `node_modules/.package-lock.json`（不存在记 `absent`）。
  - `calculate()` 得出各步骤的键。
  - `prepare` 模式写出执行脚本：prep 只在其后还有不能沿用的步骤时才写。这就是 B-4 的现状。
  - `key <i>` 模式供 `execute_step` 事后重算。
  - `quick-prepare` / `quick-plan` 是快检的对应路径。
- `scripts/ci/lib.sh`：
  - `execute_prep`（支持 `--if-missing`）。
  - `execute_step`（约第 193 行）有四个分支：沿用 → `reused`；失败 → `failed`；重算键相同 → `ran`；否则 → `partial -`。
  - `ci_report` 写 `$AUTOME_GATE_REPORT`。
- `scripts/ci-local.sh`：`--plan` / `--all` / `--check-claims` / `--package`。
- 测试：
  - `scripts/test/ci-reuse.test.mjs` 共 18 例，其中第 11 例「全沿用时没有 Build core」对应 B-4，第 13 例「运行中输入被改
    → partial」对应 B-3，第 17 例是 prep 失败。
  - `scripts/test/ci-install.test.mjs` 用假 `npm` 测指纹。
  - `ci-quick.test.mjs`、`ci-e2e.test.mjs`。
  - 跑法：`node --test scripts/test/*.test.mjs`（门禁 `Scripts` 步骤）。

**可用的实测材料（只读）。**

- 主检出 `/Users/dannie/project/autome-v2/.autome/output/gates/T-99/` 与 `T-96/` 的 `*.report` / `*.log`。
- 真实账本 `~/.autome/state/automed.sqlite3` 的 `gate.started` / `gate.result` / `task.metrics` 事件，只读打开。
- `docs/合并前门禁一次通过/lessons.md`（T-99）、`docs/面板收尾批次-用上新的内核数据-补齐备注/lessons.md`（T-98）。
- T-97 文档 `docs/降低宿主门禁开销-输入没变的步骤不重跑/`：步骤沿用与键的组成。单份文档 20–100 KB，按节读，不要整份读。

## 9. 前车之鉴（来自已合并任务的复盘，逐条可核对）

1. **复盘轮写不出数字，正是本任务 A 的起因。** T-98、T-99 的 `lessons.md` 首段都说没有指标。验-1 要的是提示词里真的有表，不是
   「代码路径存在」。
2. **改了 RPC 回包或事件形状，grep 全部使用方**（T-95：四个里程碑各自过审，合并前门禁红了 7 个）。`task.metrics` 事件与任务回包的
   `metrics` 被面板与 CLI 读取。
3. **测试替身要与真实接口对齐，不是与调用方对齐。** 夹具任务的门禁事件要用内核真实写事件的路径或同形载荷，不要手写一个只有
   测试认识的形状。
4. **负向对照照原文注入并核实会变红**（T-95 两条、T-98 BL-01 都按契约注入后并不变红）。验-1、验-4 都有指定的负向对照。
5. **改了模块，跑引用它的全部测试。** 按改动位置对应：
   - 改 `crates/<c>/src`：跑一次不带名字过滤的 `cargo test -p <c> --lib`；
   - 改提示词注入：跑 `cargo test -p automed --test prompt_rendering`；
   - 改 `scripts/ci/`：跑 `node --test scripts/test/*.test.mjs`。
6. **改了 `.rs` 就 `cargo fmt --all`**（T-97 一轮因一处换行被快检退回）。
7. **断言子进程输出时固定颜色环境**（T-97：会话里带 `NO_COLOR=1` 全绿、宿主上红）。
8. **实现会话多半不能监听端口、不能 `ps`。** T-98 的 29 份实现证据里有 24 份记了 EPERM。本任务大部分验收是 Rust 单测与
   `node --test`，不依赖这两样。遇到被拦时如实记录，不弱化测试、不加跳过。
9. **已关闭里程碑的「最新证据」指针不要动；rebase 之后跑受影响包的全量。**

## 10. 项目背景摘要（生成于 2026-11-17）

**这是什么。** Autome 2.0 是一个驱动 Loop 的本地工具：

- 需求走「设计 → 评审 / 裁决 → 实现 → 审计 → 复盘 → rebase → 合并前门禁 → 合并 → 清理」的 Loop。
- Bug 另有「定位 → 复现检查 → 修复 → 修复门禁 → 复核」的 Loop。

运作方式：

- 内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上带长度前缀的 JSON-RPC 服务面板。
- 命令行 `autome` 直接链接内核库，`autome core` 就是面板用的内核。
- 会话由内核在后台起。Claude Code 无 OS 沙箱；Codex 在 `workspace-write` 沙箱里。
- 门禁、快检、准备命令由内核在宿主上起（不在沙箱里），日志在 `<检出>/.autome/output/gates/<task>/`。

本仓库用 Autome 自己开发自己，`.autome/config.toml` 的现值：

- `[loop] parallel = 3`
- `[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`
- `quick = 'if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else …; fi'`、`quick_timeout_minutes = 20`
- `quick_session = 'sh scripts/ci-quick.sh --since {round_start}'`
- `[merge] auto = true`；`[retry] attempts = 2`

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  - `crates/autome-domain`：纯域，包括状态机、配置形状与校验、状态块解析、指标类型、门禁失败指纹、队列规则，不做 I/O。
  - `crates/automed`：内核，包括调度、JSON-RPC 分发、账本、门禁、会话启动与提示词、简报、会话后检查、Git、内置协议。
  - `crates/autome-cli`：`autome` 命令行。
- `apps/web`：Node（≥ 22.22）服务（`server/`）+ React Router 8 面板（`app/`，SSR）；vitest（分组见 `test-groups.ts`）、
  Playwright（`e2e/`）、eslint、tsc。
- `apps/desktop`：Electron 薄壳，`node --test`。
- `protocol/`：内置协议 seed（v9，编译进内核；本任务不改）。`plugins/`：Claude Code / Codex / Cursor 插件。
- `scripts/`：门禁（`ci-local.sh`、`ci-quick.sh`、`ci/`、`test/`）、安装与打包。
- `.github/workflows/ci.yml`：本仓库无远端，从未真正执行，步骤清单被 `apps/web/test/ci-config.test.ts` 钉住。
- 文档：
  - `README.md`：英文，Host gate 一节描述门禁、快检、沿用、准备命令。
  - `docs/development/{requirements,technical-design}.md`、`docs/plans/`、`docs/adr/`。
  - 每个任务的文档在 `docs/<slug>/`。

**测试与检查命令（在 worktree 根目录运行）。**

- Rust：
  - 全量：`cargo test --workspace --all-targets --no-fail-fast`。
  - 单个 crate：`cargo test -p automed --lib`、`cargo test -p autome-domain --lib`。
  - 内核端到端：`cargo test -p automed --test end_to_end <用例名>`。
  - 提示词快照：`cargo test -p automed --test prompt_rendering`。
  - 静态检查：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`。
- 门禁脚本：
  - `node --test scripts/test/*.test.mjs`；单个文件 `node --test scripts/test/ci-reuse.test.mjs`。
  - 只看决定不执行：`sh scripts/ci-local.sh --plan`、`sh scripts/ci-quick.sh --plan --since <rev>`。
- Web：
  - 新 worktree 先 `sh scripts/ci/install-node.sh apps/web`。
  - `npm run lint|typecheck|build|test --prefix apps/web`。
  - 拉真实内核的测试先 `cargo build -p automed -p autome-cli`。
- Desktop：`npm test --prefix apps/desktop`。
- 整套本地 CI（即宿主全量门禁命令）：`sh scripts/ci-local.sh`。门禁由宿主在合并前跑一次；实现轮 / 审计轮跑里程碑验收命令与直接
  相关的测试；宿主在实现轮 / 修复轮后跑快检。
- 输出长的命令重定向到 `.autome/output/`，只读头尾，例如
  `cargo test -p automed --lib > .autome/output/t.txt 2>&1; tail -20 .autome/output/t.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）规定：
  - 不改 `.autome/` 下任何内容；
  - 只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、不推送远端；
  - 完成本轮后结束会话，不自行启动下一个会话；
  - 超过 5 行的命令输出与临时验证产物写入 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/复盘轮拿到本任务指标-含门禁数据/protocol/`（见文首指路）。

**环境注意。** 运行环境是 macOS（Darwin），系统 `sh` 是 bash 3.2。

- `/tmp` 是 `/private/tmp` 的符号链接。
- macOS 对改名的二进制副本可能 SIGKILL。需要第二个名字时用符号链接，并先实测。
- launchd 起的进程只有最小 PATH。
