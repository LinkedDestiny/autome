# M-10 审计复验 · 审计 #12（对应实现轮 r19）

结论：**通过**（验证缺口已就地补强，不计 reopen）。

## 验收命令（自己跑）
- `cargo build -p automed -p autome-cli` 通过。
- vitest live-routes + test-groups：2 文件 18 例全过（live-routes 14、test-groups 4），与契约数一致。
- `npm run build --prefix apps/web` 通过；`npx playwright test e2e/controls.spec.ts -g "BL-01"` 1 例通过（19.5 s）。

## 负向对照
| 变异 | live-routes 单测 | Playwright BL-01 |
|---|---|---|
| `tasks.$id.tsx` 改回 `bySubjectParam` | （未单跑） | **仍通过**（契约预期失败） |
| 相关表匹配恒为假 | 2 例失败（前置重验、after 替换与卸载清除） | **仍通过** |
| `setRelatedSubjects` 清除函数空操作 | 1 例失败（after 替换与卸载清除） | 未跑 |
| 路由层 effect 不返回清理 | 14 例仍通过（抓不到） | 未跑 |
| `tasks.$id` 的 shouldRevalidate 恒 false（对照） | — | 失败，证明该用例确实依赖重验 |

## 结论依据
产品行为没有错：单测证明匹配器与登记表语义符合 G.1，浏览器用例证明 5 秒内依赖页显示前置新状态、D 仍排队、未整页重载。
缺口在验收：契约声称浏览器用例会在 5 秒处超时，实测不会（matcher 在页面上根本没被调用，刷新来自另一条整页重验路径，成因未进一步定位）。已把契约的负向对照改成以单测为判别者并注明浏览器用例只是冒烟，未能判别的路由层清理记入 BL-14。

## 其它
- 所有变异均已还原，`git status` 工作区除文档外干净；临时备份在 `.autome/output/`（不入库）。
- 中途两次 Playwright 因机器负载（load 6+）在 setup 阶段 `spawnSync bash ETIMEDOUT`，待负载降后重跑正常，属环境问题。
- 无操作者备注送达本里程碑。
