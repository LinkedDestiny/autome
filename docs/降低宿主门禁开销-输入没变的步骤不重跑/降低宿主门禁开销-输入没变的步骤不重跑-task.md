# 任务：降低宿主门禁开销——Rust 测试一次报全、输入没变的步骤不重跑、每轮宿主跑改动相关的测试、Web 测试并行

本任务的规则见 `docs/降低宿主门禁开销-输入没变的步骤不重跑/protocol/loop-protocol.md` 与
`docs/降低宿主门禁开销-输入没变的步骤不重跑/protocol/session-protocol.md`。

- 设计文档：`docs/降低宿主门禁开销-输入没变的步骤不重跑/降低宿主门禁开销-输入没变的步骤不重跑.md`
- 任务卡（用户写的，第一输入）：`docs/降低宿主门禁开销-输入没变的步骤不重跑/降低宿主门禁开销-输入没变的步骤不重跑-card.md`
- 任务编号：T-97；分支 `autome/降低宿主门禁开销-输入没变的步骤不重跑`；固定协议 `protocol/v9`
- 任务整理日期：2026-11-14；整理时分支基于 main `a762d3f8`（T-95 合并提交），HEAD `f2e4c0b1`
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree，状态块不写 `repos:`）

下文把任务文档目录 `docs/降低宿主门禁开销-输入没变的步骤不重跑/` 简写为 `<doc_dir>`。

**怎么读这份文件。** §1–§5 是全任务的目标、总要求、范围与硬性约束；§6 按项（A、B、C、D）列出要做什么与验收标准，
每条带稳定编号（如 `A-R1`、`B-S3`、`C-V2`、`X-4`），评审意见、裁决、里程碑验收契约与审计结论都用这些编号追溯
（协议规定的人工验收清单条目 `H-01`、`H-02`… 是另一套编号）；§7 是项目背景；§8 是整理时核对过的代码事实；§9 是
任务卡明确交给设计轮定准的事项。任务卡保留了完整的背景与原始措辞——本文件对「要做什么」做了不改变含义的整理，
**任何一处与任务卡不一致，以任务卡为准**。

## 1. 需求原文

> 降低宿主门禁开销：输入没变的步骤不重跑、每轮在宿主上跑改动相关的测试、Web 测试并行、Rust 测试一次报出全部失败——一次设计、一次评审

## 2. 目标与背景

本仓库的宿主门禁（`.autome/config.toml` 的 `[gate]`）现在是：合并前跑一次完整门禁 `sh scripts/ci-local.sh`
（`run = "before_merge"`），每个实现轮 / 修复轮结束后跑一次快检（格式、Clippy、Web lint、Web typecheck）；合并后在
main 上的检查复用合并前门禁的结果，不再重跑。**这三点不改。**

2026-11-13 一天的实测（31 个任务合并，数字出自任务卡）：

| 项 | 次数 | 总用时 | 说明 |
|---|---|---|---|
| 完整门禁 | 54 | 509 分钟 | 其中 23 次失败，占 204 分钟 |
| 快检 | 81 | 23 分钟 | 平均 17 秒 |
| 合并后 main 检查 | 28 | 0 | 全部复用 |

单次完整门禁在三个任务并行的负载下要 9–15 分钟：Rust 测试 5–7 分钟，Web 测试 3.5–8 分钟，其余步骤合计不到 2 分钟。
开销集中在四处，本任务各对应一项：

| 项 | 一句话 | 对应的开销 |
|---|---|---|
| A | Rust 测试一次报出全部失败 | `cargo test` 遇到第一个失败的测试目标就停。T-95 的合并前门禁失败 4 次，其中 3 次是「前一个目标失败，后面的没跑，修完再跑才看到下一个」 |
| B | 输入没变的步骤不重跑 | 改什么都跑全部步骤：只改 `apps/web/` 照样跑 5–7 分钟 Rust 测试，只改 `docs/` 也跑全套；失败修复后上次已通过、输入没变的步骤全部重跑 |
| C | 每轮在宿主上跑改动相关的测试 | 会话沙箱里跑不了的测试（Codex 沙箱不能监听 127.0.0.1、不能执行 `ps`），第一次真跑是在合并前门禁；23 次失败大部分属于这一类，一次失败的代价是 9–15 分钟门禁加一个实现轮、一个审计轮、一次复盘 |
| D | Web 测试并行 | `apps/web/vitest.config.ts` 是 `fileParallelism: false`，全部测试文件逐个跑 |

**目标：** 用一份设计文档、一次评审覆盖四项，按 **A → B → C → D** 的顺序实现并逐项审计通过，最后一次合并；在不降低
检查强度的前提下，降低一次完整门禁的墙钟时间与「合并前才发现失败」的次数。

## 3. 对设计轮、实现轮与审计轮的总要求（任务卡原文整理，编号 X-1 – X-7）

- **X-1 一份设计文档覆盖四项。** 里程碑按项分组、同一项的里程碑连续排列，标题以「A·」「B·」「C·」「D·」开头，
  **总数不超过 6 个**；每个里程碑都要能独立验收。实现顺序 A → B → C → D，后面的项不得成为前面验收的前提。
- **X-2 先量再改。** 设计文档给出改动前的基线表：`scripts/ci-local.sh` 各步骤在空闲与负载下的耗时，取自主检出
  `.autome/output/gates/` 下的真实日志与设计轮自己的实测；每项给出预期收益。实现完成后在证据里给出同口径的改动后数据。
- **X-3 不降低检查强度。** 判定标准：合并到 main 的每个提交，`ci-local.sh` 的每个步骤都在「与该提交等价的输入」上
  通过过。设计文档逐步骤论证输入声明没有漏项，并说明漏项时的兜底（B-S1、B-S2、B-S5）。
