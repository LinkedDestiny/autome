# 审计 #4 · M-04 · 通过

- 验收命令全链复验：cargo build、vitest 5 文件 50/50、web build 通过；Playwright B-2、B-4 命令行排队通过，B-3 通过（见下，存在测试竞态）。
- 负向对照（均已还原，git status 干净）：排序不用 queueOrder → board 2 例失败；`top` 不组装 → queue-deps 1 例失败且 B-2 浏览器失败；新建丢 `after` → task-new 2 例失败且 B-3 浏览器失败。
- 发现：B-3 在 `state==='active'` 后立即断言 `sessions.length>0`（未轮询），约 4 次中 2 次失败于该行；重跑多次通过。属测试竞态，产品行为（前置合并后依赖任务开始）正确，记 Backlog BL-13，不退回。
- 简报无操作者备注。无实现缺陷。
