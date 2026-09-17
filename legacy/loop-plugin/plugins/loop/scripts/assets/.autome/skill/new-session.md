# 启动新的会话（按角色路由）

当协议要求你「启动一个新的 chat」时，唯一正确的做法是运行：

```bash
bash .autome/skill/new_session.sh "<协议指定的 prompt 原文>"
```

例如：

```bash
bash .autome/skill/new_session.sh "Please execute docs/my-task/my-task-task.md Task 1 additional task 2."
```

脚本会从启动语句自动推导目标角色（additional task 1→review、2→adjudicate、3→impl、4→audit、无→plan），按 `.autome/skill/loop-roles.conf` 把会话路由到该角色配置的运行时与模型（默认全部 claude）。

规则：

1. prompt 必须逐字使用协议条文中给出的启动语句，不要改写、不要附加说明。
2. 脚本会在后台拉起新会话并立即返回（输出 `LAUNCHED: ...`）。看到该输出后，**你的本轮任务即告结束**——不要等待新会话、不要轮询它的日志、不要重复启动。
3. 如果脚本输出 `REFUSED`（kill switch 已拉下）、`PARKED`（人工卡点停靠，放行由人工执行 `autome approve` 完成）或 `SAME-MODEL`（生成/评测角色模型未分开，需人工修改 loop-roles.conf），如实记录后直接结束，不要重试、不要绕过脚本自行启动会话。
4. 角色的运行时与模型由人工编辑 `.autome/skill/loop-roles.conf` 决定；该文件是人工维护的配置，你不得修改它。生成与评测角色强制使用不同模型（plan≠review、impl≠audit），由脚本校验。
5. 会话日志在 `.autome/output/sessions/` 下，启动台账在 `.autome/output/sessions/launches.jsonl`，仅供人工排查，你不需要读取。
