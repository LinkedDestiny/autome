# 任务：合并前门禁一次通过——浏览器用例每轮由宿主跑，门禁失败一次报全

本任务的规则见 `docs/合并前门禁一次通过/protocol/loop-protocol.md` 与 `docs/合并前门禁一次通过/protocol/session-protocol.md`。

- 设计文档：`docs/合并前门禁一次通过/合并前门禁一次通过.md`
- 任务卡（用户写的，第一输入）：`docs/合并前门禁一次通过/合并前门禁一次通过-card.md`
- 任务整理日期：2026-11-16
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `e82af068`（其前一个是 T-98 的合并提交
  `f314e4b3`，之后只有本任务的输入与协议固定两个提交）
- 本任务固定的协议版本：`protocol/v9`；账本 schema 现为 9（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）

## 1. 需求原文

> 合并前门禁一次通过：浏览器测试提前到每轮由宿主跑，门禁失败一次报全

任务卡正文比这句话具体；两者不一致时以任务卡为准。§2–§5 照任务卡整理（A-1…E-2、验-1…验-6 沿用任务卡编号），
§6–§7 是整理轮从调研得出的约束与待定问题，§8 是代码现状，§9 是前几个任务留下的教训，§10 是项目背景。

## 2. 背景与目标

T-98（13 个里程碑，11-15 至 11-16）全部里程碑审计通过之后，合并前门禁连续失败 6 次、第 7 次才过。数据取自账本
`gate.result` 事件与 `docs/面板收尾批次-用上新的内核数据-补齐备注/lessons.md`（L-01…L-11）：

- 合并前门禁 7 次共 7412 秒（约 2 小时 4 分），6 次失败全部在 `Web e2e` 这一步；每次失败各多花一个修复实现轮、一个审计、
  一个复盘（实现 #24–#29、审计 #16–#21、复盘第 2–7 轮）。
- 每轮快检 29 次共 4906 秒，其中 8 次失败、13 次超过 120 秒预算，中位数 116 秒，最长 603 秒。
- 6 次失败依次是：360 秒 `globalTimeout`、B-3 读到 0 个会话、夹具 `prepare()` 60 秒超时、D-1「运行中」列不可见、
  360 秒 `globalTimeout`（同第一次）、C-2 准备阶段 75 秒只凑到 17 个会话。

任务卡给出的四条根因（都是机制问题，不是 T-98 特有的）：

1. **浏览器用例在合并前门禁才第一次完整运行。** Codex 实现会话在 `--sandbox workspace-write` 下运行
   （`crates/automed/src/launcher.rs`），不能 `ps`、不能监听 127.0.0.1，29 份实现证据里 24 份记了 EPERM，Playwright 在
   globalSetup 就停下。审计只跑本里程碑点名的 spec 子集。整套 29 条用例第一次一起跑就是合并前门禁。
2. **门禁一次只暴露一个问题。** Playwright 配置 `workers: 1`、`retries: 0`、没有 `maxFailures`，被 `globalTimeout` 截断时
   报告是「N 通过、M 未执行、2 个非用例错误」，修复轮拿到的日志尾部没有这些错误的详情，于是一次修一个、往返六次。
3. **实现轮拿到的快检命令没有起点。** 简报里是不带参数的 `sh scripts/ci-quick.sh`，脚本支持 `--since <rev>`，7 份证据写了
   「未提供本轮起点，未跑改动相关的测试」。会话内只跑了静态检查，相关测试等宿主快检才第一次运行，8 个实现轮只用来修
   宿主快检抓到的失败。
4. **审计见过的失败被放行。** 第二、三次门禁失败的位置，审计 #4（BL-13，B-3 四次里两次读到 0 个会话）与审计 #14
   （`prepare()` 60 秒 ETIMEDOUT）都已经看到，分别按「测试竞态」「环境限制」通过。

**目标。** 让合并前门禁在正常情况下一次通过：浏览器用例在每轮就由宿主跑、失败当轮退回；门禁失败时一次给全材料；
实现轮快检带本轮起点；审计对门禁用例的放行口径由内核机械核对；顺带补齐 T-98 留下的两条测试补强。
**只做机械保证，不靠提示词要求会话自觉。**

## 3. 任务要求（任务卡「要做什么」逐条照录；设计文档的里程碑必须全部覆盖）

**总要求（任务卡）。** 一次设计、一次评审，按下面 A–E 分组里程碑实现。只做机械保证，不靠提示词要求会话自觉。

### 3.1 A 浏览器用例每轮由宿主跑

- **A-1** 实现轮 / 修复轮结束后，宿主在每轮快检之后按本轮改动跑相关的浏览器用例：改了 `apps/web/e2e/<x>.spec.ts` 就跑该文件；
  改了 `apps/web/e2e/harness/` 或 `playwright.config.ts` 就跑整套；改了 `apps/web/app`、`apps/web/server` 时怎样选用例
  由设计轮定（可以是里程碑验收命令里点名的 spec）。失败当轮退回给实现方，日志进简报。
- **A-2** 这一步有自己的预算，不计入快检的 120 秒；设计轮按 T-98 的实测耗时（审计 #21：单跑 C-2 共 3.9 分钟，其中用例外
  2.4 分钟）给出预算与超预算时的行为。通过记录进入现有的步骤沿用机制（T-97），合并前门禁在输入没变时沿用。
