# autome 插件：在对话里整理任务卡并用命令行创建（Claude Code / Codex / Cursor）

status: 实现中
design-round: 1/15
implementation-round: 2/40
current-milestone: M-02
current-milestone-reopens: 0
convergence-mode: normal
next-action: 审计 M-02；下一实现轮推进 M-03 技能与短别名

任务文件：`docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件-task.md`（下称「任务文件」；验收标准指它 §3 的六条，记作 A1–A6；硬性约束指它 §5，记作 C1–C12；待定问题指它 §7，记作 Q1–Q9）。

## 背景与现状

Autome 的任务卡有三个来源（网页、对话、其他 Agent 的命令行），落到内核里是同一个 `task.create`。本任务做「对话」这一来源，并把「其他 Agent」的用法写进 README。内核侧（K1 类型/正文/待开始、K2 Bug Loop）已在 main；面板侧 `autome web`（W10）与任务详情页（W7）都还没有。

设计轮在基线上核对过的事实（隔离 `AUTOME_HOME` 实测，原始输出在 `.autome/output/plan1/`，不入库）：

- **命令行在进程内直接打开账本。** `autome` 不连守护进程，`Client::open()` 读 `AUTOME_HOME`（默认 `~/.autome`）下的账本，但 **`AUTOMED_DB_PATH` 优先于 `AUTOME_HOME`**（`crates/automed/src/entry.rs:14-21`）。所以隔离验证必须同时设 `AUTOME_HOME` 并 `unset AUTOMED_DB_PATH`。
- **`task new --brief - --json` 的输出**是 `{"queue_position", "started", "task": {...}}`；`task` 含 `id`（如 `T-1`）、`kind`、`slug`、`title`、`request`、`state`（待开始为 `{"state":"draft"}`，直接开始为 `{"node":"locate","state":"active"}`）、`branch`（`autome/<slug>`）、`doc_dir`、`created_at` 等。带 `--draft` 时 `started: false`；不带时进入调度（`crates/automed/src/dispatch.rs:1497-1504`），项目有空闲槽位并实际启动后才是 `started: true`，槽位占满时 `state` 为 `queued`、`started: false`、`queue_position` 为 1 起的位置（已有测试 `create_without_the_new_params_is_unchanged` 断言了这一点；`queue_position` 只在排队态非空，`dispatch.rs:1548-1554`）。不带 `--json` 时打印 `T-3  普通  待开始` 和一行 `已存为待开始。用 \`autome task start T-3\` 开始。`
- **标题与 slug 都取自正文第一行 `# <标题>`**（`# 测试标题` → `title`、`request`、`slug` 都是「测试标题」）。
- **`project ls --json`** 是 `{"projects":[{"counts","project":{"id","display_name","path","removed_at","kind","members",…},"slots_in_use"}]}`；`path` 是物理路径（临时目录记为 `/private/var/...`）。
- **`-p` 只认 id、`display_name` 或与项目根完全相同的路径**；传子目录路径会报「没有叫 … 的项目」。不写 `-p` 时取「包含当前目录的最深一个未移除项目」（`client.rs::resolve_project`）。
- **错误**一律是标准错误一行 `autome：<原因>`，执行失败退出码 1、用法错 2。实测：空正文「任务卡正文不能为空」；65546 字节「任务卡正文超过 64KB（65546 字节，上限 65536 字节）」；当前目录不在项目里「当前目录不在任何已注册的项目里：用 -p 指定，或先 `autome project add <路径>`」；`task start` 非待开始任务「只有待开始的任务可以开始」。
- **插件宿主。** 在临时副本里放一个名为 `autome` 的插件：`claude plugin validate <市场>` 通过；`claude --plugin-dir <插件> plugin details autome` 列出 `Skills (2)  autome-task, task`（短别名命令也计入 Skills）；`--strict` 下缺 `author` 的警告会判失败（`loop` 带 `author`、`license`，`--strict` 通过）。Codex 0.155.1 可在临时 `CODEX_HOME` 下 `plugin marketplace add` + `plugin add loop@autome`，`plugin list` 显示 `installed, enabled  <版本>`，插件被复制进 `$CODEX_HOME/plugins/cache/autome/<名>/<版本>/`。本机没有 `cursor-agent` / `agent`。
- **`claude plugin eval`**：Bash、Write、Edit 等是「gated tools」，不加 `--allow-tools` 时被评估的 Agent 跑不了任何命令；`scaffold_script` 不加 `--scaffold` 不运行。
- **面板**：`apps/web/server/http.ts` 有 `GET /healthz` → `{"ok":true}`，默认端口 7420（`config.ts`）。`server.json` 的格式按参考文档 §5.3 只说了「实际端口、pid、启动时间」，字段名待 W10 定。
- **安装命令**：`scripts/install-cli.sh` 头注释与仓库根 `README.md:155` 的远程地址是 `https://raw.githubusercontent.com/LinkedDestiny/autome/main/scripts/install-cli.sh`。

## 目标与非目标

**目标。** 新建 `plugins/autome/`，三宿主共用一个技能 `autome-task`：用户在对话里说「建成 autome 任务」之类的话，技能把已讨论内容整理成符合模板的任务卡正文、判断类型、确定项目，**完整展示并等到明确确认**后，用 `autome task new --brief - --kind <类型> [--draft] -p <项目 id> --json` 创建，回显任务 id、下一步命令和（面板在运行时）面板链接。README 写明 Agent 程序化创建的方法。覆盖 A1–A6。

