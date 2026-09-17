# 启动新的 Codex 会话

当协议要求你「启动一个新的 chat」时，唯一正确的做法是运行：

```bash
bash .autome/skill/new_codex_session.sh "<协议指定的 prompt 原文>"
```

例如：

```bash
bash .autome/skill/new_codex_session.sh "Please execute docs/my-task/my-task-task.md Task 1 additional task 1."
```

规则：

1. prompt 必须逐字使用协议条文中给出的启动语句，不要改写、不要附加说明。
2. 脚本会在后台拉起新会话并立即返回（输出 `LAUNCHED: ...`）。看到该输出后，**你的本轮任务即告结束**——不要等待新会话、不要轮询它的日志、不要重复启动。
3. 如果脚本输出 `REFUSED`（kill switch 已拉下）或 `PARKED`（人工卡点停靠，放行由人工执行 `autome approve` 完成），如实记录后直接结束，不要重试、不要绕过脚本自行启动会话。
4. 会话日志在 `.autome/output/sessions/` 下，启动台账在 `.autome/output/sessions/launches.jsonl`，仅供人工排查，你不需要读取。
