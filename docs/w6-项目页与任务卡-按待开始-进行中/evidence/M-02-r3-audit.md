# M-02 审计复验（实现轮 r3）

未读实现轮证据，独立复验。

## 验收命令
`cd apps/web && npx vitest run test/task-card.test.ts test/board-detail-real.test.ts test/labels.test.ts test/overview.test.ts` → 4 文件、27 用例通过、0 跳过，与预期一致（真实内核二进制存在）。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| Bug 卡读 `status_block` 进度（并去掉修复轮次显示） | task-card (4) 失败 |
| `gate` 为空时显示「门禁通过」 | task-card (8)、board-detail-real (1) 失败 |
| `needsTaskDetail` 去掉 draft 排除 | task-card (10)、board-detail-real (2) 失败 |
| `done` 卡不取详情 | task-card (10) 失败（契约另写 (8)：(8) 测的是纯函数 cardView，不经 loader，故不失败；(10) 已覆盖该行为，不构成缺口） |
| `needYouKinds` 排除 `not_a_bug` | task-card (11) 失败 |

五项注入均被拦住，验收命令有判别力。

## 结论
通过。无产品缺陷。全量门禁由宿主合并前统一运行。
