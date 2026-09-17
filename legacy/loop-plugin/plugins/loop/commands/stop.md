---
description: 紧急停止 Loop（拉下 kill switch，接力即中断）；loop:stop resume 恢复
argument-hint: "[resume]"
allowed-tools: Bash, Read, Edit
---

操作 kill switch（`.autome/SCHEDULE.json` 的 `autorun` 字段）。

**参数解析**：完整参数串为：`$ARGUMENTS`（可为空）。

- 参数串为空或不含 `resume` → **停止**：把 `autorun` 置为 `false`。正在运行的会话不会被杀死，但它结束时的接力启动会被脚本拒绝（REFUSED），循环随之中断。如需立即终止，向用户展示运行中的会话（`ps aux | grep -E "claude -p|codex exec" | grep -v grep`）并给出 kill 命令，**由用户决定是否执行**。
- 参数串含 `resume` → **恢复**：把 `autorun` 置回 `true`，并提示用 `/loop:run <slug> <入口>` 从中断处重新拉起（协议是幂等的：评审/裁决/审计轮会从文档现状继续；入口 task1/review/adjudicate/impl/audit）。

用 Edit 修改 `.autome/SCHEDULE.json`（保持 JSON 合法），改完读回确认并报告当前值。