**非目标。** 不改 `plugins/` 以外的任何文件，不动 `plugins/loop/`（C1）；不做 `autome web`、`server.json` 写入、详情页；不加命令行参数或输出字段；不支持附件上传；创建后不自动 `task start`，也不做批准、合并等控制；不注册项目（项目不存在时只告诉用户 `autome project add`）。

## 设计

### 插件结构（Q1、C9）

```
plugins/autome/
├── .claude-plugin/plugin.json      name/version/description/author/license/keywords + "commands": "./claude-commands/"
├── .codex-plugin/plugin.json       同上 + "skills": "./skills/" + interface{displayName,shortDescription,developerName,category}
├── .cursor-plugin/plugin.json      同上 + "skills": "./skills/"
├── skills/autome-task/
│   ├── SKILL.md                    全部行为
│   ├── scripts/check-card.sh       正文模板与大小校验（确定性）
│   └── scripts/panel-url.sh        面板链接探测（确定性）
├── claude-commands/task.md         Claude Code 短别名 /autome:task，只转发到 SKILL.md
├── evals/                          README.md + 三个场景（见「eval」）
├── tests/run.sh                    零依赖确定性检查（分组运行）
├── tests/hosts.sh                  宿主加载检查（需要 claude、codex）
├── tests/verify-cli.sh             对真实命令行的隔离验证（需要 autome）
└── README.md
```

- 三份 manifest 的 `name` 为 `autome`、`version` 为 `0.1.0`、`description` 逐字相同，`author` 为 `{"name": "dannie"}`、`license` 为 `MIT`（让 `--strict` 通过，与 `loop` 一致）。JSON 采用两空格缩进、一键一行，便于 `tests/run.sh` 用 `sed` 比对（C10 不依赖 `jq`）。
- 三份市场文件在 `plugins[]` 末尾追加 `autome` 条目，格式照抄同文件里的 `loop` 条目（Claude Code：`source: "./autome"` + `description` + `keywords`；Cursor：`source` + `description`；Codex：`source: {source: "local", path: "./autome"}` + 同样的 `policy` 与 `category: "Productivity"`）。`loop` 条目一字不改。
- **一个技能**，名为 `autome-task`。程序化创建不需要技能（Agent 直接读 README 跑命令行），所以不另设技能。
- **短别名**：`claude-commands/task.md`，在 Claude Code 里是 `/autome:task`，正文只有「读取 `${CLAUDE_PLUGIN_ROOT}/skills/autome-task/SKILL.md` 按步骤执行，参数 $ARGUMENTS」，写法照 `plugins/loop/claude-commands/task.md`。
- **不用 `disable-model-invocation`。** 需求要求讨论完说一句话就能触发；设了它，Claude Code 只能靠斜杠命令调用，Codex/Cursor 的隐式触发也会失效。副作用的安全由技能内的确认闸门（第 4 步）承担：技能被误触发时，最多是整理出一张卡并停下来问，不会创建任何东西。
- **触发描述**（`description`）写明：把对话里讨论好的需求或 Bug 整理成 Autome 任务卡，确认后用 autome 命令行创建；用户说「建成 autome 任务」「建个 autome 任务卡」「提给 autome」「交给 autome 做」「把这个 Bug 提到 autome」时使用。
- 技能里找脚本的写法照 `loop-init`：脚本在本 SKILL.md 所在目录的 `scripts/` 下（Claude Code 中是 `${CLAUDE_SKILL_DIR}/scripts/…`；Codex、Cursor 给出 SKILL.md 的绝对路径，取其目录）。

### 技能流程（SKILL.md 的固定小节，按此顺序）

SKILL.md 用下列二级标题，`tests/run.sh skill` 按标题检查：`## 1. 整理与追问`、`## 2. 检查命令行`、`## 3. 确定项目`、`## 4. 展示并等待确认`、`## 5. 创建`、`## 6. 回显`、`## 失败时`、`## 禁止`。

**第 1 步 整理与追问（Q2、Q3、C5）。** 只用对话里已有的内容，不跑命令。

- *类型判断*：用户明说了就照用户的。否则：讨论的是「已有行为出错」——报错、崩溃、结果不对、以前好的现在坏了、和文档或预期不符——为 `bug`；讨论的是「新增或改变行为」——新功能、改交互、重构、加配置——为 `feature`。两种信号都有、或都没有，就问一句「这是 Bug（现有行为出错）还是需求（新增或改变行为）？」，**不默认**。类型定下之前不追问小节内容，因为两种模板的必填项不同。
- *小节规则*。二级标题逐字用模板里的名字，顺序与模板相同，**每个小节都写出来**（让正文与模板一一对应，也让 `check-card.sh` 能检查）：

  | 类型 | 必填（不得写「无」） | 可选（没有就写「无」） |
  |---|---|---|
  | 需求 `feature` | `## 背景`、`## 要做什么`、`## 验收标准`（至少一条列表项） | `## 不做什么`、`## 参考` |
  | Bug `bug` | `## 现象`、`## 复现步骤`（至少一条编号步骤）、`## 期望行为`、`## 实际行为`、`## 环境` | `## 线索` |

  第一行是 `# <标题>`，标题由技能根据讨论起，一句话、不含 `#`，建议 30 字以内（它同时成为 slug 与分支名）。「要做什么」把讨论中定下的细节逐条写；「实际行为」尽量贴报错原文、日志片段（放进代码块）。
