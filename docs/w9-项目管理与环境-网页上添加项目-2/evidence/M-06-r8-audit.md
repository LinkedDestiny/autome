# M-06 · 审计 #7 独立复验

## 验收命令
`cd apps/web && npx vitest run`（契约列出的 10 个文件）：10 文件、106 例全过，skipped 0，与预期完全一致。本环境允许监听，w9-http 的 9 例（含 env 两条）实际执行并通过，实现轮的 EPERM 阻塞不再存在。

## 负向对照（均已还原，`git status` 干净）
对 env-ops / env-page / env-real / write-gate 四文件（64 例）逐一注入：
| 注入 | 结果 |
|---|---|
| 仅处理函数改用 `params.command` | 无失败（write-gate 先拒额外键，纵深防御，属契约措辞偏宽，非缺陷） |
| write-gate 不再拒额外键（单独） | 失败 3：env-ops 2、env-real 2、write-gate 新增条 |
| 处理函数用 body command + gate 放行额外键 | 失败 3，同上 |
| 重新检测不带 `force` | 失败 1：env-page 3 |
| `startServer` 默认 `createMacTerminal()` | 失败 1：env-ops 5 |
| `onTick` 每次都发布 `env.changed` | 失败 1：env-ops 7 |
| 首份报告只记基线 | 失败 2：env-ops 7、env-real 3 |

## 自造检查
读 `admin.ts`/`write-gate.ts` 核对：prerequisite 不满足 409、`login_command` 为 null 400、终端不可用回 200 `launched:false`，均有对应用例覆盖，并被上述注入间接验证。未发现产品行为偏差。

## 人工项
H-01..H-04 已在设计文档「人工验收清单」，由用户合并前确认，不影响本里程碑关闭。

结论：通过。
