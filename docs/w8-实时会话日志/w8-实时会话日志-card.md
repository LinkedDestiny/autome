# W8 实时会话日志：服务端按订阅跟随日志文件、经 WebSocket 推增量，任务时间线里可展开查看

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6「W8 实时会话日志」。会话日志在 `<repo>/.autome/output/sessions/<task>/<ses>.log`（渲染过的可读文本）与 `.jsonl`（原始流）；内核 `session.log { session_id }` 返回日志路径与是否在跑，命令行 `autome logs <任务> [--session id] [-f]` 已经会跟随。W3（T-14）的事件总线 `apps/web/server/eventbus*.ts` 定义了 §5.2 的 WebSocket 协议，W8 在同一条连接上加日志订阅。

## 要做什么

1. 服务端 `apps/web/server/logtail*`：客户端发 `{ type: "log.subscribe", session_id }` / `log.unsubscribe`，服务端用内核 `session.log` 拿到路径（**只接受内核返回的路径**，不接受客户端给的路径），从文件末尾（或指定偏移）开始跟随，按增量推 `{ type: "log.chunk", session_id, offset, text }`；会话结束后推最后一段并 `log.end`，自动停止跟随。
2. 限制：每个连接最多同时订阅 4 个日志；单条消息不超过 64KB（大段按 64KB 切）；文件被轮转或删除时推 `log.end { reason }`。
3. 前端 `apps/web/app/components/session-log*`：任务时间线（W7）每一轮可展开查看该会话的日志——正在运行的实时滚动，历史会话一次加载；可以暂停自动滚动、跳到最新；大日志只保留最近 N KB 在 DOM 里。
4. 协议补进 `apps/web/server/eventbus-protocol.ts` 并有测试；§5.2 的断线续传语义对日志订阅同样成立（重连带 offset）。

## 不做什么

- 不解析 `.jsonl`，只推 `.log` 文本。
- 不做搜索、下载；不做多任务合并视图。
- 不改 `crates/`；不改 `apps/desktop/`。W7 的时间线本身不在这里做，W8 只提供组件与服务端能力，先挂在一个最小的任务页占位上即可。

## 验收标准

1. 日志路径只能来自内核 `session.log`：客户端传路径的请求被拒绝，有测试。
2. 第 5 个订阅被拒绝并带中文原因；>64KB 的增量被切块且顺序正确，有测试。
3. 集成测试：用假 CLI 起一个会话，浏览器端在会话写日志后一个 tick 周期内收到增量；会话结束后收到 `log.end`，服务端不再持有文件句柄。
4. 前端：暂停滚动时新内容不抢焦点，「跳到最新」恢复；历史会话展开能看到完整内容。
5. `npm --prefix apps/web test`、lint、typecheck、build 全绿；`sh scripts/ci-local.sh` 全绿。

## 参考

- 计划 §5.2（WebSocket 协议）、§5.3（面板库与文件）、§5.5（本机安全）。
- `apps/web/server/eventbus-ws.ts`、`eventbus-protocol.ts`、`apps/web/app/lib/live*`。
- `crates/autome-cli/src/main.rs` 里 `logs -f` 的跟随实现（文件增量读法可以照搬）；`crates/automed/src/dispatch.rs` 的 `session.log`。
