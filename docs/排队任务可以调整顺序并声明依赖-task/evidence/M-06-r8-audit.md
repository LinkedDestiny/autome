# M-06 · r8 · 审计 #6 独立复验

结论：通过。

- 验收命令：`write-gate` 47 个通过（基线 44 + 3）；三条 grep 成立（旧句 `排队按 created_at 升序` 已无，`queue move` 在 README，`task.set_after` 在技术设计）。
- 负向对照一：删掉 `ARRAY_FIELDS['task.set_after']` → 3 个用例失败，含「task.set_after 的 after 不允许数组」。已还原。
- 负向对照二：去掉 `after.length > MAX_AFTER_IDS` → 「创建与修改依赖的列表边界…」失败。已还原。还原后 47 个通过，`git status` 干净。
- 自造判别探针（一次性，已删，结果 `.autome/output/probe.json`）：move 零位置、两位置、`top:false`、`before:'x'`、`before:['T-2']`、缺 task_id 均拒绝；仅 `before:'T-2'` 放行；set_after 8 个放行、9 个、缺 after、字符串、`T-01x` 拒绝；task.create 的 after 9 个与非数组拒绝。全部符合设计 §8。
- 文档：技术设计 §8/§13/§14 与 README 与设计 §9 一致；事件与 `task_id` 参数名同内核。
- 无实现缺陷，无验证缺口。人工验收 H-01/H-02 本就在清单，由用户合并前确认。
