# M-04 审计复验（实现轮 #5，审计 #4）

结论：通过。

## 验收命令
`cd apps/web && npx vitest run test/task-new.test.ts test/task-new-real.test.ts` → 2 文件、12 用例通过、0 跳过（与契约预期一致）。

## 负向对照（逐条注入、跑、还原）
| 注入 | 结果 |
|---|---|
| Bug 模板末节 `## 线索（可选）` 改 `## 线索` | task-new (2) 失败，其余 11 过 |
| `switchKind` 无条件覆盖正文（返回 true 分支） | task-new (3) 失败 |
| action 去掉 `draft: intent === 'draft'` | task-new (5) 与 task-new-real (2) 失败 |
| 创建成功后按 `uploads.resolve(id)` 删暂存文件 | task-new-real (2) 与 task-new (5) 失败 |

第一次做第四条时误用了 `describe().path`（该方法无 path 字段），注入没生效、12 过；改用 `resolve` 后才真正注入。这是我的注入写错，不是验收漏洞。
全部还原后复跑 12/12，`git status` 无改动。

## 自造检查
无新增检查；契约的负向对照已能区分错误实现。未发现产品行为缺陷。人工项 H-02、H-03 已在清单中，不属本轮。
