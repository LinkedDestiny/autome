---
description: 把一句话需求生成为自包含循环任务文件 docs/<slug>/<slug>-task.md（融合协议，自动读取/建立项目画像）
argument-hint: "<一句话需求，如：修复 xxx / 实现 xxx>"
---

使用 Skill 工具调用 `loop:loop-task`，args 为：$ARGUMENTS

严格按该 skill 的流程执行：前置检查项目已初始化（否则先建议 `/loop:init`）→ 读取或建立项目画像 `docs/agent-project-profile.md`（缺失时按 schema 调研起草，默认停下等用户确认）→ 调研整理任务 → 逐字套用模板生成 `docs/<slug>/<slug>-task.md`（画像全文嵌入）→ 自检 → 交付（默认不启动；提示 `autome go <slug>` 或 `/loop:run <slug> task1`）。