- **X-4 与并行任务 T-96 的衔接。** T-96（面板交付批次，分支 `autome/面板交付批次-w10-启动与安装`）正在实现，它会给
  `scripts/ci-local.sh` 增加一个浏览器步骤并改 `.github/workflows/ci.yml`。本任务的步骤声明方式要让「新增一步」只需要
  写出它的命令和输入；谁后合并谁在 rebase 时对齐。
- **X-5 `.autome/` 不由会话修改。** 需要调整 `.autome/config.toml`（`[gate]` 的 `command` / `quick`）时，设计文档与
  最终证据里给出建议的配置原文，由操作者在合并后手工应用。在那之前，**旧配置配新脚本、新配置配旧脚本都不能出错**：
  `sh scripts/ci-local.sh` 不带参数的行为保持「全部步骤都被覆盖」（每一步要么执行、要么有据可查地沿用，不存在
  不带参数就少检查一步的情况）。
- **X-6 不接触真实账本。** 测试与实测一律用临时 `AUTOME_HOME`（内核测试同时设 `AUTOMED_DB_PATH`，见 §8 第 12 条）；
  不在 worktree 里不带临时 `AUTOME_HOME` 地跑 `autome` / `automed` / 面板；不跑 `sh scripts/ci-local.sh --package`。
- **X-7 会话跑不了的如实写明。** 会话沙箱里跑不了的验收命令（监听端口、`ps`、起真实内核）如实写进证据，由宿主门禁
  复验，不因此改结论、不因此改写验收条件。

## 4. 全任务的不做什么与验收标准

**不做什么（N-1 – N-6）：**

- N-1 不改门禁的时机：仍然是每轮快检 + 合并前一次完整门禁；合并后的 main 检查继续复用结果。
- N-2 不删减任何测试，不加条件跳过，不放宽 Clippy / lint 规则。
- N-3 不改门禁并发数与合并队列（T-70）、轻量检查车道（T-95 的 E 项）的调度。
- N-4 不做跨机器或远程缓存；通过记录只在本机。
- N-5 不改 `.autome/`。
- N-6 内核侧只做对任何项目都成立的事（导出环境变量、保存与展示结果）；本仓库特有的步骤划分留在 `scripts/` 里。
  没有配置快检的项目、没有配置门禁的项目，行为与现在一致。

**验收标准（V-1 – V-7，任务卡「验收标准」原文，各项细目见 §6）：**

- V-1（A）造两个测试目标各有一个失败用例的夹具（或等价的脚本级测试），一次门禁的汇总里两个都列出；汇总含失败用例名。
- V-2（B）用脚本级测试覆盖：只改 `apps/web/` 下一个文件时 Rust 测试步骤标为沿用；只改 `crates/` 下一个文件时受影响
  的步骤（含 Web 测试）全部执行；只改 `docs/` 时只执行声明了该输入的步骤；改 `scripts/ci-local.sh` 时全跑；顶层新增
  未归属路径时全跑且守卫测试失败；强制全跑开关有效；在 worktree 内伪造通过记录不被采信。
- V-3（B）失败 → 修一个文件 → 再跑，第二次只执行输入变了的步骤；日志逐步骤写明执行 / 沿用。
- V-4（C）e2e（真实内核 + 假 CLI）：实现轮改动一个测试文件并让它失败，快检当轮退回，失败摘要带用例名；本轮没碰的
  测试不跑；超预算时写明哪些没跑。未配置快检的项目行为与现在一致。
- V-5（D）Web 测试步骤空闲机器墙钟时间不超过改动前的 50%（证据给出前后各 3 次的数据）；连续 5 次全绿；无残留进程；
  ENOTEMPTY 一项有结论。
- V-6 设计文档有改动前基线表，最终证据有同口径的改动后数据；给出建议的 `.autome/config.toml` `[gate]` 原文。
- V-7 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`sh scripts/ci-local.sh`（强制全跑）
  全绿。

## 5. 范围与硬性约束

### 5.1 在范围内

- `scripts/ci-local.sh`（步骤声明、输入哈希、沿用判定、汇总），以及设计轮认为需要的 `scripts/` 下新文件。
- `.github/workflows/ci.yml`（A 项的 `--no-fail-fast`；B-S5「GitHub CI 始终全跑」；与 D 项的 Web 测试命令保持一致）。
- 内核 `crates/automed`、`crates/autome-domain`、`crates/autome-cli`：给门禁 / 快检命令导出环境变量，保存与展示
  「执行 / 沿用」结果（事件字段只增不减），失败摘要带用例名。以设计轮确定的最小改动为准。
- `apps/web/`：`vitest.config.ts` 与测试文件的并行改造（D）；任务详情时间线上门禁条目的展示（B-R6）；钉住
  `ci-local.sh` / `ci.yml` 文本的既有测试随之更新（§8 第 5 条）。
- 文档：`docs/development/technical-design.md` §8.1「两级门禁」与 §16、`README.md` 里 `ci-local.sh` 的说明，行为变了
  就同步。
- 本任务新增的脚本级测试、守卫测试与 e2e。

### 5.2 硬性约束（下文引用时写「约束 n」）

1. **不改 `.autome/` 下任何内容**（`AGENTS.md`）。配置调整只给建议原文（X-5）。
2. 只在当前 worktree 内工作：不切分支、不操作 `.worktree/` 下其它目录（包括 T-96 的检出）、不推送远端；完成本轮后
   结束会话，不自行启动下一个会话。主检出的 `.autome/output/gates/` **只读**用于基线统计（X-2）。
