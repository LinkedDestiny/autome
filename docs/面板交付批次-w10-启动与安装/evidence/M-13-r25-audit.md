# M-13 r25 审计 · 集成复验（提交 235b6e2）

结论：通过，里程碑表不动。

- 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' 235b6e2 -- . ':(exclude)docs/…'` 为空。
- rebase 丢失提交：简报列 0 个。
- 五个测试文件 diff 逐个看：
  - env-page 7 / live-status / settings-onboarding 7：改成假时钟，断言 999ms 未触发、1000ms 触发一次，比原断言更严（原来只测「会触发」）。
  - shell-data 6：root loader 现在多返回 `clock`（root.tsx:15 既有产品行为），测试固定系统时间后逐字段比对，没有放宽。
  - web-launcher 并发 start：由「立即 1 个服务」改为 3s 内收敛到仅 registered.pid；若有第二个常驻服务仍会失败。已在本机跑过 12 条全过。
- 负向对照：把 `app/lib/live-routes.ts:23` 的批处理窗口 1000 改成 0、再改成 2000，env-page 7、live-status、settings-onboarding 7 三条都失败；已还原（git status 干净）。
- 验收命令 `npx vitest run test/timeline-failure-path.test.ts test/task-timeline.test.ts test/labels.test.ts`：3 文件 35 条全过。
- 四个改动单测文件 26 条全过；web-launcher 12 条全过。
- 新造检查（窗口注入 0/2000）：通过后丢弃。
