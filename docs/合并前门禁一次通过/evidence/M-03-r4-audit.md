# M-03 r4 审计复验（审计 #3）

## 验收命令（全部自己跑）
- `gate::tests::digest` 5 个通过；`prompt_rendering` 47 个通过（基线 43 + 4）；`end_to_end gate_digest` 2 个通过；`ci-summary` 10 个通过（8 + 2）；vitest gate-reporter + gate-reporter-run + ci-config 共 40 个通过（20 + 7 + 13）。
- 实现轮因 ps EPERM 未能取证的真实 Playwright 用例（gate-reporter-run 7 条）本轮在本环境实际运行，全过。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| `scheduler.rs` 不读 digest 段（`digest = None`） | `gate_digest` 两条都失败 |
| `gate.rs` 16 KiB 截断改为不对齐字符边界 | `digest_utf8_limit` 失败（panic） |
| `gate-digest.ts` 截断措辞不看结果状态（`timedout` 分支恒真） | `四种截断成因不混淆` 与 `globalSetup 无 onBegin…` 失败 |
| `gate-reporter.ts` setup 失败不回退 `listCases()` | `globalSetup 无 onBegin 时用 list 找回…` 失败 |

## 结论
通过。无实现缺陷、无验证缺口、无人工验收项。