3. 超过 5 行的命令输出、临时验证产物写入（本 worktree 的）`.autome/output/`，不进 `<doc_dir>`。
4. 不接触真实账本（X-6）。
5. 不删测试、不加条件跳过、不放宽规则（N-2）。`apps/web/test/core-sidecar.test.ts` 有一条「测试源码不按条件跳过真实
   内核用例」的守卫（§8 第 9 条），D 项不得绕开或削弱它。
6. 旧配置配新脚本、新配置配旧脚本都不能出错；`sh scripts/ci-local.sh` 不带参数时全部步骤都被覆盖（X-5）。
7. 通过记录只由宿主门禁进程写入，放在任务会话不会写到的位置，不在 worktree 里（B-S3）。
8. 事件、RPC 回包的字段只增不减；`READ_METHODS` 等白名单若要动，`apps/web/server/gates/` 与 `apps/desktop` 的对应
   测试按现有的子集关系保持通过。
9. 若需要改协议正文 / prompt（`protocol/`），整个任务合成**一个**新版本，带独立的 `## protocol/vN` 标题，照
   `protocol/README.md`「怎么改」执行，旧协议版本的在跑任务不得因此协议失败；若需要改账本表结构，整个任务只加一次
   `SCHEMA_VERSION`。任务卡没有要求这两样，能不动就不动。
10. `<doc_dir>/protocol/` 是本任务固定的协议副本，会话只读。

## 6. 逐项要求

### 6.A Rust 测试一次报出全部失败

要做什么：

- **A-R1** `scripts/ci-local.sh` 的 `cargo test` 带 `--no-fail-fast`，一次门禁报出所有失败的测试目标。
- **A-R2** `.github/workflows/ci.yml` 对应步骤同样处理。
- **A-R3** 门禁日志末尾的汇总除了步骤级的 ok / FAILED，再列出失败的测试目标与用例名——**Rust 与 Web 都列**——让
  实现轮简报里的失败摘要一眼看全，不用翻几千行日志。（简报里的失败摘要取自日志末尾若干行，且内核的失败指纹也解析
  同一份日志，见 §8 第 3、4 条；汇总的格式要同时照顾这两个读者。）

验收：

- **A-V1**（= V-1）两个测试目标各有一个失败用例的夹具（或等价的脚本级测试），一次门禁的汇总里两个都列出，汇总含
  失败用例名。

### 6.B 输入没变的步骤不重跑

目标：一次完整门禁只执行「输入相对上一次通过发生了变化」的步骤；其余步骤标为「沿用」，并写明沿用的是哪一次通过。

要做什么：

- **B-R1 步骤声明输入。** 每个步骤声明自己的输入：路径集合，加上命令本身与工具链版本。任务卡给的例子（具体划分
  由设计轮按代码事实定，§8 第 5–8 条有整理时核对到的跨目录依赖）：
  - Rust 测试：`crates/`、`Cargo.toml`、`Cargo.lock`、`protocol/`（seed 是 `include_str!` 进去的）；
  - Web 测试：`apps/web/`，以及内核源码（`*live*` 用例起真实内核）；
  - Desktop 步骤：`apps/desktop/` 和它解析的 `crates/automed/src/dispatch.rs`。
- **B-R2 通过记录按内容寻址**：输入路径的 git tree 哈希 + 命令 + 工具链版本；不按时间、分支名或提交号。
- **B-R3 三种情形共用这一套机制**：只改一侧的任务跳过另一侧；失败修复后重跑只跑受影响的步骤；rebase 到新的 main
  之后，main 上已经通过的输入不重跑。
- **B-R4 日志与汇总逐步骤写明**：执行 / 沿用（沿用自哪次、输入哈希）/ 失败。
- **B-R5 新增一步的成本**：步骤声明方式要让「新增一步」只需要写出它的命令和输入（X-4）。
- **B-R6 时间线可见**：任务详情时间线上的门禁条目能看出这次跑了几步、沿用了几步。内核若需要给事件加字段，只增不减。
- **B-R7 实现位置由设计轮定**（`ci-local.sh` 内部、内核、或两者配合，例如内核给门禁命令导出基线提交的环境变量），
  但受 N-6 约束：门禁命令是项目配置的一条 shell 命令，内核不认识本仓库的步骤。

安全边界（**必须满足**）：

- **B-S1 未归属路径 → 全跑。** 不属于任何步骤输入、也不在「明确不需要门禁」名单里的路径发生改动时，整套门禁全跑。
  加一条守卫测试：仓库顶层新增目录或文件而没有归属时测试失败。
- **B-S2 门禁自身变化 → 全部失效。** `scripts/`、`.github/`、各 `package.json` / lockfile、门禁脚本自身的改动让全部
  步骤失效。
- **B-S3 通过记录的信任边界。** 通过记录只由宿主门禁进程写入，放在任务会话不会写到的位置（不在 worktree 里）；设计
  文档写明信任边界：会话伪造一条通过记录应当做不到，或者做了能被发现。
- **B-S4 构建产物不陈旧。** 沿用的步骤所产出、而别的步骤依赖的构建产物（例如 Web `*live*` 用例需要的内核二进制），
  在依赖它的步骤确实要跑时保证是当前源码构建的，不能用到旧产物。（T-95 复盘记过一次：`target/debug` 里 `autome` 比
  `automed` 旧，3 个用例假失败；现在 Web 测试能用到新二进制只是因为前面的 Rust 测试步骤顺带构建了它，见 §8 第 6 条。）
- **B-S5 强制全跑。** 提供强制全跑的开关（参数或环境变量）；GitHub CI 始终全跑。

