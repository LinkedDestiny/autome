---
description: 从指定入口启动 Loop 循环会话，或放行/驳回人工卡点
argument-hint: "<slug> <task1|review|adjudicate|impl|audit> [claude|codex|cursor] [--gated] | <slug> approve | <slug> reject"
---

读取 `${CLAUDE_PLUGIN_ROOT}/skills/loop-run/SKILL.md`，按其中步骤执行。用户附带的参数：$ARGUMENTS

该文件里说的「本 skill 目录」就是 `${CLAUDE_PLUGIN_ROOT}/skills/loop-run/`。本命令只是 Claude Code 里的短别名；三个宿主（Claude Code / Codex / Cursor）共用的行为都写在 skill 里。