- *什么算信息不全*：任一必填小节在讨论里找不到用户说过或明确同意过的内容。特别是：需求的「验收标准」、Bug 的「复现步骤」「期望行为」，**不得由技能推断补写**；Bug 的「环境」可以由技能读当前分支与提交（`git rev-parse --abbrev-ref HEAD`、`git rev-parse --short HEAD`）、`autome version` 作为**建议**提给用户，用户同意后才写入。用户明确说「复现不稳定」时，复现步骤写已知的触发条件并注明「不能稳定复现」（这是用户的原话，不算编造）。
- *追问方式*：**一次问完**。把所有缺的必填小节编号列出，每项一句具体问题（例如「验收标准：怎样算做完？给一两条能检查的条目」），不附带一份带占位符的草稿卡。用户回答后重新检查，仍有缺口就再问一次；用户明确说「不知道/先不写」的必填项，请他给一句最低限度的描述，不替他写。

**第 2 步 检查命令行（Q6、A4、C3）。** 运行 `command -v autome`。找不到时只输出下面这段并结束本次技能，不尝试任何其他创建方式，也不帮用户执行安装：

```text
没找到 autome 命令行（PATH 上没有 `autome`），装好后再说一次「建成 autome 任务」：

  # 在 autome 仓库的检出里
  sh scripts/install-cli.sh
  # 其它机器
  curl -fsSL https://raw.githubusercontent.com/LinkedDestiny/autome/main/scripts/install-cli.sh | sh

默认装到 ~/.local/bin，请确认它在 PATH 上；安装脚本最后会跑 `autome doctor` 检查环境。
```

（第 1 步不需要命令行，放在前面是为了让没装命令行的用户也先把卡整理清楚；整理好的内容留在对话里，装好后再触发即可。）

**第 3 步 确定项目（Q4、A2）。**

1. 用户在讨论里点名了项目（名称、id 或路径）就以它为准，在下一条的列表里按 `id`、`display_name` 或 `path` 匹配；匹配不上按第 3 条处理。
2. 否则运行 `autome project ls --json` 与 `pwd -P`，只看 `removed_at` 为 `null` 的项目；`path` 等于当前目录、或当前目录以 `path + "/"` 开头的，取 `path` 最长的那个——与命令行 `resolve_project` 同一规则，工作区里单独注册的成员项目优先于工作区本身。用 `pwd -P` 是因为账本里存的是物理路径。
3. 推断不出（当前目录不在任何项目里，或点名的项目不存在）时，把未移除的项目编号列出——每行 `显示名 · 路径 · id`——请用户选一个；列表为空时告诉用户先 `autome project add <路径>`，并结束本次技能。
4. **创建时一律显式传 `-p <项目 id>`**。展示给用户确认的项目和 `-p` 用的是同一个 id，不依赖创建命令执行时的当前目录（Codex、Cursor 不保证命令在同一目录执行）。

**第 4 步 展示并等待确认（A1、C2、C6）。**

1. 先把正文写入临时文件（见第 5 步「正文传输」），再跑 `sh <脚本> <类型> < <临时文件>`。不合格就按它的提示修正正文并重写该文件——缺内容回第 1 步追问，超长则告诉用户超了多少字节、请用户决定删减什么——**合格之前不展示确认卡**。
2. 合格后，一条消息里依次展示：

   ```text
   类型：需求（feature）            或  Bug（bug）
   项目：<显示名>（<id>，<路径>）
   创建后：存为待开始（--draft）     或  立即开始
   大小：<字节数> 字节（上限 65536）
   ```

   然后是**完整正文**，原样放在 Markdown 代码块里（正文自身含三个反引号时外层用四个），即将交给 `--brief -` 的那份，一字不差，不是摘要。最后问「确认创建吗？回复『确认』就创建；要改哪里直接说。」
3. **闸门：没有明确确认，不执行第 5 步。** 明确确认是：用户回复表达了现在就创建、且不带任何修改，如「确认」「可以，建吧」「好，就这样创建」「yes」。以下都**不算**：「差不多」「再看看」「先这样？」这类模糊回应；附带任何修改意见的回复（哪怕同时说了「确认」）；只针对其中一部分的回应。拿不准就再问一句「要我现在创建吗？」。
4. 用户要求任何修改（正文、类型、项目、是否待开始）：改完回到本步第 1 条，**再次完整展示、再次等待确认**。包括用户在确认时才说「直接开始」——它改变了展示过的一项，所以也要重新展示。

**是否待开始（A3、C4）。** 默认存为待开始（带 `--draft`）。只有用户明确说了以下之一才改为立即开始（不带 `--draft`）：「直接开始」「创建并开始」「建好就开始」「不要待开始」「马上开始跑」。这五种说法写在第 4 步；「创建后不调用 `autome task start`，只把命令告诉用户」写在第 6 步——第 1–5 步的文字里不出现 `autome task start`（T6）。

**第 5 步 创建（C3）。** SKILL.md 里的创建命令写成一个带标记的模板，`tests/verify-cli.sh` 会逐字取出来对真实命令行执行：

```text
<!-- create-command -->
autome task new --brief - --kind {{KIND}} --draft -p {{PROJECT_ID}} --json < '{{CARD_FILE}}'
<!-- /create-command -->
```

`{{KIND}}` 为 `feature` 或 `bug`，`{{PROJECT_ID}}` 为第 3 步的 id，`{{CARD_FILE}}` 为下面的临时文件路径；立即开始时删去 ` --draft`，其它一字不改。