验收：

- **B-V1**（= V-2）脚本级测试逐条覆盖：
  - a. 只改 `apps/web/` 下一个文件 → Rust 测试步骤标为沿用；
  - b. 只改 `crates/` 下一个文件 → 受影响的步骤（含 Web 测试）全部执行；
  - c. 只改 `docs/` → 只执行声明了该输入的步骤；
  - d. 改 `scripts/ci-local.sh` → 全跑；
  - e. 顶层新增未归属路径 → 全跑，且守卫测试失败；
  - f. 强制全跑开关有效；
  - g. 在 worktree 内伪造通过记录不被采信。
- **B-V2**（= V-3）失败 → 修一个文件 → 再跑，第二次只执行输入变了的步骤；日志逐步骤写明执行 / 沿用。
- **B-V3** 时间线上的门禁条目显示执行几步、沿用几步（B-R6；验收方式由设计轮定，须是会话能跑的测试）。

### 6.C 每轮在宿主上跑改动相关的测试

目标：把「合并前门禁第一次真跑才发现」的失败提前到当轮发现。

要做什么：

- **C-R1 快检加跑改动相关的测试。** 实现轮 / 修复轮结束后的宿主快检，在现有的格式、Clippy、lint、typecheck 之外，再跑
  本轮改动相关的测试：改到或新增的测试文件本身，以及能便宜地确定受影响的测试。Web 侧可用 vitest 的 related / changed
  能力；Rust 侧**至少**是改动所在 crate 的单元测试与被改动的集成测试文件，`end_to_end` 是否整套跑由设计轮按实测耗时定。
- **C-R2 保持「快」。** 给一个时间预算（设计轮实测后定；任务卡建议热缓存下中位数不超过 90 秒、上限 5 分钟）；超预算
  的部分留给合并前门禁，并在快检结果里写明哪些没跑。
- **C-R3 内核告诉快检「本轮改了什么」。** 给门禁 / 快检命令导出本轮起点提交与任务基线提交的环境变量（名字设计轮定；
  现有导出见 §8 第 2 条）。没有配置快检的项目行为不变。
- **C-R4 失败流程不变，摘要带用例名。** 快检失败退回当轮的现有流程不变；失败摘要里带上失败的用例名。
- **C-R5 与 B 的通过记录衔接。** 快检里已经通过、输入没再变的测试步骤，合并前门禁可以沿用——**前提是快检跑的是该
  步骤的全集而不是子集；子集的结果不能当作步骤通过。**

验收：

- **C-V1**（= V-4）e2e（真实内核 + 假 CLI）：实现轮改动一个测试文件并让它失败 → 快检当轮退回，失败摘要带用例名；
  本轮没碰的测试不跑；超预算时写明哪些没跑。
- **C-V2** 未配置快检的项目行为与现在一致（既有的 `gate_quick_*` 用例与「无 quick」分支逐事件不变）。
- **C-V3** 子集结果不被记成步骤通过（C-R5 的反面，设计轮给出可执行的验收）。

### 6.D Web 测试并行

要做什么：

- **D-R1 实测分组。** 实测全部 Web 测试文件里哪些必须串行（占端口、起真实内核、检查进程残留、改全局状态），其余
  并行。分成两组（例如两个 vitest project）：纯逻辑与组件测试并行，`*live*` 一类保持串行或按可隔离程度有限并行。
- **D-R2 目标。** 空闲机器上 Web 测试步骤的墙钟时间降到现在的一半以下；连续跑 5 次全绿；跑完没有残留进程（按
  `apps/web/test/logtail-live-cleanup.test.ts` 的检查方式）。
- **D-R3 查清 ENOTEMPTY。** T-95 第四次门禁的失败是 `test/eventbus-live.test.ts` 的 afterAll 清理临时目录时报
  ENOTEMPTY，根因没有定位，当时只加了有界重试（§8 第 10 条）。本项实测并行时把它查清楚：是谁在 afterAll 之后还往
  临时目录写；能定位就修根因并去掉重试，定位不了如实写明。
- **D-R4** 不允许用条件跳过来换时间（约束 5）。

验收：

- **D-V1**（= V-5）Web 测试步骤空闲机器墙钟时间不超过改动前的 50%，证据给出前后各 3 次的数据；连续 5 次全绿；无
  残留进程；ENOTEMPTY 一项有结论。

## 7. 项目背景摘要（2026-11-14 整理时）

### 7.1 这个仓库是什么

`autome-v2`：Autome 2.0——用一套固定协议驱动 AI 编码 CLI 会话，把一句话需求经「任务整理 → 设计 → 评审 → 裁决 →
实现 ⇄ 审计 → 复盘 → rebase → 门禁 → 合并」跑到合并提交；Bug 任务走「定位 → 复现检查 → 修复 → 修复门禁 → 复核」。
本仓库用自己的 Loop 开发自己（主检出里同时有三个任务并行，`[loop] parallel = 3`）。工作目录下只有这一个仓库。

### 7.2 技术栈

- Rust 工作区（`Cargo.toml`，edition 2024，`rust-toolchain.toml` 固定 `1.93.1`，带 rustfmt 与 clippy）：`serde` /
  `serde_json`、`rusqlite`（bundled SQLite）、`sha2`、`toml` / `toml_edit`、`uuid`、`libc`、`tracing`。没有异步运行时。
- `apps/web`：Node ≥ 22.22、React Router 8、React 19、Express 5、`ws`、Vite 8、**vitest `^5`**、TypeScript 6、
  ESLint 10（`--max-warnings 0`）。
