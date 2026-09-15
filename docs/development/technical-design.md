# Autome 2.0 技术设计

版本：v3　日期：2026-09-15　状态：定稿

- 本文替代 `2026-09-13-autome-2.0-greenfield-development-plan.md` 中 §3–§8、§10–§18 的技术设计。冲突处以本文为准。
- 需求见 `2026-09-15-autome-2.0-requirements.md`；决策记录见 `2026-09-15-autome-2.0-project-task-ui-decisions.md`。
- 沿用 09-13 方案的边界：D1 在独立仓库 autome-v2 从零建设，不复用 1.x 代码；D2 Rust 内核 `automed` 是唯一状态权威；D3 Electron 只做界面；D9 本地、单用户、macOS。1.x 的 `.autome/` 布局、任务文件协议与角色划分作为约定沿用，代码重新实现。

## 1. 架构总览

```
┌──────────────── Electron ────────────────┐      ┌──────────── automed (Rust) ────────────┐
│ Renderer（九块屏幕，无业务状态）           │      │ 项目登记 · 任务状态机 · 调度器          │
│ Preload（白名单 IPC）                      │◀────▶│ Git 工作区 · 会话启动器 · 配置解析      │
│ Main（启动 sidecar，安全基线）              │ JSON-RPC│ 环境探测 · 技能盘点 · SQLite 台账        │
└───────────────────────────────────────────┘ stdio └────────┬──────────┬──────────┬─────────┘
                                                            │          │          │
                                                   git / worktree   osascript   文件系统
                                                            │      iTerm2 / Terminal   │
                                                            ▼          ▼          ▼
                                                   用户仓库      claude / codex    .autome · docs ·
                                                   .worktree/    会话进程          .worktree · ~/.autome
```

与 09-13 方案相比的三个结构性变化：

1. **会话在 iTerm2 可见终端里运行**，不是 automed 的子进程。automed 通过启动包装脚本写出的退出标记感知会话结束。
2. **任务状态从仓库文件推导**：设计文档头部状态块、里程碑三态、Backlog 与争议项是事实来源；SQLite 只存登记、索引、台账与用户表态。
3. **automed 独占节点转换**。Agent 不再自己启动下一个会话；每个会话结束后由 automed 依据状态块决定下一节点并启动。这使暂停、并行控制、角色开关、预算与停顿点全部可由内核执行。

## 2. 进程与生命周期

- Electron Main 启动 `automed` sidecar（单实例，framed JSON-RPC over stdio，沿用 autome-v2 现有实现）。sidecar 崩溃时 Main 重启它，Renderer 显示「内核重连中」。
- 会话进程由包装脚本 `.autome/skill/run_session.sh` 在终端中启动。脚本职责：写 pid 文件；执行 CLI；tee 输出到日志；结束后写退出标记（exit code、结束时间）。
- automed 用文件系统 watcher 监听 `.autome/output/sessions/`，并以 5 秒轮询兜底。退出标记出现即视为会话结束。
- 停止 = 读 pid 文件，向进程组发 SIGTERM，3 秒后 SIGKILL，随后写「已停止」标记。
- 应用重启后 automed 扫描所有项目的 `.autome/output/sessions/`：有 pid 且进程存活 → 会话仍在运行；有退出标记未消费 → 立即消费并推进；无标记且进程不存在 → 判定会话崩溃，任务进入失败状态供用户选择。

## 3. 文件布局

### 3.1 仓库内（随仓库提交，除标注忽略者）

