---
description: 在当前项目初始化 Loop 工程（启动脚本、SESSION_SKILL、kill switch、目录骨架）
argument-hint: "[--force]"
allowed-tools: Bash, Read, Write, Glob, Grep
---

在当前项目初始化 Loop 工程。步骤：

1. 运行初始化脚本（幂等；`$ARGUMENTS` 中若含 `--force` 则透传）：

```bash
bash "${CLAUDE_PLUGIN_ROOT}/scripts/init.sh" $ARGUMENTS
```

2. 将脚本输出（NEW/SKIP/APPEND 清单）如实转述给用户。

3. 检查项目画像 `docs/agent-project-profile.md`：
   - 已存在：告知用户将复用，展示其小节标题清单供确认。
   - 不存在：说明画像将在首次 `/loop:task` 时按 `loop:loop-task` skill 的画像规范调研起草（默认起草后停下等用户确认）；也可现在就按该规范生成——由用户决定。
   - 若当前目录几乎是空项目（无代码可分析），先询问用户项目定位。

4. 最后给出下一步指引：`/loop:task <一句话需求>` 生成任务文件；`autome go <slug>` 启动全自动循环（`--gated` 启用人工卡点）；`/loop:run <slug> task1` 仅启动首会话；`/loop:stop` 紧急停止。
