# M-02 审计证据（审计 #2）

结论：通过。

## 验收命令
`node --test scripts/test/ci-e2e.test.mjs scripts/test/ci-quick.test.mjs`：共 28 个，27 过 1 败。
败的是 ci-quick 14（`CI_QUICK_BUDGET=1`，依赖前置步骤在 1 秒内跑完）；失败当时机器负载 7～10，单跑 3 次过 2 次，该用例与 M-02 无关（本里程碑只改了它旁边一个计数断言），判为计时脆弱，已写进 Backlog。ci-e2e 13 个全过。

## 负向对照（逐项变异、只跑对应用例、还原后 `git status` 干净）
| 变异 | 用例 | 结果 |
|---|---|---|
| `ci_e2e_admit` 开头改用 `ci_quick_admit` | ⑥ | 红 |
| 去掉 `CI_E2E_STAGE=quick` 导出 | ⑫ | 红 |
| `ci-local.sh` 删掉 `unset CI_E2E_*` | ⑬ | 红 |
| 子集改走整套路径 | ⑩ | 红 |
| 截断按退出码记通过 | ⑧ | 红 |
| 选择规则去掉 fixture 判断 | ② | 红 |
| `e2e-result.mjs` 的 `enforced` 改 `low` | ⑬ | 仍绿，等价变异（见审计文件） |

## 其它检查
- 真实 `sh scripts/ci-quick.sh --plan --since 5f448659`：输出 `e2e 不跑 本轮改动不涉及浏览器用例的输入`。
- 变异脚本 `.autome/output/m02-mut.sh`，不进仓库。
