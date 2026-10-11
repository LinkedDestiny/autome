---
name: loop-run
description: 启动或放行 Loop 循环会话——从指定入口（task1/review/adjudicate/impl/audit）拉起一个自治会话，此后由协议自驱动接力；也负责人工卡点的放行与驳回。用户要求启动、继续、放行 loop 任务时使用。
argument-hint: "<slug> <task1|review|adjudicate|impl|audit> [claude|codex|cursor] [--gated] | <slug> approve | <slug> reject"
disable-model-invocation: true
---

# 启动 / 放行 Loop 会话

**参数**：用户调用本 skill 时附带的文本，按空白拆分。三种形式：

| 形式 | 作用 |
|---|---|
| `<slug> <入口> [运行时] [--gated]` | 从入口拉起一个会话；此后协议自驱动接力 |
| `<slug> approve` | 放行停靠在人工卡点的会话 |
| `<slug> reject` | 驳回停靠的会话，循环就此停下 |

参数缺失或不合法时，列出 `docs/*/` 下的任务文件（`docs/<slug>/<slug>-task.md`）请用户确认，不启动任何会话。

## 启动：`<slug> <入口> [运行时] [--gated]`

1. 前置校验（不满足则告知用户并停止）：
   - `docs/<slug>/<slug>-task.md` 存在；`.autome/skill/new_session.sh` 存在（否则先 loop-init）。
   - 入口是 `task1` / `review` / `adjudicate` / `impl` / `audit` 之一。
   - 入口为 `impl` 而设计文档 `docs/<slug>/<slug>.md` 的 `status` 不是 `实现中`：提醒用户确认设计已通过（正常流程由裁决轮自动进入实现；手动 impl 用于人工处理后续跑）。

2. 卡点模式：带 `--gated` 时写入 `.autome/output/gates/<slug>.mode`，内容为一行 `gated`；不带时，若该文件存在则改写为 `auto`。gated 模式下，启动脚本在「plan→评审」「设计→实现」两个边界停靠等放行。

3. 入口 → prompt（逐字，不得改写）：
   - `task1`      → `Please execute docs/<slug>/<slug>-task.md Task 1.`
   - `review`     → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 1.`
   - `adjudicate` → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2.`
   - `impl`       → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 3.`
   - `audit`      → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 4.`

4. 在项目根拉起。未指定运行时：用按角色路由的 `new_session.sh`（读 `.autome/skill/loop-roles.conf`）。指定了 `claude` / `codex` / `cursor`：用对应脚本强制该运行时。

   ```bash
   bash .autome/skill/new_session.sh "<prompt>"
   bash .autome/skill/new_claude_session.sh "<prompt>"   # 或 new_codex_session.sh / new_cursor_session.sh
   ```

5. 如实转述脚本输出：
   - `LAUNCHED`：给出 pid 与日志路径；提示用 loop-status 看进展，循环自驱动，无需逐阶段启动。
   - `REFUSED`：kill switch 已拉下，提示 `loop-stop resume` 恢复。
   - `PARKED`：停靠在人工卡点，提示 `loop-run <slug> approve` 放行或 `reject` 驳回。
   - `SAME-MODEL` / `BAD-ROLES-CONF` / `MISSING-CLI`：说明原因（生成与评测同模型 / 路由表写错 / CLI 未安装），请用户处理，不要绕过。

## 放行：`<slug> approve`

1. 读 `.autome/output/gates/<slug>.parked`。不存在：告知用户当前没有停靠，停止。
2. 文件内容就是被停靠的启动 prompt。先删除该文件，再带放行变量重新拉起：

   ```bash
   LOOP_GATE_OVERRIDE=1 bash .autome/skill/new_session.sh "<parked 文件里的 prompt>"
   ```

3. 如实转述输出（同上）。

## 驳回：`<slug> reject`

删除 `.autome/output/gates/<slug>.parked`，告知用户循环已停在该边界；之后可修改设计文档或任务文件，再用 `loop-run <slug> <入口>` 从相应入口重新拉起。