```
.autome/
  config.toml              项目 Loop 配置（角色、并行、轮次、系数、技能绑定）
  rules/*.md               项目规则，供 Agent 读取
  skill/
    run_session.sh         会话包装脚本（automed 调用，也可手工调用）
    session-protocol.md    会话须遵守的协议：不改 .autome、结束即退出、状态块格式
  output/                  忽略。sessions/<task>/<session-id>.{log,pid,exit}
.worktree/<slug>/          忽略。每个任务一个 worktree
.gitignore                 init 时追加 .worktree/ 与 .autome/output/
AGENTS.md                  Onboarding 起草；含指向 .autome/rules/ 的固定段
docs/agent-project-profile.md
docs/<slug>/               任务文档（任务分支上产生，合并后进入默认分支）
  <slug>-task.md · <slug>.md · <slug>-review.md · <slug>-adjudication.md · <slug>-audit.md · retro.md
  attachments/
docs/.archive/<slug>/      归档或取消的任务文档
.claude/skills/ · .agents/skills/   项目自定义技能，Autome 只读
```

1.x 的 `SCHEDULE.json` 不再沿用，其 autorun 语义由 automed 调度器承担。

### 3.2 全局与应用数据

```
~/.autome/config.toml                              全局默认（结构同项目配置）
~/Library/Application Support/Autome/autome.sqlite  项目登记、任务索引、会话台账、用户表态、事件日志
~/Library/Application Support/Autome/logs/          automed 与 Electron 日志
```

### 3.3 配置文件

```toml
# .autome/config.toml（项目）· 只写覆盖项；~/.autome/config.toml（全局）· 写全量默认
[loop]
parallel = 3            # 项目内并行上限，1..5
design_rounds = 15      # 设计循环上限
budget_factor = 5       # 实现预算 N = budget_factor × 初始里程碑数

[roles.plan]
enabled = true
runtime = "claude"      # claude | codex
model = "claude-opus-5"
effort = "high"         # 留空用 CLI 默认
skills = ["island-shop-conventions"]

[roles.review]
runtime = "codex"
model = "gpt-5.4"
effort = "high"

[roles.adjudicate]
[roles.impl]
[roles.audit]
```

## 4. 领域模型

```text
Project
  id · path · display_name · default_branch · parallel_limit
  onboarding(step 0..5 | skipped) · added_at · removed_at?

Task
  id(T-n) · project_id · slug · title · request_text · attachments[] · doc_refs[]
  state(见 §5) · node · design_round · impl_round · budget_n · convergence_mode
  milestones[{id, title, state(open|pending|done), reopen_count}]
  backlog[{id, text, decision(none|include|ignore)}]
  disputes[{id, text, sides, decision(none|a|b|custom(text))}]
  branch · worktree_path · created_at · completed_at? · merge_commit? · archived_at?

Session
  id · task_id · role · runtime · model · effort · skills[] · prompt
  started_at · ended_at? · exit_code? · log_path · outcome(见 §5.3)

RoleConfig { enabled, runtime, model, effort, skills[] }        × 5
ResolvedConfig = global ⊕ project（逐字段覆盖）· 校验结果 · 生效时间

Skill { name · source_dir · scope(global|project) · visible_to{claude, codex} }
SkillBinding { role · skill_name }                              存 config.toml

Environment { git, claude, codex, iterm2 : { present, version, path, login(ok|expired|n/a) } }
```

Task 的 `milestones`、`backlog`、`disputes`、轮次与 `convergence_mode` 由 automed 在每次会话结束后解析设计文档状态块得到，不由 Agent 通过 IPC 上报。

## 5. 任务状态机

### 5.1 状态

```text
Queued · Intake · Design · Review · Adjudicate · AwaitDesignApproval
Implement · Audit · Rebase · AwaitMerge · Merging · Cleanup · Done
Paused(resume_node) · Stopped(node) · Failed(reason) · Cancelled
```

界面上的 13 个节点对应 Intake…Done；Queued、Paused、Stopped、Failed 以芯片叠加显示。

### 5.2 角色与任务文件入口

沿用 1.x 任务文件协议的入口语句，automed 组装为会话 prompt：

| 角色 | 入口 |
|---|---|
| plan | `Please execute docs/<slug>/<slug>-task.md Task 1.` |
| review | `… Task 1 additional task 1.` |
| adjudicate | `… Task 1 additional task 2.` |
| impl | `… Task 1 additional task 3.` |
| audit | `… Task 1 additional task 4.` |

