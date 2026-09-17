---
name: loop-task
description: 将一句话需求转化为自包含循环任务文件 docs/<slug>/<slug>-task.md（设计 → 评审/裁决 → 实现/审计 全自动收敛，协议融合自 cycle-runner）。自动读取或建立项目画像 docs/agent-project-profile.md。当用户提出新功能、重构、修复等需求并希望走全自动收敛循环，或提到生成任务文件、loop 任务、循环任务、收敛任务时使用。
---

# Loop 任务文件生成

把一个需求整理为自包含的 `docs/<slug>/<slug>-task.md`。任务文件包含生成时的完整项目画像、融合协议，以及 Task 1 / additional task 1–4；执行它的会话不会加载本 skill，**所有协议都必须在文件内**。

任务文件把循环写成可直接执行的任务序列，而不是需要推理的抽象协议：

- **Task 1**：设计初稿，完成后启动 additional task 1。
- **additional task 1**：设计评审（六类复审条件），完成后启动 additional task 2。
- **additional task 2**：设计裁决（维护 append-only 裁决记录）；按分支启动 additional task 1、additional task 3，或终止。
- **additional task 3**：实现（推进最小编号里程碑至`待审`），完成后一律启动 additional task 4。
- **additional task 4**：实现审计（独立复验、实现缺陷/验证缺口二分、reopen 收敛）；按分支启动 additional task 3，或终止。

设计循环最多 15 轮；实现循环总预算为初始里程碑数乘以 5。所有路径一律相对当前项目的仓库根。

## 路径

| 内容 | 路径 |
|---|---|
| 项目画像 | `docs/agent-project-profile.md` |
| 任务模板 | 本 skill 的 [template.md](template.md) |
| 画像规范 | 本 skill 的 [references/project-profile-schema.md](references/project-profile-schema.md) |
| 会话后端说明 | 本 skill 的 [references/session-backends.md](references/session-backends.md) |
| 任务文件 | `docs/<slug>/<slug>-task.md` |
| 设计文档 | `docs/<slug>/<slug>.md` |
| 设计评审 | `docs/<slug>/<slug>-review.md` |
| 裁决记录 | `docs/<slug>/<slug>-adjudication.md` |
| 实现审计 | `docs/<slug>/<slug>-audit.md` |
| 运行记录 | `docs/<slug>/retro.md` |

## 生成流程

### 0. 前置检查：项目已初始化

检查项目根下 `.autome/skill/new-session.md` 与 `.autome/SCHEDULE.json` 是否存在。缺失说明项目未初始化——先执行 `/loop:init`（或运行插件的 `scripts/init.sh`）再继续，不要手工散装补文件。

### 1. 确定参数

- `slug`：kebab-case，如 `session-launcher-hardening`。
- 任务标题：简短、准确，不包含实现细节。
- 会话后端：用户未指定时使用 `.autome/skill/new-session.md`（按角色路由）。
- 是否立即启动：默认只生成任务文件；只有用户明确要求时才启动首个会话。

若同名任务目录 `docs/<slug>/` 已存在：

- 用户要求保留旧运行时，先将完整任务目录移入 `docs/.archive/` 归档，再使用原 slug。
- 不需要归档时，使用 `-v2` 等新 slug。
- 不得覆盖仍在执行的任务。

### 2. 读取或建立项目画像

固定路径 `docs/agent-project-profile.md`。

文件存在时：读取全文，对照 [references/project-profile-schema.md](references/project-profile-schema.md) 检查内容；若画像仍包含已废弃的流程专用内容（里程碑状态机、评审流程、轮次预算等协议内容），在用户明确要求重新生成任务时将画像整理为项目事实、工程原则、调研入口和测试命令。

文件不存在时：

1. 阅读画像规范。
2. 调研仓库的权威规则、源码布局、测试命令和运行环境。
3. 起草 `docs/agent-project-profile.md`。
4. 默认停止，等待用户确认；用户已明确授权连续执行时可以继续。

生成任务文件时，必须把项目画像全文逐字复制到 `{{PROJECT_PROFILE}}`。不得只写画像路径或摘要——任务目录归档后仍能完整保留生成时的项目背景。

### 3. 调研并整理任务

使用项目画像中的调研入口：

1. 阅读权威规则和与需求直接相关的源码、测试及设计文档。
2. 核实用户描述中的关键现状；不确定的事实写成待验证事项。
3. 将需求整理为背景和编号要求。要求只描述目标、边界和验收约束，不预先指定未经验证的实现方案；含糊处细化为可评审的表述或开放点，交给设计循环裁决。
4. 若需求依赖外部事实（第三方服务参数、协议细节等），在注意事项中加入「写方案前联网搜索核对」的条目。
5. 只有相互独立、可以分别验收的需求才拆成多个任务。
6. 任务若依赖前置成果，列出权威文件和接口，但不得复制旧运行的设计结论作为新设计。

### 4. 准备会话后端