- `apps/desktop`：Electron 43 外壳，测试用 `node --test test/*.test.js`。
- 门禁脚本：POSIX `sh`（`scripts/ci-local.sh` 以 `#!/bin/sh`、`set -u` 写成；宿主是 macOS，系统 bash 是 3.2）。
- 平台：macOS（产品只发 macOS；Git、worktree、进程相关测试依赖真实平台行为）。

### 7.3 布局

| 路径 | 职责 |
|---|---|
| `crates/autome-domain` | 纯类型与转换，无 I/O：配置叠加与校验（`config.rs`：`QuickGate`、`GateRun`、`gate_concurrency`、`gate_quick_concurrency`）、门禁失败指纹（`gate_failure.rs`）、门禁队列纯逻辑（`gate_queue.rs`）、状态机（`task.rs`）等 |
| `crates/automed` | 一切碰外部世界的：账本（`store.rs`）、调度器（`scheduler.rs`，约 1.3 万行，含 `run_gate` / `run_gate_attempt` / `record_gate_result`）、宿主门禁作业（`gate.rs`）、门禁排队（`gate_queue.rs`）、简报（`brief.rs`）、会话启动与 prompt 注入（`launcher.rs`，含 `gate_line`）、配置读写（`config_io.rs`）、JSON-RPC（`dispatch.rs`，含 `READ_METHODS`）、协议与 eval（`protocol/`） |
| `crates/autome-cli` | `autome` 命令（二进制名 `autome`；`automed` 是内核二进制） |
| `apps/web` | Node 网关 + RR8 面板；`test/` 下 110 个 `*.test.ts`（其中 17 个文件名含 `live`）、`test/fixtures/`；`app/components/task-timeline-model.ts` 是任务详情时间线的模型（含门禁条目） |
| `apps/desktop` | Electron 外壳；`test/` 下 14 个条目 |
| `protocol/` | 协议源（编译进 `automed`）：`loop-protocol.md`、`session-protocol.md`、`bug-protocol.md`、`prompts/*.md`、`brief-map.toml`、`evals/`、`CHANGELOG.md`（整理时最新 `## protocol/v9`） |
| `plugins/` | `loop`（1.x 插件）与 `autome` 插件 |
| `scripts/` | `ci-local.sh`（本地 CI = 完整门禁）、`package.sh`、`package-cli.sh`、`install-cli.sh`、`gen-eval-seed.py` |
| `.github/workflows/ci.yml` | GitHub CI（仓库没有远端，从未真正执行过；`ci-local.sh` 是它的本地对应物） |
| `.githooks/post-commit` | 主检出每次提交后在后台跑 `ci-local.sh`（`.worktree/` 下的检出直接退出；`NO_CI=1` 跳过） |
| `docs/development/` | `requirements.md` 与 `technical-design.md`——规格权威 |
| `docs/<slug>/` | 各任务的文档目录；`docs/adr/`、`docs/plans/` 是决定记录 |
| 仓库顶层其余 | `AGENTS.md`、`README.md`、`Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`、`.gitignore`、`.claude/skills/`、`.autome/`（只读） |

### 7.4 测试与检查命令（均在仓库根目录执行）

| 用途 | 命令 |
|---|---|
| 格式 | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| 全部 Rust 测试（门禁现状） | `cargo test --workspace --all-targets -- --nocapture`（T-95 复盘时共 12 个测试目标） |
| 单个集成测试目标 | `cargo test -p automed --test end_to_end <测试名>`、`--test prompt_rendering`、`--test real_cli`、`--test build_rerun`；`cargo test -p autome-cli --test cli <测试名>` |
| 单个 crate 的单元测试 | `cargo test -p <crate> --lib`（automed 库约 993 个、T-95 复盘实测 73 秒） |
| Desktop | `npm test --prefix apps/desktop`（先要有 `target/debug/automed`） |
| Web | `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm run test --prefix apps/web`（= `vitest run`）、`npm run build --prefix apps/web`；新 worktree 先 `cd apps/web && npm ci`；跑测试前先 `cargo build -p automed -p autome-cli` |
| 完整门禁 | `sh scripts/ci-local.sh`（Format、Clippy、Rust 测试、Desktop、Contract、Web lint / typecheck / test / build；每步都跑完再汇总，任一步失败退出码非 0）。**不要加 `--package`**（X-6） |

本项目的门禁配置（`.autome/config.toml`，只读）：

```toml
[gate]
command = "sh scripts/ci-local.sh"
run = "before_merge"
timeout_minutes = 30
quick = 'cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web'
quick_timeout_minutes = 10
```

另有 `[loop] parallel = 3`、`[merge] auto = true`（过了合并前门禁自动合并）。所以实现轮在会话里只跑里程碑验收与直接
相关的测试，加上 `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings`（命令与退出码写进证据）。

### 7.5 权威规则文件

- `AGENTS.md`（仓库根）：不改 `.autome/`；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、不推送；
  完成本轮后结束会话，不自行启动下一个；超过 5 行的命令输出与临时产物写入 `.autome/output/`。
- `.autome/rules/`：整理时只有一份 `README.md`（说明这个目录的用途），**没有具体的项目规则文件**。
- `docs/agent-project-profile.md`：不存在。
- `docs/development/requirements.md`、`docs/development/technical-design.md`：规格；§8.1「驱动锁、门禁…」里的
  「宿主门禁」「两级门禁」「合并前门禁模式」三段与 §16「测试与质量门」是本任务相关的小节，改了行为就同步。
- `protocol/README.md`「怎么改」与 `protocol/CHANGELOG.md`：协议改动的规矩（约束 9）。
- 本任务各轮的规则：`<doc_dir>/protocol/` 下的固定副本（见文件开头）。