任务文件模板相对 1.x 的唯一协议变化：删除「启动下一个会话」的条款，改为「完成本轮后结束会话，由 Autome 调度下一节点」。

### 5.3 转换表

| 当前 | 触发 | 条件（来自状态块） | 下一 |
|---|---|---|---|
| Queued | 槽位空出 | 按提交顺序 | Intake |
| Intake | 会话结束 | 任务文件存在、标题已生成 | Design |
| Intake | 会话结束 | 任务文件缺失 | Failed(protocol) |
| Design | 会话结束 | review 启用 | Review |
| Design | 会话结束 | review 关闭 | AwaitDesignApproval |
| Review | 会话结束 | — | Adjudicate |
| Adjudicate | 会话结束 | 裁决要求复审 且 d < design_rounds | Review（d 已加一） |
| Adjudicate | 会话结束 | 裁决判定定稿 | AwaitDesignApproval |
| Adjudicate | 会话结束 | status = 不可实现 | Failed(infeasible) |
| Adjudicate | 会话结束 | d ≥ design_rounds | Failed(design_budget) |
| AwaitDesignApproval | 用户批准 | 无未消费表态 | Implement（计算 N） |
| AwaitDesignApproval | 用户批准 | 有裁定或纳入 | Design（注入表态） |
| AwaitDesignApproval | 用户驳回 | 附意见 | Design（注入意见） |
| Implement | 会话结束 | audit 启用 | Audit |
| Implement | 会话结束 | audit 关闭 且 全部里程碑 ≠ open | Rebase |
| Implement | 会话结束 | audit 关闭 且 有 open 且 k < N | Implement |
| Audit | 会话结束 | 全部里程碑 done | Rebase |
| Audit | 会话结束 | 有未完成 且 k < N | Implement |
| Audit | 会话结束 | k ≥ N | Failed(impl_budget) |
| Rebase | 内核执行 | 无冲突 | AwaitMerge |
| Rebase | 内核执行 | 有冲突 | Implement（冲突修复 prompt） |
| AwaitMerge | 用户点合并 | 无纳入表态 且 前置条件满足 | Merging |
| AwaitMerge | 用户点合并 | 有纳入表态 | Implement（新增里程碑，N 加 5 × 新增数） |
| AwaitMerge | 用户点合并 | 前置条件不满足 | AwaitMerge（提示原因） |
| Merging | 内核执行 | 成功 | Cleanup |
| Cleanup | 内核执行 | — | Done |
| 任一运行态 | 用户暂停 | — | Paused（当前会话跑完后不再启动） |
| 任一运行态 | 用户停止 | — | Stopped（杀会话） |
| Failed | 用户追加轮次 | — | 回到失败前节点，上限加值 |
| Failed | 用户从节点重跑 | — | 目标节点，重置该节点之后的计数 |
| 任一非终态 | 用户取消 | — | Cancelled |

会话 `outcome` 由解析结果归类：`advanced`、`needs_rerun`、`design_final`、`milestone_done`、`reopen`、`protocol_error`、`crashed`。

### 5.4 状态块解析

automed 解析设计文档头部的固定字段：`status`、`design-round`、`implementation-round`、`current-milestone`、`current-milestone-reopens`、`convergence-mode`、`next-action`，以及里程碑小节中的三态标记、`## Backlog` 与 `## 争议项`。字段缺失或格式错误一次即判 `protocol_error`，任务进入 Failed(protocol)，用户可选从该节点重跑。解析器有表驱动单元测试与 1.x 真实任务文档回归样本。

### 5.5 计数与预算

- d 由裁决轮加一，k 由实现轮加一，均以状态块为准，automed 只校验单调性。
- N 在首次进入 Implement 时计算：`budget_factor × 初始里程碑数`，写入 Task。追加轮次与纳入 Backlog 修改 Task.budget_n，不改配置。

