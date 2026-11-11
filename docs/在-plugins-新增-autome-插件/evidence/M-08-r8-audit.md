# M-08 · 审计 #8 独立复验

- 验收命令 `sh plugins/autome/tests/run.sh skill`：13/13（基线 T1–T10 加 T11–T13，差 3 项即 M-08 新增时序检查）。
- 负向对照（逐个注入后跑 skill 组，再从备份还原）：
  - 第 1 步加入 `git rev-parse --short HEAD` → T11 失败
  - 第 3 步加入 `autome version` → T11 失败
  - 第 2 步在 `command -v autome` 之前先写 `autome version` → T11 失败
  - 删去「用户明确同意后才写入」→ T13 失败
- 还原后 `git status` 干净；`run.sh all` 62/62，`verify-cli.sh skill` 6/6，`hosts.sh` 5/5。
- 语义核对：第 1 步「不跑命令」且仅 Bug 环境缺失留待第 2 步；第 2 步先 `command -v autome` 再采集，须用户同意才写入，失败/空/模糊回第 1 步追问，并声明环境确认不等于创建确认。符合 M-08 契约。
- 人工项 H-07 已在 `## 人工验收清单`，由用户合并前确认。