**正文传输（D1-P01）。** 正文**不得**出现在任何 shell 命令行或 heredoc 里——shell 会先解析命令文本，正文里一行恰好等于定界符的内容会提前结束 heredoc，其后的行被当作命令执行，在确认之前就越过 C2 闸门。所以：

1. 第 4 步第 1 条之前，在系统临时目录（`${TMPDIR:-/tmp}`）下取一个不存在的文件名 `autome-card-<8 位随机字符>.md`，用宿主的文件写入能力（不是 shell）把正文原文写进去，**一字不差地等于**第 4 步展示的那份。
2. 校验与创建都用输入重定向读这同一个文件：`sh <脚本> {{KIND}} < '<临时文件>'` 与上面的创建命令。正文只被当作数据读取、从不经 shell 解析，两处字节数一致由「同一个文件」保证。路径用单引号包住。
3. 用户要求修改正文时重写该文件，再回到第 4 步第 1 条。创建成功或最终放弃后 `rm -f` 该文件；创建失败而要重试时，经第 4 步重新确认后复用或重写。
4. 宿主不能写文件时，退路是停下并把确认过的正文和命令交给用户自己执行，**不得**退回 heredoc 或把正文拼进命令行。

**第 6 步 回显（A3、Q5、D1-P02）。** 退出码 0 时从标准输出的 JSON 里取 `task.id`、`task.title`、`task.kind`、`task.state.state`、`started`、`queue_position`，按下面顺序判定状态（先匹配先用）并输出 `已创建 <id>「<title>」· 需求|Bug · <状态>`：

1. `queue_position` 非空 → **排队中**：加「排队第 N 位，空出槽位后由调度器自动开始」；**不附** `autome task start`（排队的任务不是待开始，该命令会报错）。
2. 否则 `task.state.state` 为 `draft` → **待开始**：加 `开始它：autome task start <id>`（只告诉用户，不执行）。
3. 否则 `started` 为 `true` → **已开始**：加「Loop 由 `autome run` 或打开的桌面端推进」。
4. 以上都不满足：原样转达 `task.state` 与 `started`，不猜测状态，也不附任何命令。

之后运行 `sh <本 skill 目录>/scripts/panel-url.sh <id>`，有输出就加一行 `面板：<URL>`，没有输出就不提面板。

**失败时（Q7）。** 退出码非 0：把标准错误原文（`autome：…` 那一行）放在引用块里转达，并附退出码。然后：

- 退出码 2（用法错）：说明是技能自身的命令写错了，停下，不自行改参数重试；
- 「没有叫 … 的项目」「当前目录不在任何已注册的项目里」或提到项目已移除：回第 3 步重新选项目，再走第 4 步；
- 「任务卡正文超过 64KB」「不能为空」「不是 UTF-8」：回第 4 步改正文；
- 其它：转达后停下，由用户决定。
- 退出码 0 但输出读不出 `task.id`：**不重试**，请用户用 `autome ls` 核对是否已创建，避免重复建卡。

任何重试都必须重新经过第 4 步的完整展示与确认。

**禁止（C3）。** 这一节列出不得使用的创建方式：`autome rpc`（包括 `task.create`）、直接读写 `~/.autome` 或 SQLite（`sqlite3`）、调用面板或任何 HTTP 接口创建、替用户安装命令行或注册项目。`tests/run.sh skill` 检查这些字样只出现在本节。

### 任务卡校验脚本 `check-card.sh`（C6、A1）

`sh check-card.sh <feature|bug>`，正文从标准输入读（技能用 `< 临时文件` 重定向喂入，见第 5 步「正文传输」）。正文里出现 `AUTOME_CARD_EOF` 之类的任何文字都只是普通内容，没有保留字。类型参数不合法退出 2。合格时退出 0，输出一行 `OK <字节数> 字节 · 标题：<标题>`；不合格退出 1，每个问题输出一行 `不合格：<原因>`。检查项（共五项）：

1. 第一行以 `# ` 开头且标题非空；
2. 二级标题（不在 ``` 代码块内的、以 `## ` 开头的行）恰好是该类型模板的小节，名字逐字相同、顺序相同、没有多余；
3. 必填小节去掉空白后非空，且不是「无」「待补充」「TBD」「TODO」；可选小节非空（没有内容写「无」）；
4. 需求的「验收标准」至少一个列表项（`- ` 或 `1. ` 开头）；Bug 的「复现步骤」至少一个编号项（`1. ` 开头）；
5. 字节数（`wc -c`）≤ 65536——与内核上限相同，由 `verify-cli.sh` 的 V5、V6 对真实命令行确认边界一致。

用 `/bin/sh` + `awk`/`sed`/`wc`，不用数组、`[[`、`local`，中文紧跟变量写 `${var}`（C10）。

### 面板链接 `panel-url.sh`（Q5、A3）

在 W10/W7 都没有的现状下，「`autome web` 在运行」的判据是**能连上面板并且它自己说健康**，而不是某个文件存在：

1. 候选端口：先读 `${AUTOME_HOME:-$HOME/.autome}/web/server.json` 里的 `"port": <数字>`（`sed` 取，取不到就跳过）；再加默认端口 `${AUTOME_PANEL_DEFAULT_PORT:-7420}`（环境变量只为测试用），去重。
2. 依次 `curl -fsS --max-time 2 http://127.0.0.1:<端口>/healthz`，响应体匹配 `"ok": *true` 的第一个端口即面板地址；都不行、或 `PATH` 上没有 `curl`，什么都不输出、退出 0。
3. 面板在：`curl -s -o /dev/null -w '%{http_code}' --max-time 2 <面板>/tasks/<id>` 为 `200` 时输出 `<面板>/tasks/<id>`（W7 合并后自然生效），否则输出 `<面板>/`（首页按项目展示任务卡）。

