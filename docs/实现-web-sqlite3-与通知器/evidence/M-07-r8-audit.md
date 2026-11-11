# M-07 审计复验（审计 #7）

结论：通过。

负向对照（均已还原，git 工作区干净）：
1. 在补齐前就 `states.set(...)` 提前更新最近状态 → `test/notifier.test.ts` 的「补齐无 code 的离线/超时错误…」失败（1 failed / 10 passed）。
2. 补齐失败时 `save(event.seq)` 而不是保留 `cursor` → 同一用例失败（1 failed / 10 passed）。
验收用例因此能区分错误实现。

验收复验：
- `npm test -- test/notifier.test.ts`：11 通过。该用例覆盖离线（无 code）与超时（timeout:true）两种错误，停在事件前（游标 1、无通知），恢复后重试同一事件恰好 1 条通知、推送 1 次、游标 2。
- 全量：web 33 文件 / 301 用例全过；lint、typecheck 无错；desktop 104 用例，103 过、1 跳过（基线自带）。
- 宿主门禁 `ci-local.sh` 在 93bc1412a5 上通过（含 Web lint/typecheck/test/build）。
