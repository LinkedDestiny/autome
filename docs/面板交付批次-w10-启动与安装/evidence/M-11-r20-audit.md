# M-11 审计复验（实现 #20 之后，审计 #19）

结论：通过。

## 验收命令
- vitest 6 个文件：53 条通过、0 跳过（契约预期 50 条，实际多 3 条：基线已高于契约推算，不是缺陷）。
- `npx playwright test e2e/layout.spec.ts`：先重新 `npm run build`（e2e 起的是生产构建，不重建则测的是旧产物），12 条通过、0 跳过。

## 负向对照（逐条注入、跑 vitest、`git checkout -- app` 还原）
| 注入 | 结果 |
|---|---|
| 总览卡片加回 `.task-status` 第二行 | overview 第 6 条失败 |
| `columnOf('cancelled')` 归入 done | board-cancelled 第 5 条、board 第 6 条失败 |
| loader 缺省也带已取消对象 | board-cancelled 第 1 条失败 |
| 已取消链接丢掉 `done=all` / 已完成链接丢掉 `cancelled=all` | board-cancelled 第 4 条失败（两方向各一次） |
| `logName` 返回完整路径 | task-timeline 3 条失败 |
| 只让 `setup.started` 写完整路径 | task-timeline 新增第 2 条失败 |
| 去掉列体 `data-scroll-region`（重建后） | layout「已取消展开」失败；还原重建后通过 |

首次做布局注入时没重建，用例仍通过——证明 e2e 测的是构建产物；重建后注入被拦住。所有注入已还原，`git status` 干净。

## 其它
- 清理了 4 个残留的 e2e `serve.ts` 进程（此前运行遗留）。
- 未发现产品缺陷。无人工验收项。