## 8. 整理时核对过的事实（供设计参考，不是设计结论）

行号是 2026-11-14 本分支（HEAD `f2e4c0b1`）上的位置，会随改动漂移，以符号名为准。

1. **`scripts/ci-local.sh` 的结构（158 行）。** `run <步骤名> <命令…>` 函数（59–73 行）打印 `==> <步骤名>`，执行后
   打印 `    ok (Ns)` 或 `    FAILED (Ns)`，把步骤名记进 `passed` / `failed`（换行分隔）；末尾汇总逐行打印
   `  ok      <步骤名>` 与 `  FAILED  <步骤名>`，再打印「全部通过，用时 Ns。」或「有步骤未通过，用时 Ns。」并以 0 / 1
   退出。步骤依次是：`Format`、`Clippy`、`Test (unit, end to end, adapter flags)`（第 83 行，
   `cargo test --workspace --all-targets -- --nocapture`）、`Desktop install`（仅缺 `node_modules` 时）、`Desktop`、
   `Contract`（grep `dispatch.rs` 里的 `pub const READ_METHODS`）、`Web install`（仅缺 `node_modules` 时）、`Web lint`、
   `Web typecheck`、`Web test`、`Web build`；`--package` 另加 `Package`、`CLI package`、
   `Bundle carries a runnable core`。只认 `--package` 与 `-h` / `--help`，其它参数报错退出 2。
2. **内核怎么起门禁（`crates/automed/src/gate.rs`、`scheduler.rs`）。** `gate::start(job, cwd, command, env)`
   （`gate.rs` 约 168 行）用 `/bin/sh -c` 跑包装脚本，自己一个进程组，给包装脚本导出 `AUTOME_GATE_COMMAND`、
   `AUTOME_GATE_LOG`、`AUTOME_GATE_PID`、`AUTOME_GATE_EXIT`；调用处（`scheduler.rs` `run_gate_attempt`，约 4832 行）
   另外导出 **`AUTOME_TASK`（任务号）、`AUTOME_HEAD`（被检的 head）、`AUTOME_GATE_NODE`（节点名）、
   `AUTOME_GATE_KIND`（`quick` / `full` / `repro`）**。**整理时没有导出任务基线提交或本轮起点提交。** 作业文件在
   **主检出** `<项目路径>/.autome/output/gates/<任务>/<节点>-<head 前 12 位>.{log,pid,exit,started,kind}`
   （`gate::Job::at`、`dir_for`）；`cwd` 是任务 worktree（`MainCheck` 用单独的 detached 检出）。`session.started`
   事件里有 `scope_heads`（会话开始时各仓库的 head，约 4014 行），`layout.rs` 的 `RepoSlot.base` 是任务基线分支。
3. **失败摘要怎么进简报。** `gate.result` 事件的载荷是 `{node, sha, ok, exit_code, kind, command, log, tail}`，失败时
   再加 `fingerprint`、`fingerprint_summary`、`same_failures`、`tripped`（`record_gate_result`，约 4985 行）。`tail` 是
   `gate::tail(&job.log, 60)`：**日志最后 60 行，且不超过 6000 字节**（`gate.rs` 约 412 行）。失败退回实现轮 / 修复轮
   时注入 `GateFailed{head, command, tail}`，`launcher.rs`（约 553–564 行）把 `tail` 原文放进 prompt；
   `brief.rs` 的 `gate_evidence` / `gate_line_of_result` 生成简报里的门禁一行。所以「汇总写在日志末尾」是失败用例名
   进简报的现成通道，但要挤进 60 行 / 6000 字节。
4. **失败指纹解析的就是 `ci-local.sh` 的日志格式。** `crates/autome-domain/src/gate_failure.rs` 的 `fingerprint`
   （约 158 行）读日志最后 256 KiB（`gate::log_window`）：去掉首尾空白后以 `FAILED` 加空白开头、且后面不是 `(` 的行
   被当作**失败步骤名**；`==> <名字>` 行划分步骤段，只取失败步骤段内的关键行（`test … FAILED`、含 error / panicked
   等词的行）。同因连续失败会熔断（`same_failures` / `tripped`）。**汇总里新增的行（失败用例清单、「沿用」标记）如果
   以 `FAILED` 开头或改了 `==>` / 汇总行的格式，会改变指纹**——要么保持兼容，要么连指纹解析与它的测试
   （`crates/autome-domain/tests/gate_failure_window.rs`、`t14_gate_logs.rs`、`tests/fixtures/gate-t14/`）一起改。
5. **有测试逐字钉住 `ci-local.sh` 与 `ci.yml` 的文本。** `apps/web/test/ci-config.test.ts` 读这两个文件并断言：Web job
   的 run 步骤序列恰为 `npm ci`、`npm run lint`、`npm run typecheck`、`cargo build -p automed -p autome-cli`、
   `npm test`、`npm run build`；`ci-local.sh` 里 `Web install` 的三行原文；web 段四行恰为
   `run 'Web <name>' npm run <name> --prefix apps/web`；`cargo test --workspace --all-targets` 出现在
   `run 'Web test'` 之前（「本地门禁在 Web test 之前已构建内核」）；`CLI package` 片段的行为。改脚本结构就要同步改
   这份测试，且不能把它改成不再有鉴别力。`crates/automed/src/launcher.rs`（约 2719、2752 行）的单测里也出现
   `scripts/ci-local.sh` 字样（作为门禁命令样例）。
