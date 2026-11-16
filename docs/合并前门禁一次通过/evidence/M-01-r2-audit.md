# M-01 审计复验（实现轮 r2 之后，审计 #1）

## 负向对照（均在 gate-reporter.ts 上临时注入，已用备份还原，git status 干净）
| 注入 | 结果 |
|---|---|
| 余量改按生效上限（config.globalTimeout）比较 | 红：「截短上限仍按声明值比较，截断不比较」 |
| onEnd 返回 result.status，不回写 failed | 红 3：gate-reporter-run「快检过余量线改写真实退出码」等 |
| 余量不足不看 CI_E2E_STAGE 一律 enforced | 红 2：run「合并前门禁过余量线退出零…」、unit「合并前门禁只报告且 digest 含警告」 |
| 过半阈值改成「超过预算」 | 红 4：字面量断言、用例、harness 计时、真实 poll 1.5s/2s |
| 删去 onEnd 的 reapRegistered | 红 3：run「预算截断后回收登记的服务及临时根」等 |

## 验收命令
- vitest 四个文件：36 个通过（gate-reporter 14、run 5、ci-config 13、test-groups 4）。契约写 34（gate-reporter 12）；差 +2 来自实现多写的 gate-reporter 用例（清理扫描失败、截断等待退出回调），多于契约，不是缺项。
- `npx playwright test --list`：`Total: 29 tests in 5 files`。
- ★ build + cargo build 通过；`npm run e2e`（未设 CI_E2E_STAGE）：29 通过，退出 0，输出含「总耗时 279.9 秒，上限 720 秒（余量 61.1%）」，X < 576 故无余量不足行；过半列表 5 条。
- report.json：status passed、complete true、timings 含两条 `prepare()`（37.9 s、25.6 s）及 globalTeardown。
- 运行后无残留 automed / vite / headless 进程，本次的两个 autome-e2e 临时根已清除（临时目录里旧日期的 autome-e2e 目录属前几轮失败遗留）。

## 新造检查
即上表五项缺陷注入，结果如上，已全部还原；无需搬进项目测试。

## 结论
通过。实现轮 r2 因 ps EPERM 未能取到的整套浏览器证据，本轮在审计环境取得，无人工验收项。
