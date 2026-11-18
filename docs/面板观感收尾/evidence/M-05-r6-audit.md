# M-05 r6 审计复验

- 验收命令：6 个文件 73 用例通过、0 失败（契约下限 73）。
- 负向对照（均已还原，git status 干净，还原后 47 用例复绿）：
  1. 保存按钮文字改回「保存{字段}」→ loop-defaults 新增条失败。
  2. noSkills 空态文案清空 → loop-page 新增条失败。
  3. `.settings-card-body` 恢复 `flex:1 1 0` + `overflow-y:auto` → w9-layout 设置页守卫失败。
  3b. 区块自身加 `data-scroll-region` → settings-config 新增条失败。
- 另查：e2e/layout.spec.ts 的项目设置用例含三区块 `scrollHeight ≤ clientHeight+1` 与 `#remove-name` 断言；`playwright test --list` 可列出。浏览器执行交宿主门禁。
- 新造检查：上述缺陷注入，结果均能区分。无实现缺陷、无验证缺口。
