# AGENTS.md

本仓库对 AI Agent 的项目级规范。

<!-- autome:begin -->
## Autome

本仓库由 Autome 驱动 Loop 开发。运行中的会话必须遵守：

- **不要修改 `.autome/` 下的任何内容。** 配置、规则与启动脚本由用户在 Autome 界面或编辑器中维护；任务内的改动会被下一次初始化覆盖，也不会被 Autome 读取。
- 本轮工作只在当前 worktree 内进行，不要切换分支、不要操作 `.worktree/` 下的其它目录、不要推送远端。
- 完成本轮后结束会话即可，**不要自行启动下一个会话**——下一个节点由 Autome 调度。
- 项目级规范见 `.autome/rules/` 下的文件，与本文件同等效力。
- 超过 5 行的命令输出、临时验证产物写入 `.autome/output/`，不要进入任务文档目录。
<!-- autome:end -->
