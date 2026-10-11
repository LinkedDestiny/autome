---
name: loop-stop
description: 紧急停止或恢复 Loop——拉下 / 恢复 kill switch（.autome/SCHEDULE.json 的 autorun），拉下后会话接力即中断。用户要求停止、暂停、恢复 loop 时使用。
argument-hint: "[resume]"
disable-model-invocation: true
---

# 停止 / 恢复 Loop

**参数**：用户调用本 skill 时附带的文本，可为空。

- **不含 `resume` → 停止**：把 `.autome/SCHEDULE.json` 的 `autorun` 置为 `false`。正在运行的会话不会被杀死，但它结束时的接力启动会被脚本拒绝（`REFUSED`），循环随之中断。如需立即终止，向用户列出运行中的会话（`ps aux | grep -E "claude -p|codex exec|cursor-agent -p|agent -p" | grep -v grep`）并给出 kill 命令，**由用户决定是否执行**。
- **含 `resume` → 恢复**：把 `autorun` 置回 `true`，并提示用 `loop-run <slug> <入口>` 从中断处重新拉起（协议是幂等的：评审、裁决、审计轮会从文档现状继续）。

修改后保持 JSON 合法，读回确认并报告当前值。