理由：只读不写，不依赖 W10 定下的 `server.json` 字段名（字段名不同时退回探测默认端口，最坏是不给链接，**不会给出打不开的链接**）；不依赖本任务范围外的改动。链接只用 `127.0.0.1`，与面板只绑本机一致。

### README 的程序化创建说明（A6）

`plugins/autome/README.md` 固定小节：`## 技能`、`## 安装`、`## 在对话里用`、`## Agent 程序化创建`、`## 验证`。「Agent 程序化创建」写：

- 命令：`autome task new --brief - --kind feature|bug [--draft] [-p <项目>] --json`（正文从标准输入，第一行 `# <标题>`，≤ 65536 字节），`autome task start <id>`，`autome project ls --json`；`-p` 接受 id、名称或项目根路径，不写取当前目录所在项目。
- 输出：一个 `task new --json` 示例与一个 `project ls --json` 示例，各放在 `<!-- task-new-json -->`、`<!-- project-ls-json -->` 标记之间，内容取自隔离账本的真实输出（只把 id、路径、时间换成示例值，不增删键）；字段说明（`task.id` 是后续命令用的 id，`task.state.state` 为 `draft` 即待开始，`started` 仅在实际启动后为 `true`，槽位占满时 `state` 为 `queued`、`started` 为 `false`、`queue_position` 给出排队位置，另有 `branch`、`doc_dir`）；不带 `--json` 的两行输出。
- 错误：标准错误一行 `autome：<原因>`，执行失败退出 1、用法错 2，并列出上文实测的四条典型原文。
- 状态目录 `$AUTOME_HOME`，以及「`AUTOMED_DB_PATH` 优先」这一点（程序化调用方做隔离时要知道）。

`plugins/README.md` 改为介绍两个插件：总表加 `autome` 一行；安装节为三宿主各补 `autome@autome` 的安装命令（Cursor 的复制命令同 `loop`）；验证节补 `claude plugin validate plugins/autome --strict`、`claude --plugin-dir plugins/autome plugin details autome`、`sh plugins/autome/tests/run.sh all`。`loop` 的现有内容保留。

### eval 用例（Q9、A5、C8）

三个场景，放 `plugins/autome/evals/<场景>/case.yaml` + `graders/*.md`，格式照 `plugins/loop/evals/`，**不用 `scaffold_script`**（场景只需要一段讨论，不需要夹具）：

| 场景 | prompt 的内容 | 判正 | grader 的阳性对照（应判 0） |
|---|---|---|---|
| `ask-before-inventing` | 一段需求讨论，定了背景与要做什么，没提怎样算做完；末尾「建成 autome 任务」 | 先追问验收标准；没有输出含自编验收条目的整卡；没有执行 `autome task new` | 回复直接给出整张卡，「验收标准」下列了讨论里没有的条目 |
| `bug-missing-repro` | 一段 Bug 讨论，只有现象与报错，没有复现步骤和期望行为；末尾「把这个 Bug 提到 autome」 | 一次追问复现步骤与期望行为；没有编造步骤 | 回复写出「1. 打开… 2. 点击…」这类讨论里没有的步骤 |
| `kind-ambiguous` | 讨论「把导出改成支持 CSV，现在导出的 Excel 打开乱码」，未说类型 | 问类型，不默认 | 回复直接按需求或 Bug 出卡 |

- **安全**：三个场景的正确行为都在第 1 步停下，不需要任何命令。运行时**不得**加 `--allow-tools Bash`——不加时被评估的 Agent 跑不了命令，就算技能写错也碰不到真实账本（C7）。
- 运行命令（写进 `evals/README.md`，不进门禁）：`claude plugin eval ./plugins/autome --ablation with-without --max-cost-usd 2.00 --runs 3 --threshold 0.7 --json`。
- grader 用 LLM 判分，每个 grader.md 带「## 阳性对照」小节写应判负样本（纪律同 `plugins/loop/evals/README.md`）。字段以本机 `claude plugin eval --help` 为准。

### 确定性检查（Q8、C8、C10）

三个脚本，分工按依赖划开。每个用例输出 `ok <编号> <名称>` 或 `not ok <编号> <名称>：<原因>`，最后一行 `<通过>/<总数> 通过`，有失败就非 0 退出。

- **`tests/run.sh <组…|all>`**：零依赖（`/bin/sh`、`grep`、`sed`、`awk`、`git`），六组共 59 个用例（原契约 51 项，加 M-02 边界 8 项），用例清单见各里程碑验收契约。`panel` 组用 `PATH` 前置的 `curl` 替身驱动 `panel-url.sh`，替身的行为按真实 `curl` 对齐：连接被拒时退出 7、`-w '%{http_code}'` 打印 `000`；`-f` 遇 4xx/5xx 退出 22。另有一个用例用真实 `curl` 连无人监听的端口作对照。替身用符号链接放进临时目录（macOS 改名副本会被杀）。
- **`tests/hosts.sh`**：需要 `claude` 与 `codex`。Claude Code 跑 `plugin validate` 与 `plugin details`；Codex 在 `mktemp -d` 的 `CODEX_HOME` 下 `marketplace add` + `plugin add autome@autome` + `plugin list`，不碰 `~/.codex`。Cursor 本机没有命令行，只做 `run.sh structure` 的结构检查，真机加载列入人工验收清单。
- **`tests/verify-cli.sh`**：需要 `autome` 与 `git`。开头 `AUTOME_HOME=$(mktemp -d)/home`、`unset AUTOMED_DB_PATH`，断言 `AUTOME_HOME` 不等于 `$HOME/.autome`；建临时 git 仓库、`project add --skip-onboarding`；结束删除临时目录。所有创建都发生在这个隔离账本里（C7）。不用 `autome` 替身——技能侧的确定性检查直接打真实命令行，没有替身与真实输出不一致的问题。

