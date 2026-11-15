# M-12 审计复验（审计 #14）

结论：通过。

## 自己跑的
- `cargo build -p automed -p autome-cli`、`npm run build --prefix apps/web`：成功。
- `npx vitest run test/eventbus.test.ts test/live-client.test.ts`：2 文件 31/31（eventbus 23、live-client 8），0 失败。
- `! grep -rqE "liveSeq|live-seq" apps/web/app apps/web/e2e apps/web/test`：退出 0，无命中。
- `npx playwright test e2e/live.spec.ts`：未能执行，globalSetup 的 `prepare()` 里 `bash w7-flow.sh` 60 秒 ETIMEDOUT（实现轮在另一环境报 ps EPERM）。环境限制，非测试通过也非产品失败。

## 负向对照（恢复路径变异）
- 变异：`setCoreStatus` 在内核从非 connected 回到 connected 时 `initialized = false`（跳到 latest_seq）。
- 结果：`eventbus.test.ts` 2 例失败——新单测「已在线连接重连从断开前游标续读」与既有「恢复后立即从原游标补拉离线事件」；其余 21 例仍过，说明只破坏恢复路径。还原（git checkout）后 23/23，`git status` 干净。
- 浏览器侧「前置步骤通过、目标帧断言失败」无法在本环境实跑；人工核对 `e2e/live.spec.ts` D-4：goto 前注册 `websocket.framereceived`，在线草稿先用 task.created 帧取 checkpoint（前置），离线草稿 seq > checkpoint，恢复后 poll 断言存在 seq、subject、kind 三字段都匹配的 `type:'event'` 帧；帧线格式 `{type:'event', ...event}`（server/eventbus.ts:25）与解析一致。该断言在游标被重置时必然拿不到离线期间的事件帧。
- 浏览器两例的实跑与变异移入人工验收清单 H-07。

## 判别检查
构造：上述变异对 eventbus 单测；结果：红，还原后绿。
