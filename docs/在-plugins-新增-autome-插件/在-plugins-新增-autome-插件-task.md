# 任务：对话里整理任务卡并用命令行创建的 autome 插件（C1）

本任务的规则见 `docs/在-plugins-新增-autome-插件/protocol/loop-protocol.md` 与
`docs/在-plugins-新增-autome-插件/protocol/session-protocol.md`。

- 设计文档：`docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件.md`
- 任务整理日期：2026-11-11；任务 id T-17；基线提交 `1147ac0`
- 来源：用户的一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md` 第 6 节的 **C1「对话里创建任务卡的插件」**
  （任务卡模板见该文档 §3.1，看板列与「默认存为待开始」见 §3.2，三个来源见 §3.3）。

## 1. 需求原文

> 在 plugins/ 新增 autome 插件：讨论后把需求或 Bug 整理成任务卡、确认后用 autome 命令行创建，同时支持 Claude Code、Codex、Cursor

## 2. 目标与背景

**背景（参考文档 §1–§3 的要点）。** Autome 的任务卡有三个来源，落到内核里都是同一种任务卡：在 Claude Code
（或 Codex、Cursor）里和 Agent 讨论清楚后由它整理并创建；在网页上填写创建；其他 Agent 用 `autome` 命令行
`--json` 程序化创建。本任务做的是第一种，以及把第三种的用法写成文档。

> 任务卡 = 类型 + 标题 + 一份 Markdown 正文（下称「任务卡正文」）+ 可选附件。正文按类型有模板，三个来源都用同一套模板。
> 正文由内核写进任务的文档目录 `docs/<slug>/<slug>-card.md`……正文上限 64KB。

> 创建时可以选「创建并开始」或「存为待开始」。从 Claude 讨论里创建的卡，默认存为待开始，你在面板上看一眼再点开始。

参考文档 §3.3 对本来源的描述（逐字）：

> 讨论完说「建成 autome 任务」。插件里的技能按 §3.1 的模板整理任务卡正文，**先完整展示给你确认**，再执行
> `autome task new --brief - --kind bug|feature --draft -p <项目> --json`

> 其他 Agent：同一条命令行，`--json` 输出任务 id；`autome task start <id>` 开始。（C1 负责写进文档供 Agent 读）

**任务卡模板（参考文档 §3.1，逐字）。**

需求：

```markdown
# <标题>
## 背景            为什么要做
## 要做什么        具体行为，讨论中定下的细节逐条写
## 不做什么        明确排除的范围
## 验收标准        可检查的条目
## 参考            仓库内文件、链接、截图（附件）
```

Bug：

```markdown
# <标题>
## 现象            看到了什么
## 复现步骤        1. … 2. … 3. …
## 期望行为
## 实际行为        报错原文、日志片段
## 环境            版本、分支、配置
## 线索（可选）    怀疑的位置、最近的相关改动
```

（每行标题右侧的说明文字是给填写者的提示，不是标题的一部分；生成的正文里二级标题怎么写，由设计定，但必须
与模板的小节（需求五个、Bug 六个）一一对应、顺序相同。）

**目标。** 在 `plugins/` 下新增一个名为 `autome` 的插件，三个宿主（Claude Code、Codex、Cursor）共用同一份技能实现：
用户在对话里说「建成 autome 任务」之类的话时，技能把已讨论的内容整理成符合模板的任务卡正文、判断类型、确定项目，
完整展示并等用户确认后，调用 `autome task new … --json` 创建，回显任务 id。

## 3. 验收标准

以下六条逐字取自参考文档 C1 的「验收」。设计文档的里程碑必须覆盖每一条；用户在设计定稿时会对照这六条批准或驳回。

1. 技能按 §3.1 的模板生成正文，判断类型（需求或 Bug），**创建前把完整正文、类型、项目展示给用户并等待确认**；
2. 项目按当前目录推断；推断不出就列出 `autome project ls --json` 让用户选；
3. 默认 `--draft`，用户说「直接开始」才不加；创建后回显任务 id 和面板链接（`autome web` 在运行时）；
4. 没装 `autome` 时给出安装命令，不尝试别的方式创建；
5. 插件的 eval 用例：讨论信息不全时先追问，而不是编造验收标准；
6. README 写明 Agent 程序化创建的方法：命令行与 `--json` 的输出格式。

补充说明（从需求原文、参考文档和调研得出，不是新范围）：

- **「同时支持 Claude Code、Codex、Cursor」** 的含义与现有 `loop` 插件一致：同一个插件目录下放三份 manifest
  （`.claude-plugin/plugin.json`、`.codex-plugin/plugin.json`、`.cursor-plugin/plugin.json`），技能放在
  `skills/` 下由三个宿主共读，并登记进 `plugins/` 下已有的三份市场文件（见 §6）。不为某个宿主单独写一套行为。
- 第 1 条「完整正文」指将要通过标准输入交给 `--brief -` 的那份正文，原样、全文，不是摘要。用户要求修改时，改完后
  再次完整展示、再次等待确认；没有得到明确确认就不执行创建命令。
- 第 1 条的类型判断：类型只有 `feature`（需求）与 `bug` 两种；判断不出时问用户，不要默认。
- 第 2 条：当前命令行在不写 `-p` 时本来就按当前目录取已注册项目（`crates/autome-cli/src/client.rs::resolve_project`）。
  参考文档的命令里写了 `-p <项目>`，设计要决定技能是自行推断后显式传 `-p`，还是依赖命令行的推断，并保证
  「推断不出」时落到「列出 `autome project ls --json` 让用户选」这条路径上，且展示给用户确认的「项目」与实际创建
  用的是同一个。
- 第 3 条「面板链接（`autome web` 在运行时）」：见 §7 待定问题 5——`autome web` 子命令与 `~/.autome/web/server.json`
  属于尚未完成的 W10，任务详情页路由属于尚未完成的 W7。不得在面板未运行时编造链接。
- 第 5 条的「eval 用例」放在本插件自己的 `evals/` 下，格式沿用 `plugins/loop/evals/`（`case.yaml` + `graders/*.md`，
  `claude plugin eval` 的格式）。它是触发式、有成本的评估，不进每轮门禁（见约束 8）。
- 第 6 条的「README」指本插件的 `plugins/autome/README.md`；输出格式要以当前命令行的真实输出为准（见 §8「命令行事实」），
  并在实现轮对隔离账本实际跑一遍核对。

## 4. 范围

**改动目录：`plugins/`。**（参考文档 C1 的「目录」一栏。同一轮并行的 Web 任务改的是 `apps/web/`，越界会冲突。）

在范围内：

- 新建 `plugins/autome/`：三份 manifest、`skills/` 下的技能（数量与命名由设计定）、Claude Code 的短别名命令
  （若设计决定提供，沿用 `loop` 插件 `claude-commands/` 的做法）、`evals/` 下的 eval 用例、`README.md`；
  以及设计决定需要的确定性检查脚本（放在插件目录内）。
- `plugins/.claude-plugin/marketplace.json`、`plugins/.cursor-plugin/marketplace.json`、
  `plugins/.agents/plugins/marketplace.json`：各登记 `autome` 插件；
- `plugins/README.md`：从「目前只有一个插件」改为介绍两个插件，补上 `autome` 的安装与验证命令。

不在范围内（不要做）：

- `crates/`（命令行、内核）、`apps/`（桌面端、Web 面板）、`protocol/`、`scripts/`、`.github/`、`docs/development/`、
  `docs/plans/`、仓库根 `README.md`、`.autome/`；
- `plugins/loop/` 下的任何文件（它是另一个插件，本任务不改它的行为、版本或文档）；
- `autome web` 子命令、面板服务、`server.json` 的写入（W10）；网页上建卡（W6）；任务详情页（W7）；
- 给命令行加新参数、新输出字段，或用 `autome rpc` 绕过 `task new`；
- 附件上传、`task start` 之外的任务控制（批准、合并等）——技能只负责「整理并创建」；
- 顺手重构、改名、整理与本需求无关的文件。

## 5. 硬性约束

1. **只改 `plugins/`，且不动 `plugins/loop/`。** 若设计认为必须改范围外的文件，先在设计文档的「争议项」里提出，
   由用户在设计定稿时决定，不要自行越界。
2. **确认闸门。** 执行任何会创建任务的命令之前，必须已把「完整正文 + 类型 + 项目 + 是否待开始」展示给用户并得到
   明确确认。用户的修改意见、模糊回应（「差不多」「再看看」）都不算确认。技能文字里要把这一点写成不可跳过的步骤。
3. **唯一的创建路径是 `autome` 命令行的 `task new`。** 正文经标准输入 `--brief -` 传入，带 `--kind`、`--json`；
   不得用 `autome rpc task.create`、直接写 `~/.autome`、直接读写 SQLite、调用 HTTP 接口或任何其它方式创建。
   没装 `autome`（`PATH` 上找不到）时只给出安装命令并停下（验收 4）。
4. **默认待开始。** 默认带 `--draft`；只有用户明确说「直接开始」（或设计列出的等价说法）时才不带。不在创建后
   自动 `task start`。
5. **不编造。** 模板里用户没有提供、讨论中也没有定下的内容（尤其是需求的「验收标准」、Bug 的「复现步骤」
   「期望行为」）要追问，不得自行补写；可选小节（Bug 的「线索」）可以省略或写「无」，由设计定规则。
6. **正文要过内核校验。** 内核对 `brief` 的要求（`crates/automed/src/dispatch.rs` 约第 1340–1392 行）：非空、UTF-8、
   不超过 65536 字节；标题取正文里第一行以 `# ` 开头的行（没有就取第一行非空文本）。所以正文第一行必须是
   `# <标题>`，标题即任务标题。超长时技能要先告诉用户，而不是让命令行报错后再说。
7. **不得碰真实账本。** 驱动本仓库 Loop 的是已安装的 `~/.local/bin/autome`，它与 `~/.autome/state/automed.sqlite3`
   共用账本；`autome run` 正在常驻推进任务。实现与验证过程中**任何真实创建都必须用隔离的 `AUTOME_HOME`
   （临时目录）**，或在 `PATH` 前面放一个假的 `autome` 替身；绝不在真实 `~/.autome` 里创建、开始、取消任务。
   替身的输出必须与真实命令行一致——以对隔离账本跑出的真实输出为准，不要凭记忆写替身。
8. **eval 不进门禁。** `claude plugin eval` 是 early access、真实拉起 Agent、有成本。eval 用例写好即可，运行时必须带
   `--max-cost-usd` 上限；每轮的验证靠确定性检查（manifest 校验、脚本断言等），映射关系写进设计文档。
9. **三宿主共用一份实现。** 行为只写在 `skills/` 里；Claude Code 的短别名命令只做转发，不重复逻辑。manifest 之间的
   名称、版本、描述保持一致。
10. **脚本兼容 macOS 自带 bash 3.2 / `/bin/sh`。** 不用数组、`[[`、`local`（`/bin/sh` 下）；中文紧跟变量时写 `${var}`；
    本机没有 `bats`、`shellcheck`，检查脚本零依赖；`jq` 不能作为技能运行时的硬依赖（Agent 自己能解析 JSON）。
11. **门禁不回退。** `[gate]` 是 `sh scripts/ci-local.sh`（它不覆盖 `plugins/`）；本任务的改动不得让它变红。
12. **仓库纪律。** 不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不推送、不碰 `.worktree/` 下的其它目录；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成一轮就结束会话。

## 6. 现状（2026-11-11 在基线 `1147ac0` 上核对）

- **插件市场。** `plugins/` 是名为 `autome` 的插件市场，目前只有 `loop` 一个插件。三份市场文件：
  - Claude Code：`plugins/.claude-plugin/marketplace.json`（`plugins[]` 项：`name`、`source: "./loop"`、`description`、`keywords`）；
  - Cursor：`plugins/.cursor-plugin/marketplace.json`（`name`、`source`、`description`）；
  - Codex：`plugins/.agents/plugins/marketplace.json`（`source: {source: "local", path: "./loop"}`、
    `policy: {installation: "AVAILABLE", authentication: "ON_INSTALL"}`、`category`）。
  新插件名为 `autome` 时，安装标识是 `autome@autome`，Claude Code 的命令命名空间是 `/autome:<命令>`。
- **`loop` 插件的结构（照着做，不要改它）。** `plugins/loop/`：
  `.claude-plugin/plugin.json`（多一个 `"commands": "./claude-commands/"`）、`.codex-plugin/plugin.json`
  （`"skills": "./skills/"` 加 `interface` 段）、`.cursor-plugin/plugin.json`（`"skills": "./skills/"`）；
  `skills/<name>/SKILL.md`（YAML 头 `name`、`description`，有副作用的技能用 `disable-model-invocation`）；
  `claude-commands/*.md` 是短别名，只转到对应技能；`evals/<场景>/case.yaml` + `graders/*.md` + `scaffold.sh`，
  `evals/README.md` 写触发策略与成本；`README.md` 写技能表与各宿主调用方式。
  `plugins/README.md` 写三宿主的安装命令与验证命令（`claude plugin validate plugins`、
  `claude plugin validate plugins/loop`、`claude --plugin-dir plugins/loop plugin details loop`）。
- **命令行事实（已安装的 `autome 0.1.0` 与源码 `crates/autome-cli/src/main.rs` 一致）。**
  - `autome task new [需求] [--kind feature|bug] [--brief <文件|->] [--draft] [-p 项目] [--attach 文件]... [--doc 路径]...`；
    只给 `--brief` 不给一句话需求时，request 取正文标题。`--json` 输出 `task.create` 的原始结果，其中 `task` 对象
    （`dispatch.rs::task_json`）含 `id`、`kind`、`slug`、`title`、`request`、`state`（如 `{"state":"draft"}`）、
    `branch`、`doc_dir` 等；不带 `--json` 时打印 `<id>  <slug>  <状态>`，待开始的再提示 `autome task start <id>`。
  - `autome task start <任务>`；`autome reply <任务> "…"`；`autome project ls --json` 输出
    `{"projects":[{"project":{"id","display_name","path","removed_at",…},…}]}`，`removed_at` 非空的是已移除项目。
  - `-p/--project` 接受 id、名称或路径；不写时取当前目录所在的已注册项目。出错时向标准错误打印 `autome：<原因>`
    并以非零退出（用法错与执行失败的退出码不同，见 `CliError::exit_code`）。
  - 状态目录是 `$AUTOME_HOME`（默认 `~/.autome`）。
  - 安装：仓库里 `sh scripts/install-cli.sh`（从源码构建，装到 `$PREFIX`，默认 `~/.local/bin`）；其它机器
    `curl -fsSL <install-cli.sh 的 raw 地址> | sh` 或 `install-cli.sh --from <tar 包>`（见该脚本头部注释）。
  - **没有** `autome web` 子命令（W10 未做）。
- **内核侧已就绪。** K1（任务类型、任务卡正文、待开始，T-13）与 K2（Bug Loop，T-15）已合并到 main：`kind=bug`
  的任务走 Bug Loop，正文写进 `<doc_dir>/<slug>-card.md`。
- **Web 面板。** `apps/web` 已有骨架、内核网关、事件总线（W1–W3）；服务默认监听 `127.0.0.1:7420`
  （`apps/web/server/config.ts`，可由端口参数覆盖）；`~/.autome/web/server.json`（实际端口、pid）按参考文档 §5.3
  是 W10 才写的；`apps/web/app/routes/` 目前只有 `home.tsx`，任务详情页 `tasks.$id` 是 W7。
- **本机宿主。** `claude`、`codex` 已安装；`cursor-agent` / `agent` 未安装（Cursor 侧只能做结构性校验，设计要说明
  怎么验证）。

## 7. 待定问题（设计必须给出答案）

1. 技能的数量、名字与触发描述（例如一个「整理并创建任务卡」的技能）；是否提供 Claude Code 短别名；有副作用的部分
   是否用 `disable-model-invocation`，以及这与「讨论完说『建成 autome 任务』就触发」怎么兼顾。
2. 类型判断的规则；判断不出时怎么问。
3. 「信息不全」的判定：每种类型哪些小节是必填、哪些可省；追问的方式（一次问完还是逐条问）。验收 5 的 eval 用例
   以此为判分依据。
4. 项目推断：当前目录在已注册项目路径之下（含子目录、工作区项目）时怎么认；推断失败时展示什么、怎么让用户选；
   最终是否显式传 `-p <id>`。
5. 面板链接：在 W10/W7 未完成的现状下，怎样判断「`autome web` 在运行」（例如读 `$AUTOME_HOME/web/server.json`、
   探测默认端口的 `/healthz` 等）、链接指向哪里；判断不出时只回显任务 id 和下一步命令，不给链接。设计要写明选择的
   理由，且不能依赖本任务范围外的改动。
6. 没装 `autome` 时给出的安装命令的确切文本。
7. 创建失败（内核拒绝、项目已移除、正文超长等）时怎么把命令行的原话转达给用户，以及是否允许修改后重试。
8. 确定性检查清单：每轮门禁之外，用什么脚本/命令验证 manifest、市场登记、技能文本里的关键约束（确认闸门、默认
   `--draft`、唯一创建路径），以及如何用隔离 `AUTOME_HOME` 对真实命令行验证 README 里写的 `--json` 输出格式。
9. eval 用例的场景划分（至少覆盖验收 5；是否另加「未确认不创建」「默认待开始」等），grader 的阳性对照。

## 8. 项目背景摘要（2026-11-11 整理时）

**仓库。** 单个仓库 `autome-v2`（工作目录即本任务的 worktree，分支由 Autome 建好）。Autome 2.0：macOS 桌面应用加
命令行，把一句话需求在专用 worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 →
复盘 → rebase → 合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）推进成合并提交；Bug 走简化的 Bug Loop。
内核独占节点转换：会话只写设计文档，下一个节点由 `automed` 读状态块决定。本仓库正在用自己的 Loop 开发自己
（`docs/plans/2026-11-10-web-dashboard-via-loop.md`，并行 2：本任务 C1 与 Web 任务 W5 同轮）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1）：`crates/autome-domain`（纯类型与转换）、`crates/automed`
  （SQLite 账本、Git、会话启动、调度、门禁、协议、JSON-RPC 派发）、`crates/autome-cli`（`autome` 命令）。本任务只读。
- `apps/desktop` — Electron 外壳（`node --test`）；`apps/web` — React Router 8 + 自定义 Node 服务器（vitest）。本任务只读。
- `protocol/` — Loop 协议源（编译进 `automed`）。本任务只读。
- `plugins/` — 插件市场 `autome`：现有 `loop` 插件（Autome 1.x 的 Loop 工作方式，Markdown 技能 + bash 脚本），
  本任务新增 `autome` 插件。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 三份 ADR；
  `docs/plans/` 按日期的决策记录。

**测试与验证命令。**

```sh
# 插件（本任务的主要验证）
claude plugin validate plugins                      # 市场文件
claude plugin validate plugins/autome               # 插件 manifest（可加 --strict）
claude --plugin-dir plugins/autome plugin details autome   # 组件清单
# eval：触发式、有成本，不进门禁
claude plugin eval ./plugins/autome --max-cost-usd 2.00 --runs 3 --threshold 0.7 --json

# 对真实命令行的隔离验证（绝不碰 ~/.autome）
export AUTOME_HOME="$(mktemp -d)"
autome project add <临时 git 仓库> --skip-onboarding
printf '# 标题\n…' | autome task new --brief - --kind feature --draft --json

# 全量本地 CI（= [gate]，不覆盖 plugins/，用来确认没有回退）
sh scripts/ci-local.sh
```

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、
  不推送；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，
  不要进任务文档目录。项目级规范见 `.autome/rules/`，与 `AGENTS.md` 同等效力。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：并行 2；`[gate] command = "sh scripts/ci-local.sh"`，超时 30 分钟；崩溃重试 2 次，退避 60 秒。
- `plugins/README.md` 与 `plugins/loop/README.md`、`plugins/loop/evals/README.md`：现有插件的结构、安装、验证与
  eval 纪律（触发式、成本上限、grader 带阳性对照），新插件沿用。
- 本仓库带一份项目技能 `.claude/skills/autome-v2-simplify`（简化/清理用），本任务不需要。
