---
description: 把一句话需求生成为自包含循环任务文件 docs/<slug>/<slug>-task.md（自动读取/建立项目画像）
argument-hint: "<一句话需求，如：修复 xxx / 实现 xxx>"
---

读取 `${CLAUDE_PLUGIN_ROOT}/skills/loop-task/SKILL.md`，按其中步骤执行。用户附带的参数：$ARGUMENTS

该文件里说的「本 skill 目录」就是 `${CLAUDE_PLUGIN_ROOT}/skills/loop-task/`。本命令只是 Claude Code 里的短别名；三个宿主（Claude Code / Codex / Cursor）共用的行为都写在 skill 里。