- **A-3** 整套浏览器用例的总耗时每次运行都写进门禁报告，并与 `globalTimeout` 比较；余量低于设计轮定的比例时该步骤判失败
  并写明「总耗时 X 秒，上限 Y 秒」。
- **A-4** 用例里专设的超时 / 轮询预算（`expect.poll` 的 timeout、`test.setTimeout`、`prepare()` 的超时）：报告里列出实测
  耗时超过预算一半的条目。

### 3.2 B 门禁失败一次报全

- **B-1** `Web e2e` 步骤失败时，交给修复轮的材料包含三类完整清单：失败用例（含断言与位置）、未执行用例、非用例错误（原文）。
  不再只给日志尾部。
- **B-2** 报告是「有未执行用例」且配置没有 `maxFailures` 时，直接标明「整套被总超时截断」，并给出各用例耗时与
  setup / teardown 耗时。
- **B-3** 合并前门禁失败后的第一个修复轮，简报里附上本任务 Backlog 与既往审计证据中所有提到门禁会运行的用例或 setup 步骤
  失败、超时的记录（按文件名与用例名匹配，机械提取）。

### 3.3 C 实现轮快检带起点

- **C-1** 简报与 `AUTOME_ROUND_START`（T-97 已导出给宿主）一致地把本轮起点交给会话，简报里的快检命令是
  `sh scripts/ci-quick.sh --since <本轮起点>`。
- **C-2** 宿主每轮快检的 120 秒预算：T-98 有 13 次超出、一次跑到 603 秒。设计轮查清超出的原因（冷缓存、相关测试选得太多、
  还是与其它门禁抢资源），给出修法或把预算改成实测值。

### 3.4 D 审计放行的口径（机械部分）

- **D-1** 审计证据里把门禁会运行的用例 / setup 步骤的失败或超时记为「环境限制」「测试竞态」时，内核在里程碑关闭前核对：
  只有权限拒绝（EPERM）一类可以放行，其余按退回处理。怎样机械识别由设计轮定；识别不了的部分如实写进 Backlog，不为此改
  协议文本。

### 3.5 E T-98 留下的两条测试补强

- **E-1**（T-98 BL-13）`controls.spec.ts` B-3 已改为有界轮询会话数，在宿主上真实浏览器复验一次并把结论写进证据。
- **E-2**（T-98 BL-14）`controls.spec.ts`「BL-01」用例在「相关表恒不匹配」变异下仍通过：让用例排除整页重验路径
  （或断言详情数据请求只带本路由），并补一条挂载 / 卸载 `TaskPage` 的组件测试，守住 effect 的清理函数。

## 4. 验收标准（任务卡原文）

- **验-1** 夹具任务：实现轮改坏一条浏览器用例，宿主在该轮结束后报出失败并退回，不等到合并前门禁。
- **验-2** 夹具任务：整套浏览器用例总耗时超过上限的设定比例时，步骤失败且报告写出总耗时与上限。
- **验-3** 构造一次被 `globalTimeout` 截断的运行：修复轮简报含失败、未执行、非用例错误三类清单，并标明「被总超时截断」。
- **验-4** 实现轮简报里的快检命令带 `--since <提交>`，该提交等于宿主记录的本轮起点。
- **验-5** E-1、E-2 各有测试或结论。
- **验-6** `sh scripts/ci-local.sh` 全部通过；浏览器用例输入没变时合并前门禁沿用每轮的通过记录。

## 5. 范围

**在范围内（按任务卡「参考」与调研得出；具体落点以设计为准）。**

| 项 | 可能改的地方 |
|---|---|
| A | `scripts/ci/{steps.sh,lib.sh,quick.mjs,inputs.mjs,summary.mjs}`、`scripts/ci-quick.sh`（或新增的宿主浏览器步骤脚本）、`scripts/test/`；`apps/web/playwright.config.ts`、`apps/web/e2e/harness/`、`apps/web/scripts/`（如 reporter / 预算采集）；内核侧若要新增一种宿主作业或在快检后串一步：`crates/automed/src/{scheduler.rs,gate.rs,gate_queue.rs,brief.rs,launcher.rs}`、`crates/autome-domain/src/{config.rs,gate_queue.rs}` |
| B | `scripts/ci/summary.mjs`（现只有 `cargo` / `vitest` 两种用例解析器）、Playwright 报告采集；`crates/automed/src/{gate.rs,scheduler.rs,launcher.rs,brief.rs}`（门禁结果 → 修复轮材料）；B-3 的机械提取读 `docs/<slug>/evidence/*-audit.md` 与设计文档 `## Backlog` |
| C | `crates/automed/src/{launcher.rs,brief.rs}`（`round_gate_line` 等把快检命令写进提示词 / 简报的地方）、`crates/automed/src/scheduler.rs`（`gate_round_env`）；C-2 的修法可能落在 `scripts/ci/`、`scripts/ci-quick.sh` |
| D | `crates/automed/src/guards.rs`（会话后检查）或审计结束后关闭里程碑的路径；域类型需要时 `crates/autome-domain/` |
| E | `apps/web/e2e/controls.spec.ts`、`apps/web/test/`（`TaskPage` 组件测试；新文件进 `apps/web/test-groups.ts`）、必要时 `apps/web/app/routes/tasks.$id.tsx` |
| 文档 | `README.md`（Host gate 一节说明新的宿主浏览器步骤与配置）；`docs/development/technical-design.md` 是否同步由设计轮定 |