## 6. Git 工作区操作

所有 Git 命令由 automed 以固定二进制、净化环境执行，禁用 credential helper，不触碰远端。

| 操作 | 命令 | 备注 |
|---|---|---|
| 识别默认分支 | `symbolic-ref refs/remotes/origin/HEAD`，无远端时取 `init.defaultBranch`，再无则当前 HEAD | 结果存 Project |
| 初始化 | `git init -b <default>`（非 Git 目录）；追加 .gitignore；写 .autome | init commit 在 Onboarding 完成或跳过后创建 |
| 创建任务 | `git worktree add .worktree/<slug> -b autome/<slug> <default>`；写 `docs/<slug>/attachments/`；在任务分支提交「chore(autome): T-n inputs」 | 任务分支上的提交由 Autome 与 Agent 共同产生 |
| 会话提交 | Agent 在 worktree 内按协议自行提交；会话结束后 automed 兜底提交 worktree 内剩余的未提交改动 | 兜底提交只作用于该任务自己的 worktree，提交信息标注为「未提交的剩余改动」 |
| rebase | `git -C .worktree/<slug> rebase <default>` | 冲突时 `rebase --abort`，记录冲突文件，转 Implement |
| 合并前置 | 主工作树 `git status --porcelain` 为空；任务分支 `merge-base --is-ancestor <default> autome/<slug>` | 不满足即拒绝 |
| 合并 | 在项目根 `git merge --no-ff autome/<slug> -m "merge(autome): T-n <title>"` | 主工作树随之更新 |
| 清理 | `git worktree remove .worktree/<slug>`；`git branch -d autome/<slug>` | 失败时保留并提示 |
| 取消 | 复制 `docs/<slug>/` 到项目根 `docs/.archive/<slug>/`；`worktree remove --force`；`branch -D` | 复制结果留在主工作树，不提交 |
| 归档 / 恢复 | 项目根 `git mv docs/<slug> docs/.archive/<slug>` 及反向 | 不提交 |

## 7. 会话启动器

### 7.1 启动

1. 解析生效配置，校验 SAME-MODEL 与技能可见性；不通过则任务进入 Failed(config)，不启动。
2. 组装 prompt：入口语句 + 绑定技能条款「本会话必须使用技能 X、Y」+ 冲突修复或用户意见附加段（如有）。
3. 组装命令：`run_session.sh <session-id> <role> <runtime> <model> <effort> "<prompt>"`，工作目录为任务 worktree。
4. 通过 osascript 在 iTerm2 新标签执行；iTerm2 缺失时用 Terminal。标签标题 `autome · T-n · <角色> #<轮>`。
5. 写 Session 记录，状态进入对应节点。

### 7.2 CLI 参数映射

模型、Effort、自动执行模式的命令行参数按 CLI 版本可能变化，放在 adapter 表中，不硬编码在状态机里。安装或版本变化时用 `--help` 探测校正。自动执行模式沿用各 CLI 的非交互自动模式；会话工作目录限定在 worktree，不做 09-13 方案的沙箱资格体系。这是有意接受的风险，见 §17。

### 7.3 结束与解析

退出标记出现后：读取 exit code；exit 非零或日志空白判 `crashed`；否则解析状态块（§5.4）并按转换表推进。日志路径写入 Session，界面按需读取。

## 8. 调度器

- 每个项目一个信号量，容量为 `parallel_limit`。占用槽位的状态：Intake、Design、Review、Adjudicate、Implement、Audit、Rebase、Merging、Cleanup。AwaitDesignApproval、AwaitMerge、Paused、Stopped、Failed 不占槽位。
- 排队按 created_at 升序。槽位释放后立即启动队首。
- 单个任务同一时刻最多一个会话。
- 暂停在当前会话结束后生效；恢复时若槽位不足则排队。

## 9. 配置解析