`[gate]`（`sh scripts/ci-local.sh`）不覆盖 `plugins/`，本任务不改它；每个里程碑的实现轮仍要确认它保持绿色（C11）。

## 风险与应对

- **Codex 沙箱写不了 `$AUTOME_HOME`。** `workspace-write` 下 `autome task new` 可能因写工作区外的账本失败。技能按「失败时」转达原文；用户可批准越权执行，或复制技能展示过的那条命令自己跑（同一条 `task new`，不是别的创建方式）。人工验收 H-02 专门覆盖。
- **自动触发误伤。** 不设 `disable-model-invocation`，技能可能在用户只是提到 autome 时被拉起。闸门保证不会创建；描述里写清触发说法，降低误触发。
- **`server.json` 字段名待 W10 定。** 见面板链接一节：读不到就退回默认端口探测，最坏不给链接。W10、W7 合并后核对一次（建议裁决轮放进 Backlog）。
- **宿主找不到脚本目录。** Cursor 若不给 SKILL.md 路径，`check-card.sh` 跑不了。SKILL.md 写退路：按 `check-card.sh` 的六条规则人工核对并用 `wc -c` 计字节，不得跳过大小检查；面板链接则省略。
- **eval 处于 early access，字段可能变。** eval 不进门禁，`run.sh evals` 只做结构检查；字段按 `--help` 校订。
- **已安装的 `autome` 落后于源码。** `verify-cli.sh` 用 `PATH` 上的 `autome`，并把 `autome version` 写进证据；与 README 所写不一致时以真实输出为准修 README。

## 验收标准对照

| 验收 | 落点 | 确定性验证 | 人工 / eval |
|---|---|---|---|
| A1 模板、类型、完整展示并等确认 | 第 1、4 步，`check-card.sh` | M-02 `run.sh card`；M-03 `run.sh skill` T1–T3 | H-01、H-02；eval `kind-ambiguous` |
| A2 按目录推断，否则列项目让选 | 第 3 步 | M-03 T8；`verify-cli.sh` V2（从项目外用 `-p <id>` 创建）、V4 | H-01 |
| A3 默认 `--draft`、回显 id 与面板链接 | 第 4–6 步，`panel-url.sh` | M-02 `run.sh panel`；M-03 T3、T4、V2、V3 | H-04 |
| A4 没装给安装命令 | 第 2 步 | M-03 T7 | H-03 |
| A5 eval：信息不全先追问 | `evals/` | M-05 `run.sh evals` | H-05（真实跑 eval） |
| A6 README 程序化创建与 `--json` 格式 | `README.md` | M-04 `run.sh readme`、`verify-cli.sh` V7–V9 | — |

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 插件骨架、三宿主 manifest 与市场登记 | 0 | |
| M-02 | 待审 | 任务卡校验与面板链接脚本 | 0 | |
| M-03 | 开放 | autome-task 技能与短别名 | 0 | |
| M-04 | 开放 | README 程序化创建说明与插件市场 README | 0 | |
| M-05 | 开放 | eval 用例 | 0 | |
| M-06 | 开放 | B-1：V7 示例键值一致性回归 | 0 | |
| M-07 | 开放 | B-2：人工验收逐项隔离前置 | 0 | |
| M-08 | 开放 | B-3：环境采集时序统一 | 0 | |

## 里程碑说明

- **M-01**（覆盖 C1、C9、需求原文「同时支持三宿主」；前置：无）。结果：三份 manifest、三份市场登记、`skills/autome-task/SKILL.md`（此时只有 frontmatter 与一行占位正文）、`claude-commands/task.md`、`README.md` 占位、`tests/run.sh`（structure 组）、`tests/hosts.sh`。先做它是因为最大的不确定性在「三个宿主是否都认这个插件」。
- **M-02**（覆盖 A1 模板部分、A3 面板链接、C6、C10；前置：M-01）。结果：`check-card.sh`、`panel-url.sh`，`run.sh` 的 card、panel 组。
- **M-03**（覆盖 A1–A4、C2–C5；前置：M-02）。结果：完整 SKILL.md 与短别名，`run.sh` 的 skill 组，`tests/verify-cli.sh`（V1–V6）。
- **M-04**（覆盖 A6；前置：M-03）。结果：`plugins/autome/README.md`、`plugins/README.md`，`run.sh` 的 readme 组，`verify-cli.sh` 的 V7–V9。
- **M-05**（覆盖 A5、C8；前置：M-03）。结果：`evals/` 三个场景与 `evals/README.md`，`run.sh` 的 evals 组。

## 验收契约

### M-01 验收契约