**不做（任务卡「不做什么」，逐条照录）。**

- 不放宽 Codex 会话的沙箱（不改 `--sandbox workspace-write`）。会话跑不了的验收由宿主跑。
- 不改协议文本、不新增协议版本；复盘教训里 level 为 `protocol` / `rule` 的条目只取其中能机械保证的部分。
- 不改调度、合并队列、门禁时机（合并前门禁仍只在合并前跑一次）。
- 不动 `.autome/`；需要改 `.autome/config.toml` 的地方写进设计文档的「合并后由用户应用」一节。

**整理轮补充的边界（从调研得出，不扩大范围；带「整理轮理解」的是解释，设计轮可据证据修正）。**

- 「不改协议文本」指仓库根 `protocol/`（内核 `include_str!` 编译进去的 seed）与本任务的副本 `docs/合并前门禁一次通过/protocol/`，
  两处都不改。内核在 `launcher.rs` / `brief.rs` 里拼接的注入文字（门禁失败段落、快检说明行、简报小节）是代码，在范围内；
  改了它们会牵动 `crates/automed/tests/prompt_rendering.rs` 与 `crates/automed/tests/golden/`。
- 整理轮理解：「不改门禁时机」= 全量门禁仍是 `run = "before_merge"` 时只在 MergeGate 跑一次；A-1 新增的是每轮在快检之后
  的一步宿主检查（与快检同级），不是把全量门禁提前。
- 本项目的 `.autome/config.toml` 里 `quick` 是一条 `if [ -f scripts/ci-quick.sh ] … else … fi` 的 shell 命令，
  `quick_timeout_minutes = 10`。若新步骤需要新的配置键或改这条命令，写进「合并后由用户应用」，代码里对缺省值 / 旧配置保持
  兼容（本任务自己的运行仍按旧配置跑）。
- launchd 里的 `com.autome.loop`（驱动本仓库 Loop）在仓库之外，会话不得读写、装卸、kickstart。

## 6. 硬性约束

1. **分组与顺序（总要求）。** 里程碑按 A、B、C、D、E 分组，每组至少一个；实现轮只推进最小编号的开放里程碑，编号顺序即执行
   顺序，前面的里程碑的验收不得依赖后面才有的东西。每个里程碑有协议要求的三行验收契约（验收命令、预期、负向对照）。
2. **机械保证（总要求）。** 每一条都要由脚本或内核代码保证，并有测试钉住；不得以「在提示词里要求会话做 X」作为某条要求的
   实现。提示词 / 简报只用来**呈现**机械产出的材料（B-1、B-3、C-1）。
3. **不放宽沙箱、不改协议、不改门禁时机、不动 `.autome/`**（§5「不做」）。`launcher.rs` 里 Codex 的
   `autonomous_flags`（`exec --json --sandbox workspace-write`）不改；`protocol/`、`docs/合并前门禁一次通过/protocol/` 不改；
   账本 schema 若必须变，至多 9 → 10，不需要改表就不要改。
