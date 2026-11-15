# M-07 r13 审计证据（审计 #9）

结论：通过。

- 负向对照：把旧版 1200×451 dashboard.png（取自 3eae3213^）放回 docs/images 后，readme-images 尺寸用例失败（3 例中 1 败 2 过）；已还原，git status 干净。
- 验收命令全段退出 0：cli `web_` 10 例、`--bin autome` 67 例、构建、截图脚本两次（各 3 张 1512×944）、vitest readme-images + test-groups 共 7 例通过。
- `docs/images/` 仅 dashboard/project/task 三张，均 1512×944；README 只引用这三张；task.png 目视为真实渲染的任务页。
- 新造检查：旧图回放负向对照（见上），结果如预期。
- 操作者备注：简报无该节。