- 生效配置 = 全局 `~/.autome/config.toml` 逐字段被项目 `.autome/config.toml` 覆盖。「恢复默认」即删除项目文件中的该字段。
- 保存前校验：runtime 合法；review ≠ plan、audit ≠ impl（比较 `runtime:model` 字面值）；每个绑定技能对该角色 runtime 可见；parallel 1..5。任一失败返回结构化原因，界面据此标红。
- automed 监听两份配置文件变化，新会话启动时读取最新生效值。运行中会话不受影响。
- 界面保存只写文件，不执行 git 提交。

## 10. Onboarding 与初始化

初始化（幂等）：创建 `.autome/` 骨架与 `config.toml`（仅 `[loop]` 空节，全部继承全局）；追加 .gitignore；AGENTS.md 不存在时创建含规则段的最小版本，存在时只追加标记段。

Onboarding 第 3 步以专用 prompt 启动 Claude Code 会话（工作目录为项目根，不建 worktree）：调研仓库、起草 `docs/agent-project-profile.md` 与 AGENTS.md；空目录时向用户提问。退出标记出现后界面进入第 4 步，在应用内展示两个文件供编辑（文本框），保存即写回。第 5 步从全局默认复制全量到 `config.toml` 供修改。完成或跳过后 automed 创建 init commit。

## 11. 环境探测与安装

| 项 | 探测 | 登录态 | 安装 |
|---|---|---|---|
| Git | `git --version`、路径 | 不适用 | `xcode-select --install` |
| Claude Code | `claude --version`、路径 | 调用其登录状态命令，按版本映射 | `npm i -g @anthropic-ai/claude-code` 或 Homebrew |
| Codex | `codex --version`、路径 | 同上 | `npm i -g @openai/codex` 或 Homebrew |
| iTerm2 | `/Applications/iTerm.app` 存在与版本 | 不适用 | `brew install --cask iterm2` |

安装命令通过 osascript 在可见终端执行；Homebrew 或 npm 缺失时先给出它们的安装命令。探测在应用启动、回到前台、安装或登录终端关闭后触发，结果缓存并推送事件。

## 12. 技能盘点与绑定

- 扫描四类目录（§3.1、需求 S-01），每个子目录含 `SKILL.md` 即为一个技能。`~/.claude/skills`、`.claude/skills` 对 Claude 可见；`~/.agents/skills`、`.agents/skills` 对 Codex 可见；同名出现在两侧则双可见。
- 绑定写入 `config.toml` 的 `roles.<role>.skills`。保存校验见 §9。
- 启动时把绑定技能写入 prompt（§7.1）。不做加载限制。

## 13. 持久化与恢复

SQLite 表：`projects`、`tasks`、`sessions`、`decisions`（用户对 Backlog 与争议项的表态及消费状态）、`events`（append-only，界面事件流与审计用）。

事实分工：任务文档与状态块是任务进度的事实来源；SQLite 保存登记、索引、台账与表态。两者不一致时以文件为准并记录一条 `integrity_warning` 事件。

启动恢复：加载 projects → 校验路径与 worktree 存在 → 对每个非终态 task 扫描会话目录（§2）→ 推进或标记失败 → 重建调度队列。

## 14. IPC 接口

沿用 autome-v2 的 framed JSON-RPC stdio 与版本协商。

命令：`project.add / list / get / remove / onboarding.step / onboarding.skip`、`task.create / list / get / approve / reject / merge / pause / resume / stop / cancel / decide / extend_budget / rerun_from / archive / restore`、`config.get / set / reset_field / validate`、`env.detect / install / login`、`skills.list / bind`、`session.log`。

事件：`project.updated`、`task.updated`、`session.started / ended`、`config.changed`、`env.changed`、`skills.changed`、`core.status`。

## 15. Electron 工作台

- 沿用 autome-v2 的安全基线：contextIsolation、Renderer 无 Node、特权 `autome://` scheme、拒绝外部导航与权限请求。
- Renderer 只渲染 automed 推送的投影，不持有业务状态。屏幕与交互以设计稿为准。
- 外部动作（打开目录、打开编辑器、切到 iTerm2）经 Main 白名单执行。
- 一屏约束在 CI 用 Playwright 对 1512 × 944 逐屏测量。

