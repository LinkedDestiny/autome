---
name: loop-init
description: 在当前项目初始化 Loop 工程——把会话启动脚本、角色路由表、kill switch、测试方法论装进 .autome/，并在 AGENTS.md 追加 Loop 章节。用户要求初始化 Loop、装 loop 插件到项目、或 loop-task 提示项目未初始化时使用。
argument-hint: "[--force]"
disable-model-invocation: true
---

# 初始化 Loop 工程

**参数**：用户调用本 skill 时附带的文本。只认一个可选参数 `--force`（覆盖已存在的运行时文件）；其余内容忽略。

## 步骤

1. 运行初始化脚本。脚本在**本 SKILL.md 所在目录**下的 `scripts/init.sh`（Claude Code 中即 `${CLAUDE_SKILL_DIR}/scripts/init.sh`；Codex 会给出本文件的绝对路径，取其目录即可）。在项目根执行：

   ```bash
   bash "<本 skill 目录>/scripts/init.sh"            # 幂等，已存在的文件跳过
   bash "<本 skill 目录>/scripts/init.sh" --force    # 用户带了 --force 时
   ```

2. 把脚本输出（NEW / SKIP / UPDATE / APPEND 清单）如实转述给用户。

3. 检查项目画像 `docs/agent-project-profile.md`：
   - 已存在：告知将复用，列出它的小节标题供用户确认。
   - 不存在：说明首次运行 loop-task 时会按画像规范调研起草（默认起草后停下等用户确认）；也可以现在就起草——由用户决定。
   - 当前目录几乎是空项目（无代码可分析）时，先问用户项目定位。

4. 给出下一步：
   - `loop-task <一句话需求>` 生成任务文件；
   - `loop-run <slug> task1` 启动循环（加 `--gated` 在两个人工边界停靠）；
   - `loop-status` 看进度，`loop-stop` 紧急停止。
   - 角色用哪个 CLI 与模型：编辑 `.autome/skill/loop-roles.conf`（`role=claude|codex|cursor[:model]`）。
