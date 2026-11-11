# M-03 审计复验（第 3 轮审计，对应实现 r4）

本轮能监听 127.0.0.1，未被 EPERM 阻断，live 验收直接取证。

## 负向对照（先做，改完用 git checkout 还原，`git status` 干净）
- `onTick` 改成 `void bus.pull(); void notifier.pull();`：`notifier-api` 1 失败 / 6 过（attachNotifier 顺序用例）。
- 单条/全部已读不再 `publish`：`notifier-api` 的「单条已读先落库再广播」「全部已读写库并广播实际变化」与 `notifier-live` 第 2 个用例共 3 个失败。
- `POST /api/notifications/read` 注册在 `securityMiddleware` 之前：`notifier-live` 第 2 个用例失败，`expected 200 to be 403`。
结论：验收命令能拦住三种错误实现。

## 复验
- `notifier-api.test.ts` 7/7 退出 0；`notifier-live.test.ts` 3/3 退出 0；`notifier-live-fixture.test.ts` 1/1 退出 0。
- 全量四件：web 278 过 0 跳过；lint、typecheck 退出 0；desktop 103 过 0 失败 1 跳过（基线自带）。
- 日志在 `.autome/output/a3-*.log`。

## 结论
通过。无实现缺陷，无验证缺口，无人工验收项。