4. **不接触真实账本与真实安装。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`）驱动自己的 Loop，
   账本在 `~/.autome`。任何测试与手工验证都把 `AUTOME_HOME`（起面板时还有 `HOME`）指到临时目录；拉真实内核的测试**显式**
   指定 `<检出>/target/debug/automed` 或 `target/debug/autome`（缺省查找会落到 PATH 上已安装的另一个版本）；验-1、验-2、
   验-3 的「夹具任务」在测试自己建的临时仓库与临时账本里构造，不得把本 worktree、主检出或 `.worktree/` 下任何目录注册进测试账本。
5. **沙箱拦住时如实记录。** 实现会话多半不能 `listen(127.0.0.1)`、不能 `ps`（T-98 29 份实现证据里 24 份）。遇到时证据逐条列出
   没跑成的用例名与交给审计 / 宿主执行的完整命令；不改弱测试、不加跳过。静态检查（`cargo fmt`、`cargo clippy`、
   `npm run lint`、`npm run typecheck`）不受影响，当场跑。设计时让每个里程碑都有一部分验收不依赖监听与 `ps`
   （纯函数、解析器对固定日志样本、替身内核 / 替身命令）。
6. **没有条件跳过。** `apps/web/test/core-sidecar.test.ts` 的守卫扫 `apps/web/test/*.ts` 与 `apps/web/e2e/`，出现
   `test|it|describe.skip/skipIf/runIf/fixme/todo(` 就失败；平台前提用断言表达。Rust 侧不加 `#[ignore]`、不按环境变量提前返回。
7. **测试不留存活进程。** 起服务、内核、浏览器、假 CLI 的新测试用私有临时根目录，结束后用
   `apps/web/test/fixtures/process-scan.ts` 的 `leftoverProcesses(root)` 断言为空；e2e 走 `e2e/harness/teardown.ts`。
   宿主新步骤被超时终止时，要能拿下它起的整组进程（`gate.rs` 的作业已在独立进程组里，超时 `kill_group`）。
8. **步骤沿用机制的语义不放松（A-2、验-6）。** T-97 的沿用以内容寻址的键为准（`scripts/ci/inputs.mjs`：步骤名、命令、工具链、
   未声明路径、声明输入的 tree 条目）。每轮宿主浏览器步骤的通过记录要被合并前门禁沿用，必须保证二者在同一份输入上跑的是
   同一条命令（或设计给出等价性论证并有测试）；**只跑了子集的运行不得发布整套 `Web e2e` 的通过记录**（与现有
   「related tests never publish full passes」一致）。`gate.rs` 的 `read_report` 格式（`autome-gate-report 1` + 三列 TSV）
   若扩展，旧格式仍要能读。
9. **门禁步骤与测试分组的钉子。** `scripts/ci/steps.sh` 的每个 `step` 声明输入目录（`Web e2e` 现为
   `apps/web crates protocol plugins apps/desktop`）；`apps/web/test/ci-config.test.ts` 钉住 Web 各步骤写法与
   `.github/workflows/ci.yml` 的 job（含 `e2e` job 限时）；`scripts/test/ci-*.test.mjs` 钉住快检与沿用行为；新的 Web 测试文件
   必须出现在 `apps/web/test-groups.ts` 恰好一个分组（`unit` / `kernel` / `exclusive`，带内核、端口、进程的不得进 `unit`）。
   改这些文件的里程碑同轮同步对应测试。
10. **时间预算要算账（A-2、A-3、A-4、C-2）。** 现状：`apps/web/playwright.config.ts` `globalTimeout: 720_000`、
    `timeout: 90_000`、`expect.timeout: 10_000`、`workers: 1`、`retries: 0`、无 `maxFailures`；`e2e/harness/fixture.ts` 的
    `prepare()` 子进程超时 150 s（setup 里调用两次）；快检预算 `CI_QUICK_BUDGET` 缺省 120 s（`ci_quick_admit`
    超预算后把后续步骤记为「未跑」，不判失败）；本项目内核对快检整条命令的超时是 `quick_timeout_minutes = 10`；全量门禁
    `timeout_minutes = 30`。设计给出的每个预算都要写出依据（T-98 实测数据）与超出时的行为，并说明与上一层超时的关系
    （新步骤若放进快检命令内，10 分钟的内核超时同时约束它）。
11. **向后兼容。** 本任务分支自己的每轮快检仍由当前配置的 `quick` 命令驱动；T-97 之前切出的分支没有 `scripts/ci-quick.sh`。
    新机制在配置没改时不得让现有任务卡死或无限退回；新增配置键要有缺省值并通过 `crates/autome-domain/src/config.rs` 的校验。
12. **断言子进程文本输出时固定颜色环境**（spawn 时显式设 `NO_COLOR` / `FORCE_COLOR`），解析 Playwright / cargo / vitest
    输出前去掉 ANSI 转义（`summary.mjs` 已这样做）。
13. **脚本约定。** `scripts/*.sh`、`scripts/ci/*.sh` 为 `/bin/sh`（macOS bash 3.2）而写：不用数组、`[[`、`local`；中文紧跟
    变量时写 `${var}`。
14. **依赖。** 预计不需要新依赖（`@playwright/test` 1.64.1 已在，自带 `json` / `junit` reporter 与自定义 reporter 接口）；
    要加须说明理由，改 `package.json` 必须同轮提交 `package-lock.json`。
15. **文案语言沿用现状。** 界面、CLI 输出、报错、门禁日志、注入给会话的文字用中文；代码注释与 `README.md` 用英文；设计文档
    与 `docs/development/technical-design.md` 用中文。
16. **设计文档体量。** 协议警告线 ≤ 80KB、单轮增长 ≤ 10KB；证据写进 `evidence/`，`## 里程碑` 一节只放那一张表。
17. **会话边界（AGENTS.md）。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`
    （`.autome/output/` 是输出目录，可以写）；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；
    完成本轮后结束会话，不自行启动下一个会话。

## 7. 待设计轮回答的问题

只列问题与相关事实，不预设答案。设计文档「方案」一节须逐条给出结论与依据；没有依据支持改动任务卡字面的，按任务卡字面做。

### A

- **A-Q1 新步骤放在哪一层。** 选项至少有：(a) 放进 `scripts/ci-quick.sh`（项目脚本，内核不变，但受快检 10 分钟内核超时与
  120 s 预算机制的约束，A-2 要求它不计入 120 s）；(b) 内核在快检通过后再起一种宿主作业（新 `Launch` 种类 / 新配置键，
  走轻车道或重车道，`gate_queue.rs` 的车道规则要定）；(c) 其它。需说明：快检失败时是否还跑浏览器步骤；两者都失败时退回材料
  怎么合并；修复轮（Bug Loop 的 FixGate）是否同样跑；宿主跑浏览器时与另一任务的全量门禁并发抢 CPU 的影响（C-2 也问这个）。
- **A-Q2 `apps/web/app`、`apps/web/server` 改动时选哪些 spec。** 任务卡给了一个选项（里程碑验收命令里点名的 spec）；内核
  能否机械读到验收命令、读不到时的回退（整套 / 不跑 / 按 import 关系）。
- **A-Q3 与 T-97 沿用的衔接（约束 8）。** 每轮若只跑子集，什么时候跑整套、整套的通过记录怎样与 `Web e2e` 步骤共用同一把键；
  `Web e2e` 的输入含 `crates protocol plugins apps/desktop`，内核改动也会让它失效——验-6「输入没变时沿用」在本仓库的
  实际命中率要估一下。
- **A-Q4 预算与超预算行为（A-2）。** 依据：审计 #21 单跑 C-2 共 3.9 分钟（用例外 2.4 分钟、用例本身约 1.5 分钟）；T-98
  合并前门禁 7 次共 7412 s；第一、五次在旧的 360 s `globalTimeout` 处截断（现已是 720 s）。
- **A-Q5 总耗时与余量（A-3）。** 「总耗时」的口径（含不含 globalSetup / teardown、preflight、`npm run build`）；比例阈值；
  只跑子集时是否也比较。
- **A-Q6 专设预算的实测（A-4）。** `expect.poll` / `toBeVisible({ timeout })` / `test.setTimeout` / `prepare()` 的实测耗时
  怎样采集（自定义 reporter、`test.step`、harness 打点），报告落在哪（门禁日志、`AUTOME_GATE_REPORT`、单独文件）。

### B

- **B-Q1 三类清单的来源。** 现有 `list` reporter 的文本在被截断时缺非用例错误详情；可选 Playwright `json` / `junit` reporter
  或自定义 reporter 写结构化文件。三类清单的体量上限（`summary.mjs` 现有全局 20 条 / 2400 字节上限，`gate::tail` 60 行 /
  6000 字节）怎样定才能「完整」又不撑爆提示词——完整清单可放文件、提示词给路径加摘要，需定。
- **B-Q2 「整套被总超时截断」的判据（B-2）。** 「有未执行用例且无 `maxFailures`」是任务卡给的条件；各用例耗时与
  setup / teardown 耗时从哪来。
- **B-Q3 B-3 的机械提取。** 「门禁会运行的用例或 setup 步骤」的名单从哪来（`playwright test --list`、spec 文件名、harness
  文件名）；匹配哪些文本（设计文档 `## Backlog`、`evidence/*-audit.md`，是否含 `-impl.md`）；「第一个修复轮」怎么认定；
  这段材料进简报的哪一节、体量上限。

### C

- **C-Q1 起点的来源与一致性（C-1、验-4）。** 宿主快检的 `AUTOME_ROUND_START` 取自账本里最近一个同类 `session.started` 事件的
  `start_heads`（`scheduler.rs` `gate_round_env`，只在单仓库且节点是 `Gate` / `FixGate` 时导出）；会话启动时简报 / 提示词
  渲染发生在 `session.started` 写入之前还是之后、两者如何保证是同一个提交；工作区项目（多仓库）与旧会话没有 `start_heads`
  时怎么写。
- **C-Q2 配置命令与脚本命令的关系。** 提示词里的快检命令现在原样取配置的 `quick`（本项目是一条 if/else shell 命令）；
  「简报里的快检命令是 `sh scripts/ci-quick.sh --since <本轮起点>`」对配置了别的 `quick` 的项目怎样表述（内核不应假定所有
  项目都有这个脚本）。
- **C-Q3 120 s 超出的原因（C-2）。** 查 T-98 的 29 次快检日志（`.autome/output/gates/<task>/` 下，主检出里；若已清理则从
  账本 `gate.result` 的 `tail` 与 `log` 字段取），区分冷缓存（新 worktree 第一次 `npm ci` / `cargo build`）、相关测试选得
  太多（`quick.mjs` 的候选与 `vitest related`）、与其它门禁抢资源（轻 / 重车道各自并发）。结论可以是修法，也可以是把预算
  改成实测值；预算改动若涉及配置，写进「合并后由用户应用」。

### D

- **D-Q1 机械识别。** 审计证据是自由文本；怎样识别「门禁会运行的用例 / setup 步骤」+「失败 / 超时」+「环境限制 / 测试竞态」
  的放行，怎样识别 EPERM 一类可放行；识别不了的部分写进 Backlog（任务卡原话）。
- **D-Q2 「按退回处理」落在哪。** `guards.rs` 现有的会话后检查命中即为协议失败（停任务），不是退回；「退回」= 里程碑回到
  `开放` 并开下一个实现轮，需要新的内核路径还是复用现有的审计退回语义，需定；不得为此改协议文本。

### E

- **E-Q1** E-1 的「宿主上真实浏览器复验」由谁在什么时候执行（实现会话跑不了浏览器）：若 A 落地后由新的宿主步骤跑到，证据引用
  宿主日志即可；需定证据的写法。
- **E-Q2** E-2「排除整页重验路径」的具体判据（任务卡给了两种：排除整页重验，或断言详情数据请求只带本路由）；负向对照要在
  「相关表恒不匹配」变异下真的变红（T-98 审计 #12 实测旧用例在两种变异下都仍通过）。

## 8. 代码现状（整理轮调研，基线 `e82af068`；行号为约数，设计轮引用前自行复核）

**门禁脚本（`scripts/`）。**

- `scripts/ci-local.sh` 是全量门禁命令（本项目 `[gate] command`）；`--plan` 只看决定、`--all` / `CI_LOCAL_ALL=1` 强制全跑、
  `--check-claims` 拒绝未声明路径、`--package` 另跑打包。每个步骤都跑（前面失败不停），最后 `summary` 汇总。
- `scripts/ci/steps.sh` 声明步骤：`Format`、`Clippy`、`Test (unit, end to end, adapter flags)`（`--cases cargo`）、
  `Desktop install`（prep）、`Build core`（prep）、`Desktop`、`Contract`、`Scripts`、`Web install`（prep）、`Web lint`、
  `Web typecheck`、`Web build`、`Web test`（`--cases vitest`）、`Web e2e`（`npm run e2e --prefix apps/web`，没有用例解析器）。
  `GLOBAL_INPUTS` 与 `NO_GATE`（`docs .autome .claude AGENTS.md README.md`）也在这里。
- `scripts/ci/lib.sh`：`run`（tee 到临时文件，失败时按 `--cases` 解析器抽用例）、`summary`、`execute_step`（沿用 / 执行 /
  执行后键变了不记通过）、`ci_report`（写 `$AUTOME_GATE_REPORT`：`ran|reused|failed|not_run|partial \t key \t step`）、
  `ci_quick_admit`（快检超预算后后续步骤记「未跑（超出快检预算 Ns，留给合并前门禁）」）、`ci_web_related`
  （调 `apps/web/scripts/quick-related.mjs`）。
- `scripts/ci/inputs.mjs`：内容寻址的键（`ci-local-key 1` + 步骤名 + 命令 + 工具链哈希 + 未声明路径哈希 + 声明输入的 tree 条目），
  `AUTOME_GATE_PASSES` 提供本任务既往通过记录用于沿用。
- `scripts/ci/quick.mjs` `quickCandidates()`：起点取 `AUTOME_ROUND_START || AUTOME_BASE || CI_QUICK_SINCE`，都没有时给出
  「未提供本轮起点，未跑改动相关的测试」；按改动路径选 cargo 测试、`protocol/` 测试、`vitest related`、Desktop 全量。
  **不选任何 Playwright 用例。**
- `scripts/ci-quick.sh`：`--plan`、`--since <rev>`（导出 `CI_QUICK_SINCE`）、`--all`；`CI_QUICK_BUDGET` 缺省 120。
- `scripts/ci/summary.mjs`：`extract`（`cargo` / `vitest` 两种解析器）与 `render`（全局最多 20 条、2400 字节）。
- 脚本自身的测试：`scripts/test/ci-{claims,install,quick,reuse,summary}.test.mjs` 等，`node --test scripts/test/*.test.mjs`
  （门禁 `Scripts` 步骤）；`scripts/test/web-full-run-check.sh`、`web-full-benchmark.sh` 是 T-98 留下的整套浏览器计时脚本。

**浏览器用例（`apps/web/`）。**

- `playwright.config.ts`：`testDir: './e2e'`、`workers: 1`、`fullyParallel: false`、`forbidOnly: true`、`retries: 0`、
  `timeout: 90_000`、`globalTimeout: 720_000`、`expect.timeout: 10_000`、`globalSetup` / `globalTeardown` 在 `e2e/harness/`、
  `outputDir: '../../.autome/output/playwright'`、`reporter: 'list'`、`trace: 'retain-on-failure'`。无 `maxFailures`。
- `npm run e2e` = `node scripts/e2e-preflight.ts && playwright test`（需要先 `npm run build`，preflight 会报缺什么）。
- `e2e/*.spec.ts`：`controls.spec.ts`（B-1、B-4×2、B-2、B-3、BL-01）、`flows.spec.ts`（D-1、D-2）、`live.spec.ts`（D-3、D-4）、
  `manual-items.spec.ts`（W7 H-03、W7 H-04、W9 H-01、W9 H-02、C-2、C-2b 等）、`layout.spec.ts`（循环生成的
  `1512×944 <页面>`）；合计 29 条（任务卡口径）。用例里专设预算：多处 `toBeVisible({ timeout: 30_000–60_000 })`、
  `expect.poll(..., { timeout: 60_000 })`（B-3、BL-01）、C-2 的 `expect.poll(() => detail().sessions.length, { timeout: 150_000 })`
  （准备 22 个真实会话）。
- `e2e/harness/setup.ts`：建两个临时根（功能根 tick 100 ms、实时根 tick 3000 ms），各调一次 `prepare()`，起 `serve.ts`，
  15 s 等 `state.json`；启动前 `leftoverProcesses(root)`（实现会话里 `ps` 被拒时就在这里失败）。`fixture.ts` 的子进程超时
  30 s / 150 s。`teardown.ts` 收尾并检查残留。
- E-2 相关：`apps/web/app/routes/tasks.$id.tsx`（`TaskPage`，相关表 `setRelatedSubjects` 的登记与清理）；现有
  `test/task-page.test.ts`、`test/live-routes.test.ts`（承担相关表判别力的单测）；T-98 设计文档 `## 验收契约` 的 M-10 段落
  （约第 626 行）与 `## Backlog` 的 BL-13 / BL-14（约第 700–702 行）。

**内核（`crates/automed/src/`）。**

- `gate.rs`：`start_with_passes` 在独立进程组里用 `/bin/sh -c WRAPPER` 起门禁命令，先 `env_remove("AUTOME_ROUND_START")`、
  `env_remove("AUTOME_BASE")`，再设 `AUTOME_GATE_{COMMAND,LOG,PID,EXIT,PASSES,REPORT}` 与调用方给的 env；`poll_as` 超时后
  `kill_group`；`read_report` 解析 `AUTOME_GATE_REPORT`（≤ 256 KiB，首行 `autome-gate-report 1`）得出
  `steps{ran,reused,failed,not_run,unverified}` 与 `passes`；`tail(log, lines)`（≤ 6000 字节）；`log_window`（失败指纹，末 256 KiB）。
  `Launch` 种类有 quick / full / repro 等（`kind()`、`timeout()`）。
- `scheduler.rs`：`gate_round_env`（约第 4550 行）为单仓库的 `Gate` / `FixGate` 节点导出 `AUTOME_ROUND_START`（最近一个同类
  `session.started` 的 `start_heads`）与 `AUTOME_BASE`（merge-base）；`run_gate_attempt`（约第 4600–5000 行）起作业时附
  `AUTOME_TASK`、`AUTOME_HEAD`、`AUTOME_GATE_NODE`、`AUTOME_GATE_KIND` 与 `store.gate_passes(...)`，结束时取
  `gate::tail(&job.log, 60)` 写进 `gate.result`（含 `steps` / `passes`），`record_gate_result` 处理同因熔断
  （`same_failure_limit`）。
- `launcher.rs`：Codex 的 `autonomous_flags: ["exec","--json","--sandbox","workspace-write"]`（约第 242 行）；门禁失败注入
  （约第 520–620 行，「Autome 在提交 … 上运行了快检 `{command}` … 输出末尾（原文）」「… 运行了项目门禁 …」）；
  `round_gate_line`（约第 404 行）给实现轮写「宿主会在本轮结束后跑快检 `{quick}`」——`{quick}` 原样取配置命令，不带起点；
  `gate_line`、`gate_failure_verdict`。
- `brief.rs`：简报的「宿主门禁」小节（约第 372 行）、快检结果描述（约第 40–95 行）、`合并前门禁失败。命令：… 输出末尾：`
  （约第 981 行）。
- `guards.rs`：`check(round)` 按角色跑会话后检查（`evidence_file`、`no_closing_a_milestone`、`audit_moved_the_table`、
  `design_doc_size` 等），命中项按协议失败处理。
- `gate_queue.rs` 与 `crates/autome-domain/src/gate_queue.rs`：轻车道（quick、repro）/ 重车道（full、setup）两条 FIFO。
- `crates/autome-domain/src/config.rs`：`[gate]` 的 `command`、`run`、`quick`、`quick_timeout_minutes`、`timeout_minutes`、
  `concurrency`、`quick_concurrency`、`same_failure_limit`、`reuse_for_main_check` 等的形状与校验。
- 提示词快照测试：`crates/automed/tests/prompt_rendering.rs` + `crates/automed/tests/golden/`（差异核对后用
  `AUTOME_UPDATE_GOLDEN=1` 接受）；内核端到端 `crates/automed/tests/end_to_end.rs`。

**可用的实测材料（只读）。** `docs/面板收尾批次-用上新的内核数据-补齐备注/` 下的 `lessons.md`（L-01…L-11，尤其 L-02、L-05…L-11）、
`retro.md`、`evidence/`（M-xx-r<k>-impl / -audit，审计 #4、#6、#12、#14、#21 的耗时与放行记录）、`brief/`（实现 / 审计简报原样）；
T-97 文档 `docs/降低宿主门禁开销-输入没变的步骤不重跑/`（步骤沿用、`AUTOME_ROUND_START`、`AUTOME_GATE_REPORT` 的设计）；
T-67 文档 `docs/每轮快检-gate-quick-before/`。这些文档单份 20–100 KB，按节读，不要整份读。

## 9. 前车之鉴（来自已合并任务的复盘，逐条可核对）

1. **实现会话多半不能监听端口、不能 `ps`。** T-98 29 份实现证据 24 份记 EPERM，四次门禁修复都没在本地跑过浏览器（L-02）。
   本任务的 A 正是为此；在 A 落地前，本任务自己的浏览器验收同样只能靠审计轮与宿主。
2. **审计见过的失败别放行（L-06）。** 审计 #4 的 B-3 「4 次 2 次读到 0 个会话」、审计 #14 的 `prepare()` 60 s ETIMEDOUT 都在
   之后的门禁上原样复现。本任务的审计对门禁会跑的用例的失败 / 超时同样不得以「竞态」「环境」放行（D-1 是把它机械化）。
3. **门禁失败后一次修全（L-07）。** 修复轮先列出本任务 Backlog 与既往审计证据里所有相关失败记录，逐条修或写理由。
4. **预算要拿实测比（L-10、L-11）。** 把某处超时从 60 s 提到 150 s 要对总预算算账；用例专设预算是在没有实测的轮次定的，第一次
   有实测的审计没拿耗时去比预算。本任务每个新预算写出依据。
5. **耗时类验收不要凭一次超时退回。** T-96 的 M-10 一次 170.64 s 被退回、下一轮零改动 149.80 s 通过。
6. **偶发失败的结论要够次数。** 按观测失败率 p 取 n 使 (1-p)^n ≤ 0.05，或给出旧实现必红、新实现必绿的确定性回归。
7. **改了模块，跑引用它的全部测试。** 改 `crates/<c>/src` 的轮次跑一次不带名字过滤的 `cargo test -p <c> --lib`；改提示词注入
   跑 `cargo test -p automed --test prompt_rendering`；改 `scripts/ci/` 跑 `node --test scripts/test/*.test.mjs`；改
   `apps/web/app` 跑 `npx vitest related --run <改动文件…>`。
8. **改了 RPC 回包或事件形状，grep 全部使用方**（T-95：四个里程碑各自过审，合并前门禁红了 7 个）。`gate.result` 事件被面板
   （`apps/web/`）与 CLI（`crates/autome-cli/src/render.rs`）读取。
9. **断言子进程输出时固定颜色环境**（T-97：会话里带 `NO_COLOR=1` 全绿、宿主上红）。
10. **改了测试运行器配置，用不带过滤的入口真实启动一次**（T-97 改 vitest 分组后 `npm run test` 整体起不来）。改
    `playwright.config.ts` 同理。
11. **改了 `.rs` 就 `cargo fmt --all`**（T-97 一轮因一处换行被快检退回）。
12. **负向对照照原文注入并核实会变红**（T-95 两条、T-98 BL-01 都按契约注入后并不变红）。
13. **已关闭里程碑的「最新证据」指针不要动；rebase 之后跑受影响包的全量。**

## 10. 项目背景摘要（生成于 2026-11-16）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审 / 裁决 → 实现 → 审计 → 合并前门禁 → 合并」Loop 的本地工具（Bug 另有
「定位 → 复现检查 → 修复 → 修复门禁 → 复核」的 Loop）。内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上带长度前缀的
JSON-RPC 服务面板；命令行 `autome` 直接链接内核库，`autome core` 就是面板用的内核；多个驱动靠驱动锁轮流推进。会话由内核
在后台起（Claude Code 无 OS 沙箱；Codex 在 `workspace-write` 沙箱里）。门禁、快检、准备命令由内核在宿主上起（不在沙箱里），
日志在 `<检出>/.autome/output/gates/<task>/`。本仓库用 Autome 自己开发自己：`.autome/config.toml` 里 `[loop] parallel = 3`；
`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`；
`quick = 'if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else …; fi'`、`quick_timeout_minutes = 10`；
`[merge] auto = true`；`[retry] attempts = 2`。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：`crates/autome-domain`（纯域：状态机、
  配置形状与校验、状态块解析、门禁失败指纹、队列规则，不做 I/O）、`crates/automed`（内核：调度、JSON-RPC 分发、账本、门禁、
  会话启动与提示词、简报、会话后检查、Git、内置协议）、`crates/autome-cli`（`autome` 命令行）。
- `apps/web`——Node（≥ 22.22）服务（`server/`）+ React Router 8 面板（`app/`，SSR）；vitest 5（分组见 `test-groups.ts`）、
  Playwright 1.64.1（`e2e/`）、eslint、tsc。
- `apps/desktop`——Electron 薄壳，`node --test`。
- `protocol/`——内置协议 seed（v9，编译进内核；本任务不改）；`plugins/`——Claude Code / Codex / Cursor 插件；`scripts/`——
  门禁（`ci-local.sh`、`ci-quick.sh`、`ci/`、`test/`）、安装与打包；`.github/workflows/ci.yml`（本仓库无远端，从未真正执行，
  步骤清单被 `apps/web/test/ci-config.test.ts` 钉住）。
- 文档：`README.md`（英文，Host gate 一节描述门禁、快检、沿用、准备命令）、`docs/development/{requirements,technical-design}.md`、
  `docs/plans/`、`docs/adr/`；每个任务的文档在 `docs/<slug>/`。

**测试与检查命令（在 worktree 根目录运行）。**

- Rust：`cargo test --workspace --all-targets --no-fail-fast`；单个 crate `cargo test -p automed --lib`；内核端到端
  `cargo test -p automed --test end_to_end <用例名>`；提示词快照 `cargo test -p automed --test prompt_rendering`；
  `cargo fmt --all -- --check`；`cargo clippy --all-targets -- -D warnings`。
- 门禁脚本：`node --test scripts/test/*.test.mjs`；`sh scripts/ci-local.sh --plan`、`sh scripts/ci-quick.sh --plan --since <rev>`
  只看决定不执行。
- Web：新 worktree 先 `sh scripts/ci/install-node.sh apps/web`（或 `(cd apps/web && npm ci)`）；
  `npm run lint|typecheck|build|test --prefix apps/web`；单个文件 `npm run test --prefix apps/web -- test/<文件>`；相关测试
  `(cd apps/web && npx vitest related --run <源文件…>)`。拉真实内核的测试先 `cargo build -p automed -p autome-cli`。
- 浏览器端到端：`npm run build --prefix apps/web && npm run e2e --prefix apps/web`；单个文件
  `(cd apps/web && npx playwright test e2e/<文件>)`；`--list` 列出用例不执行。
- Desktop：`npm test --prefix apps/desktop`。
- 整套本地 CI（即宿主全量门禁命令）：`sh scripts/ci-local.sh`。门禁由宿主在合并前跑一次；实现轮 / 审计轮跑里程碑验收命令与直接
  相关的测试。
- 输出长的命令重定向到 `.autome/output/` 只读头尾，例如
  `cargo test -p automed --lib > .autome/output/t.txt 2>&1; tail -20 .autome/output/t.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、
  不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/合并前门禁一次通过/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；`/tmp` 是 `/private/tmp` 的符号链接；macOS 对改名的二进制副本可能
SIGKILL（需要第二个名字时用符号链接并先实测）；launchd 起的进程只有最小 PATH。