阅读 [references/session-backends.md](references/session-backends.md)。后端文件由 `/loop:init` 统一安装到 `.autome/skill/`；确认所选后端的说明文件存在，缺失时先补跑 init。

| `{{SESSION_SKILL}}` | 运行环境 |
|---|---|
| `.autome/skill/new-session.md`（默认） | 按启动语句推导角色（plan/review/adjudicate/impl/audit），经 `.autome/skill/loop-roles.conf` 路由 CLI 与模型 |
| `.autome/skill/new-claude-session.md` | 全程强制 Claude Code CLI |
| `.autome/skill/new-codex-session.md` | 全程强制 Codex CLI |

想让不同角色用不同客户端与模型：人工编辑 `.autome/skill/loop-roles.conf`（行格式 `role=runtime[:model]`）或用 `autome roles set`；该文件是人工维护的配置，agent 不得修改。

### 5. 生成任务文件

读取本 skill 的 [template.md](template.md)，逐字复制全文到 `docs/<slug>/<slug>-task.md`，只替换占位符：

| 占位符 | 内容 |
|---|---|
| `{{PROJECT_PROFILE}}` | 项目画像全文，逐字复制 |
| `{{SLUG}}` | 任务 slug |
| `{{DOC_DIR}}` | `docs/<slug>` |
| `{{TASK_TITLE}}` | 任务标题 |
| `{{TASK_BODY}}` | 背景和编号要求 |
| `{{TASK_NOTES}}` | 编号的任务特定调查、验证和范围说明；最后一条固定为「不要执行下面任何 additional task。」 |
| `{{SESSION_SKILL}}` | 会话后端说明文件的仓库相对路径 |
| `{{GENERATION_STAMP}}` | `generated-by: loop-task \| template-sha256: <前12位> \| generated-at: <YYYY-MM-DD>` |

生成时只创建任务目录和任务文件。设计、评审、裁决、审计、运行记录由后续会话创建。

**除占位符外，模板其余文本必须逐字保留。** 单次任务不得修改以下规则（协议由多任务复盘证据驱动演进，属插件内 template.md 的人工维护范围）：

- 六类设计复审条件。
- 设计预算 15；实现预算 N = 5 × 初始里程碑数，各里程碑共享。
- 里程碑三态机（开放/待审/已完成）；实现轮不得标记`已完成`；只有实现轮增加实现轮次计数。
- 实现缺陷与验证缺口的二分处理：验证缺口由审计轮加强验证并立即复验，产品实现仍通过时不退回里程碑。
- 审计退回实现时必须给出下一轮的具体工作（next-action）。
- reopen 计数规则与双触发收敛（同一领域第二次 → domain-review；同一里程碑第三次 → milestone-review）；四段式 `Convergence Note` 只维护在 audit 文件顶部。
- 裁决记录 append-only、复提计数与争议项冻结（复提 2 次冻结，不阻塞终止）。
- Backlog 与争议项小节只进不出。
- 停滞保险丝（连续 2 个实现轮无新增`待审`且无收敛批次完成 → 协议失败）。
- 问题可追溯（评审/审计的每个问题必须注明违反的 Task 要求编号、设计条款或验收命令）与「审计不追求完美」的 Backlog 兜底（共同原则 8/9，防止在细小问题上过度优化、偏离任务目标）。
- 设计通过后自动进入实现循环；目标不可实现和预算耗尽的终止语义。

### 6. 自检

```bash
rg -n "\{\{" docs/<slug>/<slug>-task.md    # 应无输出：无残留占位符
test -f docs/agent-project-profile.md
test -f <SESSION_SKILL>
```

并人工确认：

- 项目画像已完整嵌入。
- 任务要求编号连续、边界明确；注意事项最后一条为固定文本。
- 接力指向正确：Task 1 → at1；at1 → at2；at2 三分支（回 at1 / 进 at3 / 终止）；at3 → at4；at4 三分支（回 at3 / 完成或停滞终止）。
- 五处自启动 prompt 的文件名都是 `docs/<slug>/<slug>-task.md`。
- `{{SESSION_SKILL}}` 替换后的文件真实存在。
- 没有旧任务名称、旧归档路径或其它项目的残留内容。

### 7. 交付

报告：任务文件路径、使用的画像路径、会话后端、模板哈希、是否已启动首个会话。

用户未要求启动时，不得自行启动。首个启动提示词为：

```text
Please execute docs/<slug>/<slug>-task.md Task 1.
```

CLI 等价操作：`autome go <slug>`（默认全自动；`--gated` 在 plan→评审、设计→实现两处停靠等人工放行）。

## 维护原则

- 每个任务文件固定使用生成时的模板快照。运行期间不得同步新版模板。
- 旧任务保持原样；模板变更只影响新生成的任务。
- 模板改进以多个运行的 `retro.md` 复盘证据为依据，不因单个任务的局部问题增加全局规则。
- 任务之间如何接力必须直白写在 Task / additional task 内；不要改回需要模型推理的抽象循环协议。
- 项目特定的状态表、清单、参考函数或验证程序应留在任务设计中，不应加入全局模板。
