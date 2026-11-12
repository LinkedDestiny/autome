# M-04 r5 审计复验

## 负向对照（均先注入、跑 shell-data/overview/shell-live/load-context、再还原）
- 计数改为直接按 `state==='awaiting_user'` 统计、不经 columnOf：4 条红（shell-data 2、3；shell-live 1、2）。
- 根 shouldRevalidate 去掉 `'task.'`：2 条红（load-context 根重验用例；shell-live 2 匹配器未命中）。
- `taskHref` 不做 encodeURIComponent：2 条红（overview 2、5）。
- 三处还原后 `git status` 干净。

## 验收
- `npm test -- test/shell-data.test.ts test/overview.test.ts test/shell-live.test.ts test/server.test.ts test/live-routes.test.ts`（先 cargo build）：5 文件 30/30，含真实内核与监听用例。实测 shell-live 含 setup 约 16.7s，单次 loadShell 4ms。
- 全量：Web 42 文件 357/357；lint、typecheck、build、desktop 测试退出码均 0。
- 判据：无产品行为偏差；监听受阻问题（实现轮 EPERM）在本环境不存在，已实跑。