6. **Web 测试、Desktop 测试用的内核二进制来自 `target/debug`。** `apps/web/test/` 里驱动真实内核的用例默认跑
   `<worktree>/target/debug/autome` 与 `automed`（可用 `AUTOME_BIN` / `AUTOMED_BIN` 覆盖），并传临时 `AUTOME_HOME`；
   `apps/desktop/src/sidecar.js`（约 48 行）同样取 `target/<profile>/automed`。`ci-local.sh` 里没有单独的构建步骤，
   靠前面的 `cargo test --workspace --all-targets` 顺带构建；`ci.yml` 的 web / desktop job 则显式
   `cargo build -p automed [-p autome-cli]`。Rust 测试步骤一旦被沿用，这个隐含的构建就没了（B-S4）。
7. **Web 测试的输入不止 `apps/web/` 与内核源码。** `apps/web/test/` 下的测试还直接读：`scripts/ci-local.sh`、
   `.github/workflows/ci.yml`（`ci-config.test.ts`）、`scripts/package-cli.sh`、`scripts/install-cli.sh`、`README.md`
   （`install-alignment.test.ts` 等）、`protocol/bug-protocol.md`、
   `plugins/autome/skills/autome-task/scripts/check-card.sh`、`crates/automed/src/dispatch.rs`（`read-gate.test.ts`
   等 4 处）、`crates/autome-domain/src/task.rs`、`role.rs`、`bug_loop.rs`。整理时用
   `grep -rhn "crates/\|protocol/\|scripts/\|docs/\|\.github\|README\|plugins/" apps/web/test/*.ts` 得到，设计轮
   逐个核对后再定输入声明。
8. **Desktop 与 Rust 的跨目录输入。** `apps/desktop/test/ipc-gate.test.js`、`packaging.test.js` 读
   `crates/automed/src/dispatch.rs`。Rust 侧：`crates/automed/src/protocol/` 用 `include_str!` 编入 `protocol/` 下的
   协议文件与 `protocol/evals/`；`crates/automed/build.rs` + `build_support.rs` 生成构建标识（`tests/build_rerun.rs`
   测它的重跑条件）；`prompt_rendering.rs` 读 `crates/automed/tests/golden/`；`crates/autome-cli/tests/cli.rs`
   （约 1702 行）与 `automed/src/build_id/tests.rs` 用 `CARGO_MANIFEST_DIR` 定位文件。Rust 测试是否还读 crate 目录
   之外的其它路径（例如 `scripts/`、`README.md`、`docs/`）由设计轮逐个核对。
9. **Web 测试现状。** `apps/web/vitest.config.ts` 全文只有
   `{ test: { testTimeout: 60_000, hookTimeout: 60_000, fileParallelism: false } }`。`apps/web/test/` 顶层 112 个条目：
   110 个 `*.test.ts`、`fixtures/` 目录、`web-typecheck-regression.mjs`（任务卡写的「112 个测试文件」按 vitest 实际
   收集数核对；T-95 第四次门禁日志记的是 110 个测试文件、833 个用例）。文件名含 `live` 的 17 个：`board-live`、
   `board-live-listen`、`board-live-timing`、`eventbus-live`、`live-banner`、`live-client`、`live-routes`、`live-send`、
   `live-status`、`logtail-live`、`logtail-live-cleanup`、`notifier-live`、`notifier-live-fixture`、`older-kernel-live`、
   `shell-live`、`task-live-real`、`task-log-live`——**名字含 `live` 不等于起真实内核**（如 `live-banner`、
   `live-client`），另有不带 `live` 的 `*-real.test.ts`、`gateway-http`、`uploads-http`、`core-sidecar`、
   `core-supervisor` 等也引用 `target/debug` 二进制或监听端口；必须串行的集合要按 D-R1 实测，不按文件名猜。
   `core-sidecar.test.ts`（约 415 行）有守卫：测试源码里出现 `test|it|describe.skipIf|runIf|skip|todo(` 即失败。
   `logtail-live-cleanup.test.ts` 的检查方式：给子 vitest 进程一个私有 `TMPDIR`，子套件退出后等 2 秒，用
   `ps -axo pid=,ppid=,command=` 找命令行含该目录的进程及其后代，要求为空。
10. **ENOTEMPTY 的现场。** `apps/web/test/eventbus-live.test.ts` 的 afterAll（约 52–61 行）：终止 WebSocket、给子进程
    发 SIGTERM（5 秒后 SIGKILL）并等它退出，然后
    `await rm(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 })`，注释写「macOS can transiently
    report ENOTEMPTY…」。该测试经 `test/fixtures/automed-pidfile.sh` 起真实 `automed`（`W3_REAL_AUTOMED`），`HOME` 与
    `AUTOME_HOME` 都指向这个临时目录。`docs/内核补齐批次-2026-11-13-4/lessons.md` 的记录：重新构建后连跑 3 次 7/7
    通过，每次跑完 `pgrep` 看不到残留的 `target/debug/automed`，「内核守护进程活过 afterAll 并继续写临时目录」这个
    猜测不成立，写入者未定位。
