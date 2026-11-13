# M-03 · 实现轮 #4 · 审计复验

结论：通过。本轮环境可监听 127.0.0.1（listen 探针 ok），3 条 live 用例都在真实 automed + 假 CLI 上跑了。

## 验收命令
`cd apps/web && npm test -- test/logtail-live.test.ts test/core-sidecar.test.ts`：退出码 0，2 个文件 21 个用例通过（新增 3 + core-sidecar 18），skipped 0，与契约预期一致。
lint 退出码 0，typecheck 退出码 0。live 文件另外连跑 2 次均 3/3 通过（无抖动）。

## 负向对照（每次注入后还原，`git status --porcelain` 为空）
| 注入 | 结果 |
|---|---|
| main.ts `close()` 不调 `logtail.close()` | 第 3 条红（2 过 1 败） |
| logtail.ts 读完不 `handle.close()` | 第 2、3 条红（lsof 断言） |
| `intervalMs` 500 → 5000 | 第 1、2、3 条红（第 1 条 3000ms 超时） |

三个契约点名的缺陷都被拦住，验收命令有判别力。

## 自造检查
上面三项即判别检查；未发现产品缺陷，不留文件。
