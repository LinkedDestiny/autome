# M-04 审计 #4 独立复验

结论：通过。

验收命令 `sh plugins/autome/tests/run.sh readme && sh plugins/autome/tests/verify-cli.sh readme`：R1–R5 共 5/5、V7–V9 共 3/3。`run.sh all` 54/54（不含沙箱受限的 Web 测试）。

负向对照（逐个改 README 后跑 readme 组，还原后复跑全绿；git status 干净）：
- `id`→`task_id`：R3、V7 失败
- `state: draft`→`pending`：V7 失败
- 删 `codex plugin add autome@autome` 行：R5 失败
- `kind: feature`→`bug`（值错）：V7 失败
- `started: false`→`true`：V7 失败
- 删 `budget_n` 键：V7 失败
- `default_branch: master`→`main`：V8 失败

结论：验收能区分错误实现，V7/V8 比较键与值。未发现产品缺陷。
