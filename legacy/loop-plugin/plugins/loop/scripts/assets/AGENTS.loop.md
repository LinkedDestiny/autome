<!-- loop-plugin:begin -->
## Loop 工程（loop plugin）

本项目启用了「设计 → 评审/裁决 → 实现/审计」自驱动收敛循环（cycle-runner 融合协议）。循环运行时集中在 `.autome/` 目录。

给正在执行 `docs/<slug>/<slug>-task.md` 的 agent：一切行为以**任务文件内的协议为准**，本节只是背景。三条仓库级约束：

1. 需要启动新会话时，只能使用任务文件中 SESSION_SKILL 指向的 `.autome/skill/new-*session.md` 所述方法，禁止自行拉起进程。启动脚本输出 `REFUSED`（kill switch）、`PARKED`（人工卡点停靠）或 `SAME-MODEL`（生成/评测角色模型未分开）时如实记录后结束，不要重试或绕过。
2. 任务流程文件固定在 `docs/<slug>/`：`<slug>.md`（设计，头部状态块 + Backlog/争议项）、`<slug>-review.md`（每轮覆盖）、`<slug>-adjudication.md`（append-only 裁决记录）、`<slug>-audit.md`（每轮覆盖）、`retro.md`（每轮追加）。临时验证产物与长输出写 `.autome/output/`。
3. 项目画像 `docs/agent-project-profile.md` 与 `.autome/skill/loop-roles.conf` 是人工维护的资产，agent 不得修改。

kill switch：`.autome/SCHEDULE.json` 的 `autorun` 不为 `true` 时，启动脚本拒绝拉起任何新会话。
<!-- loop-plugin:end -->
