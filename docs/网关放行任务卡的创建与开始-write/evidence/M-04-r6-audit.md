# M-04 审计 #4 独立复验（对应实现轮 r6）

未读实现轮证据，自行复验。

## 负向对照（先做，各自只跑 gateway-task-lifecycle.test.ts，之后 cp 还原）
| 注入 | 结果 |
|---|---|
| write-gate 白名单删掉 `task.start` | 2 个用例红：`expected 400 to be 200` |
| `task.reply` 对 `info` 加 `trim()===''` 拒绝 | 用例 2 红：`expected 400 to be 409` |
| gateway.ts 对 task.reply 丢掉 `info` | 用例 2 红：内核回「缺少参数 `info`」，不含「补充信息不能为空」 |
| 删掉两行 `AUTOMED_*_BINARY = fake` 赋值 | 2 个用例红，守卫处 `expected undefined to be <替身路径>`，启动服务前即失败，未拉起真实 CLI |

四项全部还原，`git status` 干净。

## 验收命令
- `cargo build -p automed`：通过。
- `npx vitest run test/gateway-task-lifecycle.test.ts test/gateway-task-card.test.ts`：2 文件 6 用例全过。
- lint、typecheck 通过。
- 全量 `npm test`：321 用例（320 过 / 1 败），321 = 基线 301 + 20，与预期 B+20 相符。
- 唯一失败 `notifier-live.test.ts`「停服期间新增失败，重启只补一条且保留旧已读」30s 超时。单跑两次：第一次同样失败、第二次 3/3 通过 → 不稳定。`git diff <merge-base> -- apps crates` 只含 read-gate/write-gate 与 5 个测试文件，未触及 notifier-live 及其被测代码；宿主门禁（64047b4）通过。判为既有不稳定，与 M-04 产品行为无关，记 Backlog B4-01。
- 替身 `history` 非空且逐行等于替身路径（用例断言），会话只落到替身 CLI。