最新证据：docs/在-plugins-新增-autome-插件/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 验收命令：`sh plugins/autome/tests/run.sh structure && sh plugins/autome/tests/hosts.sh`
- 预期：structure 组 10 个用例通过（S1 三份 manifest 存在且 name 为 autome；S2 version 一致；S3 description 一致；S4 claude 的 commands 与 codex/cursor 的 skills 路径；S5 codex 的 interface 四个字段；S6 三份市场文件各有指向 `./autome` 的条目，Codex 带 policy；S7 三份市场文件的 loop 条目仍在；S8 SKILL.md 头有 `name: autome-task` 与非空 description、没有 `disable-model-invocation`；S9 task.md 引用 `${CLAUDE_PLUGIN_ROOT}/skills/autome-task/SKILL.md` 且不含 `autome task new`；S10 相对 `git merge-base HEAD main`，`plugins/loop/` 无改动）；hosts 5 个用例通过（H1 `claude plugin validate plugins` 退出 0；H2 `claude plugin validate plugins/autome --strict` 退出 0；H3 `plugin details autome` 含 `Skills (2)`、`autome-task`、`task`；H4 隔离 `CODEX_HOME` 下 `plugin add autome@autome` 退出 0 且 `plugin list` 有 `autome@autome  installed, enabled  0.1.0`；H5 缓存目录里有 `skills/autome-task/SKILL.md`）。共 15 个。
- 负向对照：删掉 `.cursor-plugin/marketplace.json` 里的 autome 条目时 S6 失败；把 `.codex-plugin/plugin.json` 的 version 改成 `0.1.1` 时 S2 与 H4 失败；去掉 `author` 时 H2 失败。

### M-02 验收契约

最新证据：docs/在-plugins-新增-autome-插件/evidence/M-02-r2-impl.md · 专项通过，待审 · 实现 #2

- 验收命令：`sh plugins/autome/tests/run.sh card panel`
- 预期：card 组 14 个用例通过（K1 合格需求退出 0 并输出字节数；K2 合格 Bug；K3 第一行不是 `# ` 退出 1；K4 缺「不做什么」；K5 小节顺序颠倒；K6 多余二级标题；K7 需求「验收标准」写「无」；K8 Bug「复现步骤」没有编号项；K9 恰好 65536 字节退出 0；K10 65537 字节退出 1 且原因含 65537；K11 正文含独占一行的 `AUTOME_CARD_EOF` 与其后一行 shell 命令文字，仍退出 0、字节数等于 `wc -c` 且命令文字未被执行（探针文件不存在）；K12 类型参数 `chore` 退出 2；K13 代码块里的 `## ` 不算标题；K14 可选小节「线索」写「无」退出 0）；panel 组 7 个用例通过（P1 无 server.json 且默认端口不通 → 空输出、退出 0；P2 默认端口健康、任务页 404 → `http://127.0.0.1:7420/`；P3 隔离 `AUTOME_HOME` 的 server.json 端口 7555 健康、任务页 200 → `http://127.0.0.1:7555/tasks/T-9`；P4 server.json 端口不通、默认端口健康 → 默认端口首页；P5 healthz 返回 200 但体里没有 `"ok":true` → 空；P6 `PATH` 上没有 curl → 空、退出 0；P7 真实 curl、`AUTOME_PANEL_DEFAULT_PORT=1` 且无 server.json → 空）。共 21 个契约用例；另补 K15 空输入、K16 空白标题、K17 四种必填占位、K18 空白可选小节、K19 编号验收项、P8 超时、P9 HTTP 错误、P10 无法解析端口，M-02 合计 29 个。
- 负向对照：字节上限写成 `-lt 65536` 时 K9 失败；只看 healthz 状态码不看响应体时 P5 失败；不跳过代码块内的 `## ` 时 K13 失败。

### M-03 验收契约

- 验收命令：`sh plugins/autome/tests/run.sh skill && sh plugins/autome/tests/verify-cli.sh skill`
- 预期：skill 组 10 个用例通过（T1 八个固定小节按序存在；T2 第 4 步含闸门句「没有明确确认，不执行第 5 步」并列出「差不多」「再看看」为不算确认；T3 `create-command` 标记内恰有一条 `autome task new`，含 `--brief -`、`--kind {{KIND}}`、`--draft`、`-p {{PROJECT_ID}}`、`--json`、`< '{{CARD_FILE}}'`，且整个 SKILL.md 不含 `<<`；T4 第 4 步列出「直接开始」等五种说法，第 6 步写明创建后不调用 `autome task start`，并按排队、待开始、已开始、其它四种情形分条回显，排队条不含 `autome task start`；T5 `autome rpc`、`task.create`、`sqlite3` 只出现在「禁止」节；T6 `autome task start` 不出现在第 1–5 步；T7 第 2 步含两条安装命令原文；T8 第 3 步含 `autome project ls --json`、`pwd -P`、`removed_at`、`-p`；T9 引用 `check-card.sh`、`panel-url.sh` 且文件存在；T10 description 含「建成 autome 任务」）；verify-cli skill 组 6 个用例通过（V1 隔离账本的 `project ls --json` 含临时仓库且 `removed_at` 为 null；V2 从 `/tmp` 执行代入后的模板（feature、合格正文，正文写入临时文件并含一行 `AUTOME_CARD_EOF` 加 `touch` 探针命令，创建后探针文件不存在、`task.request` 含该行原文）→ 退出 0，输出含 `"id": "T-1"`、`"kind": "feature"`、`"state": "draft"`、`"started": false` 与正文标题；V3 按技能规则删去 ` --draft` 后用 Bug 正文在空闲项目执行 → `"started": true`（槽位占满时的排队回显由 T4 与已有 Rust 测试覆盖，不推广这条结果）；V4 `-p prj-不存在` → 退出 1、标准错误以 `autome：没有叫` 开头；V5 65537 字节正文被 `check-card.sh` 拒绝且命令行也退出 1、报「超过 64KB」；V6 65536 字节正文两者都接受）。共 16 个。
- 负向对照：模板漏写 `--draft` 时 T3 与 V2 失败；模板改成依赖当前目录（去掉 `-p {{PROJECT_ID}}`）时 V2 失败（从 `/tmp` 执行找不到项目）；模板改回 heredoc 时 T3 与 V2（探针文件被创建）失败；第 6 步删去排队条时 T4 失败；SKILL.md 的闸门句被删时 T2 失败。

