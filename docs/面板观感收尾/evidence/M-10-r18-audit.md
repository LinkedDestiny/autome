# M-10 审计复验（审计 #11）

## 负向对照（先跑）
| 注入 | 命令 | 结果 |
|---|---|---|
| `env.tsx` 的 `open={!ok}` 改为 `open={true}` | `vitest run test/env-page.test.ts` | 3 个用例失败（含「正常五行标记且安装说明默认收起」） |
| `.path-suggestions` 加回 `max-height: 160px` | `vitest run test/add-project.test.ts` | 1 失败 17 通过（候选列表占满剩余高度） |
| `.board-column-body` 去掉 `flex: 1 1 0` | `vitest run test/styles.test.ts` | 1 失败 15 通过（看板守卫） |

三处均已还原，`git status` 干净。

## 验收命令
`(cd apps/web && npx vitest run test/env-page.test.ts test/add-project.test.ts test/board.test.ts test/board-cancelled.test.ts test/styles.test.ts test/w9-layout.test.ts test/labels.test.ts)`：7 文件 88 用例通过、0 失败（预期 ≥ 86）。

## 独立核对
`toolOk`（env-model.ts:22）要求存在、登录为 ok/not_applicable、不在 problems，`env.tsx:67` 的 `open={!ok}` 与之一致，覆盖未安装、登录过期、问题列表三种展开情形。

## 未取证
浏览器渲染（看板底部「已取消」分组不被裁、候选列表可见行数）沙箱跑不了，已有 H-06 等人工项与门禁 e2e 覆盖。