11. **快检现状（C 项的落点）。** `scheduler.rs` `run_gate_attempt`：需求任务的 `Node::Gate` 与 Bug 任务的
    `Node::FixGate` 在 `resolved.quick_gate()` 有值时以 `Launch::Quick{command, timeout_minutes}` 启动（约 4628–4644
    行）；Bug 的命令是 `( 复现命令 ) && ( 快检命令 )`。`before_merge` 且没配 quick 时 `Node::Gate` 直接记
    `gate.skipped` 通过（约 4559 行）。合并前门禁沿用上一次结果的条件之一是 `last["kind"] != "quick"`（约 4697 行）
    ——**内核层面快检结果永不替代 MergeGate**，C-R5 的「沿用」只能发生在脚本的步骤级通过记录里。`launcher.rs`
    约 383–440 行（`gate_line` 及其后拼接门禁说明的代码）在 prompt 里写「宿主会在本轮结束后跑快检 `…`，失败会退回」。技术设计 §8.1「两级门禁」
    现有一句「快检建议只放格式、lint、类型检查并控制在 1 分钟内，测试留给合并前完整门禁」，`quick_timeout_minutes`
    缺省 5 分钟——C 项落地后这句建议要同步。既有用例：`scheduler.rs` 内 `gate_quick_host_*`、`gate_quick_bug_*`
    单测，`end_to_end.rs` 的 `gate_quick_feature_*`（约 2880 行起）与 `gate_quick_bug_*`（约 5106 行起），
    `config_io.rs` 的 `[gate] quick` 解析（约 487–520 行）。
12. **测试怎样隔离账本（X-6）。** `crates/autome-cli/tests/cli.rs` 的沙箱同时设 `AUTOME_HOME` 与 `AUTOMED_DB_PATH`
    到临时目录；`end_to_end.rs`（约 380 个函数）用真实 Git 仓库、真实 worktree、真实包装脚本，只把模型换成假 CLI
    （`crates/automed/tests/fixtures/fake_*.sh`）。C-V1 的 e2e 放在这里。
13. **时间线上的门禁条目（B-R6）。** `apps/web/app/components/task-timeline-model.ts`（约 13–110 行）由
    `gate.queued` / `gate.started` / `gate.result` / `gate.reused` / `gate.skipped` 事件拼出条目：标题来自
    `node` + `kind`（「快检」「修复门禁（复现 + 快检）」等），摘要是「运行中 / 通过 / 未通过 · 退出码 N / 超时」，
    `gate.reused` 显示「沿用已通过的结果」（这是**整次门禁**的沿用，与本任务步骤级的沿用是两回事，措辞上要分开）。
14. **真实门禁日志。** 主检出 `/Users/dannie/project/autome-v2/.autome/output/gates/` 下整理时有 70 个任务目录；例如
    `T-95/merge_gate-806910a9b823.log`、`T-95/gate-1f666e5864c4.log`（`gate-*` 是每轮的快检或旧模式的每轮门禁，
    `merge_gate-*` 是合并前门禁；同名的 `.kind` 记录启动种类，`.started` 是开始时间戳，`.exit` 是退出标记）。日志里
    每步有 `==> 步骤名` 与 `ok (Ns)` / `FAILED (Ns)`，可直接统计各步耗时。目录不入库，只读。
15. **T-96 的状态。** 分支 `autome/面板交付批次-w10-启动与安装` 存在；整理时
    `git diff --stat main...autome/面板交付批次-w10-启动与安装 -- scripts .github apps/web/vitest.config.ts apps/web/package.json`
    为空（它还没动这些文件）。后续各轮可用同一条命令（只读 git 对象，不进它的检出目录）看它走到了哪一步。
16. **`.githooks/post-commit`。** 主检出配了 `core.hooksPath=.githooks`：每次提交后在后台跑 `sh scripts/ci-local.sh`，
    日志在 `.git/ci-local/last.log`；`.worktree/` 下的检出直接退出，rebase / cherry-pick 重放时不跑，新的一次会杀掉
    上一次。它也是 `ci-local.sh` 的一个调用方（没有 `AUTOME_GATE_*` 环境变量），通过记录的读写规则要把它算进去。
17. **版本号。** `protocol/CHANGELOG.md` 最新 `## protocol/v9`；`crates/automed/src/store.rs` `SCHEMA_VERSION = 9`。
    本任务若确需发协议版本即 v10、若确需迁移即 10（rebase 时被占用则顺延）；见约束 9。

## 9. 任务卡明确交给设计轮定准的事项（清单，不是结论）

1. 各步骤的输入划分（B-R1）与「明确不需要门禁」的路径名单（B-S1）；逐步骤论证没有漏项（X-3）。
2. 通过记录放在哪、谁能写、怎么防伪或怎么发现伪造（B-S3）；`post-commit` 钩子与手工运行算不算可信写入方。
3. B 的实现位置：`ci-local.sh` 内部、内核、或两者配合；内核导出哪些环境变量、叫什么名字（B-R7、C-R3）。
4. 沿用步骤的构建产物怎么保证是当前源码构建的（B-S4）。
5. 强制全跑开关的形式（参数或环境变量）与 GitHub CI 始终全跑的落实方式（B-S5）。
6. 日志与汇总的格式：执行 / 沿用 / 失败、失败目标与用例名怎么排，才能既进 60 行的失败摘要、又不破坏失败指纹
   （A-R3、B-R4、C-R4；§8 第 3、4 条）。
7. 时间线怎么拿到「执行几步、沿用几步」：事件加什么字段、由谁提供（B-R6）。
8. 快检的时间预算、`end_to_end` 是否整套跑、Rust / Web 两侧「改动相关的测试」怎么定、超预算时怎么报「没跑的」
   （C-R1、C-R2）。
9. 快检里什么算「步骤全集通过」可供合并前门禁沿用，什么只是子集（C-R5）。
10. Web 测试怎么分组、`*live*` 一类是串行还是有限并行（D-R1）；ENOTEMPTY 的定位方法（D-R3）。
11. 建议的 `.autome/config.toml` `[gate]` 原文，以及新旧配置与新旧脚本四种组合都不出错的论证（X-5、V-6）。
12. 基线表的口径（空闲 / 负载各取哪些日志、实测几次）与实现后同口径复测的做法（X-2）。
13. 里程碑怎么分（不超过 6 个，按 A → B → C → D 分组，X-1）。
