# loop — 本地自动化 Loop 工程插件

把一句话需求变成全自动收敛的「设计 → 评审/裁决 → 实现/审计」自驱动循环。协议为 [cycle-runner](https://github.com/sskyy/cycle-runner) 融合版（cycle-runner 骨架 + autome 嫁接机制），源流可溯至 Mesh 的 `mesh-task-prompt`。

同一份插件可装进 **Claude Code**、**Codex**（ChatGPT 账号登录的 Codex CLI / 桌面端）和 **Cursor**（IDE 与 `agent` CLI）。安装方法见上一级的 [`plugins/README.md`](../README.md)。

这是 Autome 1.x 的工作方式：会话靠 Agent 自己调启动脚本接力，不需要 Autome 桌面端。Autome 2.0 桌面端用的是仓库根的 `protocol/`，两者互不依赖。

## Skill

五个 skill 是三个宿主共用的唯一实现：

| Skill | 作用 |
|---|---|
| `loop-init [--force]` | 初始化当前项目：装入会话启动脚本、角色路由表、kill switch、测试方法论，建目录骨架 |
| `loop-task <一句话需求>` | 生成自包含任务文件 `docs/<slug>/<slug>-task.md`（自动读取/建立项目画像） |
| `loop-run <slug> <task1\|review\|adjudicate\|impl\|audit> [claude\|codex\|cursor] [--gated]` | 从指定入口拉起循环会话；此后协议自驱动接力 |
| `loop-run <slug> approve` / `reject` | 放行 / 驳回停靠在人工卡点的会话 |
| `loop-status [slug]` | 查看 kill switch、卡点、设计文档状态块（status/轮次/里程碑/收敛模式）、会话台账 |
| `loop-stop [resume]` | 拉下 / 恢复 kill switch |

各宿主里的调用方式：

| 宿主 | 调用 |
|---|---|
| Claude Code | `/loop:init`、`/loop:task`、`/loop:run`、`/loop:status`、`/loop:stop`（短别名），或 `/loop:loop-init` 等 |
| Codex | `$loop:loop-init` 等，或直接用自然语言描述 |
| Cursor | `/loop-init` 等，或直接用自然语言描述 |

`loop-init`、`loop-run`、`loop-stop` 有副作用，只在用户明确调用时执行（`disable-model-invocation`）。

## 会话运行时

每个角色用哪个 CLI 与模型，写在项目的 `.autome/skill/loop-roles.conf`（`role=runtime[:model]`）：

| runtime | 启动方式 |
|---|---|
| `claude` | `claude -p <prompt> [--model M] --dangerously-skip-permissions` |
| `codex` | `codex exec [-m M] --full-auto --skip-git-repo-check <prompt>` |
| `cursor` | `cursor-agent`（或 `agent`）`-p [--model M] --force --trust <prompt>` |

权限参数可用 `LOOP_CLAUDE_FLAGS` / `LOOP_CODEX_FLAGS` / `LOOP_CURSOR_FLAGS` 覆盖；`LOOP_CURSOR_BIN` 指定 Cursor CLI 的可执行文件。生成与评测角色必须用不同的 `runtime:model`（`plan≠review`、`impl≠audit`），违反时启动脚本输出 `SAME-MODEL` 并拒绝。

## 组成

```
.claude-plugin/plugin.json   Claude Code manifest（另指向 claude-commands/）
.codex-plugin/plugin.json    Codex manifest
.cursor-plugin/plugin.json   Cursor manifest
skills/
  loop-init/                 初始化；scripts/init.sh 与 scripts/assets/（装入项目的运行时文件）
  loop-task/                 任务生成；template.md（融合协议全文，唯一维护处）与 references/
  loop-run/  loop-status/  loop-stop/
claude-commands/             Claude Code 的 /loop:init 等短别名，只转到对应 skill
evals/                       Claude Code 的 plugin eval 用例（触发式，有成本）
```

`loop-init` 装入项目的运行时文件集中在 `.autome/`：

```
.autome/skill/               会话启动脚本（角色路由 / claude / codex / cursor）、说明文档、loop-roles.conf
.autome/skill/testing/       测试设计方法论
.autome/SCHEDULE.json        kill switch
AGENTS.md                    追加 Loop 章节（<!-- loop-plugin:begin/end --> 之间）
```

初始化同时在项目根创建兼容符号链接（`prompt/skill`、`prompt/output`、`SCHEDULE.json` → `.autome/` 内对应路径），0.6.x 旧布局项目重跑 init 自动迁移（mv + 符号链接，不丢数据）。

## 设计要点

- **协议自包含**：执行任务的 agent 不加载插件，项目画像与协议全文随任务文件进入每个会话。运行时脚本装在项目内（`.autome/skill/`），任务执行不依赖插件存在，也不依赖装在哪个宿主里。
- **会话接力**：每轮结束时 agent 用启动脚本拉起下一个角色的新会话（启动即结束）；默认经 `new_session.sh` 按 `loop-roles.conf` 把每个角色路由到各自配置的 CLI 与模型。
- **收敛判据可机判**：设计循环只认六类复审条件，无采纳问题即通过并自动进入实现；实现循环靠里程碑三态 + reopen 收敛模式 + 停滞保险丝，预算 = 5 × 里程碑数。
- **人工卡点可选**：协议本体零人工介入；`loop-run <slug> task1 --gated` 时启动脚本在 plan→评审、设计→实现两个边界停靠（`PARKED`），`loop-run <slug> approve` 放行。
- **kill switch**：`.autome/SCHEDULE.json` `autorun=false` 时启动脚本拒绝接力，是唯一的手动刹车。

## 版本

0.8.0：从 `legacy/loop-plugin` 搬到 `plugins/`；五个命令改为三宿主共用的 skill（Claude Code 保留 `/loop:*` 短别名）；新增 Codex 与 Cursor 的 manifest；会话运行时新增 `cursor`；去掉对已退役的 `autome` 命令行（`autome go` / `approve` / `roles`）的依赖，卡点放行并入 `loop-run`。
