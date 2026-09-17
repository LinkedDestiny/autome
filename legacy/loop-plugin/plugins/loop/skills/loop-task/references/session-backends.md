# 新会话启动后端

循环任务通过新的独立会话分离设计、评审、实现和审计角色。任务文件包含完整协议；会话启动文件只负责在仓库根拉起指定运行时并提交提示词（启动即结束，不等待）。

## 后端

| 用户指定 | 说明文件 | 脚本 | 行为 |
|---|---|---|---|
| 默认、角色路由 | `.autome/skill/new-session.md` | `.autome/skill/new_session.sh` | 从启动语句推导角色（Task 1→plan、at1→review、at2→adjudicate、at3→impl、at4→audit），按 `.autome/skill/loop-roles.conf` 路由到该角色配置的 CLI 与模型 |
| Claude | `.autome/skill/new-claude-session.md` | `.autome/skill/new_claude_session.sh` | 全程强制 Claude Code CLI |
| Codex | `.autome/skill/new-codex-session.md` | `.autome/skill/new_codex_session.sh` | 全程强制 Codex CLI |

后端文件由插件 `scripts/init.sh` 统一安装到仓库 `.autome/skill/`（幂等，存在且可用时不覆盖）。生成任务文件时只需确认所选后端的说明文件存在；缺失时先补跑 `/loop:init`。

## 配置

- `.autome/skill/loop-roles.conf`：行格式 `role=runtime[:model]`（如 `review=codex:gpt-5.3-spark`、`impl=claude:sonnet`）。人工维护或用 `autome roles set` 修改；agent 不得修改。缺文件/缺行回退 `claude` 默认模型。
- **双模型纪律（强制）**：生成侧与评测侧必须配置不同的 `runtime[:model]`——`plan ≠ review`、`impl ≠ audit`。相同配置时 `new_session.sh` 输出 `SAME-MODEL` 并拒绝启动（`LOOP_ALLOW_SAME_MODEL=1` 仅供调试跳过）。目的：同一模型自产自评共享盲区，且容易在细小问题上自我强化、偏离任务目标。
- 模型经 `LOOP_MODEL` 透传为 `claude --model` / `codex -m`。
- 权限模式可用 `LOOP_CLAUDE_FLAGS` / `LOOP_CODEX_FLAGS` 覆盖（默认全自治）。
- `{{SESSION_SKILL}}` 使用说明文件的仓库相对路径。

## 启动示例

```bash
bash .autome/skill/new_session.sh "Please execute docs/<slug>/<slug>-task.md Task 1."
bash .autome/skill/new_session.sh --dry-run "Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2."   # 只打印不拉起
```

## 状态传递与闸门

- 所有会话共享同一个工作目录，不要求每轮创建协议专用提交。
- 启动脚本内置闸门链：角色路由与双模型校验（`new_session.sh`，非法 runtime 输出 `BAD-ROLES-CONF`、生成/评测同模型输出 `SAME-MODEL`，均拒绝且 dry-run 也拦）→ dry-run 短路 → kill switch（`.autome/SCHEDULE.json` 的 `autorun` 非 true 时拒绝）→ gate 停靠（gated 模式下在 plan→评审、设计→实现两个边界停靠等 `autome approve`）→ runtime 校验 → 后台拉起并记录台账（`.autome/output/sessions/launches.jsonl`）。
- 协议文件负责终止条件；启动脚本不自行重复启动会话。
