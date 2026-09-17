---
description: 从指定入口启动 Loop 循环会话（协议自驱动接力；入口 task1/review/adjudicate/impl/audit）
argument-hint: "<slug> <task1|review|adjudicate|impl|audit> [claude|codex]（默认按 loop-roles.conf 角色路由）"
allowed-tools: Bash, Read, Glob
---

按参数从指定入口启动一个循环会话；此后协议自驱动接力，直至终止或（gated 模式下）停靠等人工放行。

**参数解析**：完整参数串为：`$ARGUMENTS`
按空白拆分：第 1 个词 = slug；第 2 个词 = 入口（task1 / review / adjudicate / impl / audit）；第 3 个词 = 运行时（可选；缺省时按 `.autome/skill/loop-roles.conf` 角色路由）。若拆分后 slug 或入口缺失/不合法，列出 `docs/*/` 下可用的任务文件（`docs/<slug>/<slug>-task.md`）并向用户确认，不启动。

1. 前置校验（不满足则告知用户并停止，不启动）：
   - `docs/<slug>/<slug>-task.md` 存在；
   - 入口合法：`task1` / `review` / `adjudicate` / `impl` / `audit`；
   - 若入口为 `impl` 且设计文档 `docs/<slug>/<slug>.md` 的 `status` 不是 `实现中`，提醒用户确认设计已通过（正常流程由裁决轮自动进入实现，手动 impl 入口用于人工处理后续跑）。

2. 入口 → prompt 映射（逐字，不得改写）：
   - `task1`      → `Please execute docs/<slug>/<slug>-task.md Task 1.`
   - `review`     → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 1.`
   - `adjudicate` → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2.`
   - `impl`       → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 3.`
   - `audit`      → `Please execute docs/<slug>/<slug>-task.md Task 1 additional task 4.`

3. 用项目内的启动脚本拉起。未显式指定运行时且 `.autome/skill/new_session.sh` 存在时用它（按 `.autome/skill/loop-roles.conf` 角色路由到配置的 CLI 与模型）；显式指定 claude / codex 时改用对应的 `new_claude_session.sh` / `new_codex_session.sh` 强制该运行时：

```bash
bash .autome/skill/new_session.sh "<上面映射出的 prompt>"
```

4. 把脚本输出（LAUNCHED / REFUSED / PARKED、pid、日志路径）如实转述。REFUSED 表示 kill switch 已拉下，提示用 `/loop:stop resume` 恢复；PARKED 表示人工卡点停靠，提示任意终端 `autome approve <slug>` 放行。启动成功后提示：`/loop:status <slug>` 可查看进展；循环自驱动，无需再逐阶段启动。
