# 实现审计

## M-01 · 通过（审计 #1）
验收命令 19 个用例全过；两项负向对照（去掉锁检查 / 去掉 Merging 等锁）均使对应 e2e 用例失败，已还原。无实现缺陷、无验证缺口。
证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r2-audit.md

## M-02 · 通过（审计 #2）
验收命令 17 个用例全过；三项负向对照（merge_press 接受 Failed / cancel_press 放行 Merging / 预先同意应用 Approve）均使对应用例失败，已还原。无实现缺陷、无验证缺口。
证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-02-r3-audit.md

## M-03 · 通过（审计 #3）
验收命令 6 个用例全过，fmt/clippy 通过；三项负向对照（需求侧冲突落通用分支 / Bug 侧分支失配 / 冲突前先写 gate.started）均使对应用例失败，已还原。无实现缺陷、无验证缺口。
证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-audit.md

## M-04 · 通过（审计 #4）
验收命令 6 个用例全过，fmt/clippy 通过；两项负向对照（behind_count 范围写反 / 不限定 AwaitMerge）均使对应用例失败，已还原。无实现缺陷、无验证缺口。
证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r5-audit.md
