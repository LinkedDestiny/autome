# M-01 审计复验（实现轮 r2）

未读实现轮证据，独立复验。

## 验收命令
`cd apps/web && npx vitest run test/board.test.ts test/styles.test.ts test/bell-view.test.ts test/live-routes.test.ts test/task-columns.test.ts` → 5 文件、40 用例通过、0 跳过，与预期一致。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| `board-data.ts` 包一层 `columnOf`，把 `paused` 归进行中 | board (1)、(4)、(8) 失败（契约要求 (1)、(8)） |
| `bell-targets.ts` 的 `/projects/:projectId` 改 `available: false` | bell-view 两个用例失败（含「目标可用性与真实路由表一致」） |
| `filterBoard` 把 `total` 改成过滤后数量 | board (4) 失败 |
| 自造：14 天边界 `<=` 改 `<` | board (3) 失败 |

四项变异均被拦住，验收命令有判别力。

## 结论
通过。无产品缺陷。全量门禁由宿主合并前统一运行。