### M-04 验收契约

- 验收命令：`sh plugins/autome/tests/run.sh readme && sh plugins/autome/tests/verify-cli.sh readme`
- 预期：readme 组 5 个用例通过（R1 插件 README 五个固定小节；R2「Agent 程序化创建」含 `task new --brief - --kind`、`--json`、`autome task start <id>`、`autome project ls --json`；R3 `task-new-json` 标记块含 `id`、`kind`、`slug`、`title`、`state`、`branch`、`doc_dir`、`started`、`queue_position`；R4 写明 `autome：` 前缀与退出码 1、2；R5 `plugins/README.md` 含 `autome@autome` 的 Claude Code 与 Codex 安装命令、`plugins/autome` 的 Cursor 复制命令与 `validate plugins/autome`，且 `loop@autome` 原有行仍在）；verify-cli readme 组 3 个用例通过（V7 `task-new-json` 块里的每个键都出现在真实 `task new --json` 输出里；V8 `project-ls-json` 块同理对 `project ls --json`；V9 README 写的不带 `--json` 的两行格式与真实输出相符）。共 8 个。
- 负向对照：README 示例把 `id` 写成 `task_id` 时 V7 失败；把待开始状态写成 `"state": "pending"` 时 V7 失败；漏写 Codex 安装行时 R5 失败。

### M-05 验收契约

- 验收命令：`sh plugins/autome/tests/run.sh evals`
- 预期：evals 组 5 个用例通过（E1 三个场景目录各有含 `name`、`description`、`prompt`、`graders`、`runs`、`max_turns`、`timeout_seconds` 的 case.yaml 与至少一个 grader；E2 每个 grader 有「## 阳性对照」小节；E3 `ask-before-inventing` 的 grader 以「验收标准」判分；E4 没有 case.yaml 含 `scaffold_script`；E5 `evals/README.md` 含带 `--max-cost-usd` 的运行命令、「不进门禁」与「不要加 `--allow-tools Bash`」）。共 5 个；`run.sh all` 合计 59 个。
- 负向对照：删掉任一 grader 的阳性对照时 E2 失败；`evals/README.md` 的运行命令漏掉 `--max-cost-usd` 时 E5 失败。

### M-06 验收契约

B-1 / D1-N01 已纳入；前置 M-04。V7 同时比较示例键与值，仅规范化设计允许替换的 id、路径、时间；增加将 `"state": "draft"` 改成 `"state": "pending"` 的负向回归，必须被拒绝。验收：`sh plugins/autome/tests/verify-cli.sh readme`，真实示例通过、pending 变体失败。M-04 实现 V7 时遵守此值契约，本里程碑交付独立负向回归。

### M-07 验收契约

B-2 / D1-N02 已纳入；前置 M-05。H-01、H-02、H-04 每条人工记录各自写明隔离 `AUTOME_HOME`、`unset AUTOMED_DB_PATH`，不只依赖共同说明。验收：逐条提取三条记录，各自匹配两个前置；不要求本轮执行人工宿主验收。

### M-08 验收契约

B-3 / D1-N03 已纳入；前置 M-03。统一 SKILL.md 的环境采集规则：第 1 步不跑命令，Bug 环境信息缺失时可留待第 2 步确认命令行存在后采集 Git 与 `autome version`，经用户同意后才写入；其余信息缺失仍先追问。验收：`sh plugins/autome/tests/run.sh skill`，新增检查确保环境采集命令仅位于第 2 步、且说明用户确认，补充该路径的人工对话检查说明。

## 人工验收清单


- H-01 Claude Code：`AUTOME_HOME=$(mktemp -d)` 下注册一个临时仓库，在它的子目录里 `AUTOME_HOME=… claude --plugin-dir plugins/autome`，讨论一个需求后说「建成 autome 任务」——技能触发，展示类型、项目、「存为待开始」、字节数与完整正文后停下等待；回「差不多」不创建；回「确认」后创建并回显 `T-1` 与 `autome task start T-1`；人工验证入口共同前置：隔离 `AUTOME_HOME` 并 `unset AUTOMED_DB_PATH`（H-01、H-02、H-04 同）
- H-02 Codex：同 H-01 的隔离账本，`codex` 装上 `autome@autome` 后走一遍，记下沙箱是否拦截写 `$AUTOME_HOME` 以及技能怎样转达
- H-03 在 `PATH` 里没有 `autome` 的 shell 里触发技能，只得到两条安装命令，没有任何其他创建尝试
- H-04 `apps/web` 面板运行时创建一张卡，回显里的面板链接能在浏览器里打开
- H-05 本机跑一次 `claude plugin eval ./plugins/autome --ablation with-without --max-cost-usd 2.00 --runs 3 --threshold 0.7 --json`，三个场景都 ≥ 0.7
- H-06 有 Cursor 时按 `plugins/README.md` 安装，确认技能出现在技能列表里并能触发

## Backlog


## 争议项