## 16. 测试与质量门

| 门 | 内容 |
|---|---|
| T1 单元 | 状态块解析器（含 1.x 真实样本）；转换表表驱动测试；配置合并与校验 |
| T2 Git | 临时仓库上的 worktree 创建、rebase 干净与冲突、合并前置、合并、清理、取消、归档 |
| T3 启动器 | 用假 CLI 脚本验证包装脚本、pid、退出标记、崩溃判定、停止 |
| T4 IPC 与壳 | 命令与事件契约测试；Electron 安全基线冒烟 |
| T5 真实 CLI | 手工清单：Claude Code 与 Codex 各跑一个最小任务到合并 |
| T6 界面 | Playwright 一屏测量；关键交互脚本 |
| T7 端到端 | 需求文档 §6 的 10 个场景，用假 CLI 自动化，其中 1 与 3 另用真实 CLI 手工验证 |

## 17. 风险与对策

- **退出感知依赖包装脚本**。用户手动关闭终端标签会丢失退出标记。对策：pid 探活 + 日志 mtime 心跳，超过 10 分钟无心跳判崩溃。
- **CLI 参数漂移**。对策：adapter 表 + `--help` 探测，失败时环境页提示版本不支持。
- **自动执行模式的权限风险**。Agent 在 worktree 内仍可执行任意命令。对策：协议明示边界、`.autome/` 只读约定、日志留痕；后续版本再评估沙箱。
- **Agent 不写或写坏状态块**。对策：一次判协议失败并停下，不猜测；模板中状态块格式有示例。
- **并行任务共享输出目录**。对策：`sessions/<task>/` 分目录，文件名含 session id。
- **主工作树脏导致长期无法合并**。对策：待合并卡明示原因，提供「打开目录」。

## 18. 里程碑

| 阶段 | 内容 | 通过门 |
|---|---|---|
| M0（2 周） | 项目添加与初始化、跳过 Onboarding、配置读写与校验、环境探测、Electron 壳与项目 / 设置 / 环境屏 | T1、T4 全绿；添加空目录得到 init commit |
| M1（3 周） | 单任务全流程：启动器、状态块解析、转换表、设计定稿停顿、rebase、手动合并、清理、任务面板 | 场景 1、8、10 用假 CLI 通过；场景 1 用真实 Claude Code 手工通过 |
| M2（2 周） | 并行与排队、冲突修复路径、暂停 / 停止 / 取消、失败选项、Backlog 与争议项消费 | 场景 2、3、6、7、9 通过 |
| M3（2 周） | Onboarding 完整五步、技能盘点与绑定、一键安装、登录态与横幅 | 场景 4、5 通过；T5 清单通过 |
| M4（1–2 周） | 归档与恢复、仪表盘、打包与签名、T6 一屏测量进 CI | 全部 10 个场景通过；干净机器安装可用 |

## 19. 对 autome-v2 现有代码的处置

| 保留 | 删除或替换 |
|---|---|
| `automed` 的 SQLite 事件存储、framed JSON-RPC 循环与版本协商 | `autome-domain/project.rs` 八阶段状态机与 hold → 替换为 §4 的 Project |
| Electron Main 的安全基线与 sidecar 生命周期 | `project_intent.rs` |
| Codex / Claude adapter 骨架 → 改造为 §7 启动器与参数映射表 | contract、graph、replan、completion.evaluate 及其 IPC |
| `install-recipes/` → 作为 §11 安装表的数据来源 | ExecutionQueue / HarnessLease → 替换为 §8 项目信号量 |
| 测试基础设施与 CI 配置 | `playbooks/`、`profiles/`、`skill-policies/`、capability broker |

处置完成后更新 autome-v2 的 README 与 `docs/development/plan.md` 镜像，指向本文与需求文档。
