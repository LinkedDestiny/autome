# M-04 · 实现轮 #5 · 审计复验

结论：通过。

- 验收命令：4 个文件 28 个用例通过，skipped 0；`npm run lint`、`npm run typecheck` 退出码均为 0。
- 负向对照共 5 条（含契约列出的全部 5 条）。每条都注入后实跑，再还原，最后 `git status --porcelain` 为空：

| 注入 | 变红的用例 |
|---|---|
| `hello` 分发放在置 `ready` 之前 | live-send 第 3 条 |
| 控制器忽略 `hello` | controller 第 1、3 条 |
| 暂停追加仍返回 `scroll: 'bottom'` | model 第 2 条（另有第 10 条） |
| 限量按 UTF-16 长度计 | model 第 5 条（另有第 1 条） |
| 超出 N 的头部直接丢弃 | model 第 9 条（另有第 3、5 条） |

- 自造判别检查（已丢弃）：12 个约 63KB 的多字节 chunk，暂停后再追加，窗口文字保持不变且 `unseen` 为真；`end` 之后 `close` 不发 `unsubscribe`；`top` 起反复 `later` 拼接，等于全文加追加的 `Z`。结果通过。
- 未发现产品缺陷。人工项 H-01..H-05 属于 M-05 的界面验证，不在本里程碑内。
