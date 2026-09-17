# loop — 本地自动化 Loop 工程插件

把一句话需求变成全自动收敛的「设计 → 评审/裁决 → 实现/审计」自驱动循环。协议为 [cycle-runner](https://github.com/sskyy/cycle-runner) 融合版（cycle-runner 骨架 + autome 嫁接机制），源流可溯至 Mesh 的 `mesh-task-prompt`。

## 安装

```bash
claude plugin marketplace add <本仓库路径或 git 地址>
claude plugin install loop@autome
```

## 命令

| 命令 | 作用 |
|---|---|
| `/loop:init [--force]` | 初始化当前项目：装入启动脚本、SESSION_SKILL、测试方法论、kill switch，建目录骨架 |
| `/loop:task <一句话需求>` | 生成自包含任务文件 `docs/<slug>/<slug>-task.md`（调用 `loop:loop-task` skill；自动读取/建立项目画像） |
| `/loop:run <slug> <task1\|review\|adjudicate\|impl\|audit> [claude\|codex]` | 从指定入口拉起循环会话；此后协议自驱动接力 |
| `/loop:status [slug]` | 查看 kill switch、卡点、设计文档状态块（status/轮次/里程碑/收敛模式）、会话台账 |
| `/loop:stop [resume]` | 拉下 / 恢复 kill switch |

## 组成

```
.claude-plugin/plugin.json   manifest
commands/                    上述 5 个命令
skills/loop-task/            任务生成 skill + template.md（融合协议全文，唯一维护处）
  └── references/            项目画像 schema + 会话后端说明
scripts/init.sh              项目初始化（幂等，--force 覆盖）
scripts/assets/              装入项目的运行时文件（0.7.0 起集中在 .autome/）：
  .autome/skill/             会话启动脚本（角色路由/claude/codex）+ SESSION_SKILL 文档 + loop-roles.conf
  .autome/skill/testing/     测试设计方法论
  .autome/SCHEDULE.json      kill switch
  AGENTS.loop.md             追加到项目 AGENTS.md 的 Loop 章节
```

初始化同时在项目根创建兼容符号链接（`prompt/skill`、`prompt/output`、`SCHEDULE.json` → `.autome/` 内对应路径）：旧任务文件快照与外部工具（如 autome-dashboard）经根部路径照常工作；0.6.x 旧布局项目重跑 init 自动迁移（mv + 符号链接，不丢数据）。生成与评测角色强制不同模型（`plan≠review`、`impl≠audit`，违反被 `SAME-MODEL` 拒绝）。

## 设计要点

- **协议自包含**：执行任务的 agent 不加载插件，项目画像与协议全文随任务文件进入每个 session。运行时脚本装在项目内（`.autome/skill/`），任务执行不依赖插件存在。
- **会话接力**：每轮结束时 agent 用启动脚本拉起下一个角色的新会话（启动即结束）；默认经 `new_session.sh` 按 `loop-roles.conf` 把每个角色路由到各自配置的客户端与模型（`role=runtime[:model]`）。
- **收敛判据可机判**：设计循环只认六类复审条件，无采纳问题即通过并自动进入实现；实现循环靠里程碑三态 + reopen 收敛模式 + 停滞保险丝，预算 = 5 × 里程碑数。
- **人工卡点可选**：协议本体零人工介入；`autome go --gated` 时启动脚本在 plan→评审、设计→实现两个边界停靠（`PARKED`），`autome approve` 放行。
- **kill switch**：`.autome/SCHEDULE.json` `autorun=false` 时启动脚本拒绝接力，是唯一的手动刹车。
