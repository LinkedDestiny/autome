# 审计 #6：M-06 独立复验

- 验收命令 `live-client.test.ts bell-client.test.ts`：2 文件 15 例全过，无跳过。
- 负向对照一：onMessage 只放 default 分支 → bell-client 4 例失败（出口、订阅、贯通、未知类型）。已还原。
- 负向对照二：publishLiveMessage 去掉 try/catch → 订阅异常用例失败。已还原。
- 全量四件在本轮环境可监听：web 33 文件 301 例（比 M-05 基线 300 多 1，来自 M-06 新增的未知类型出口用例）、lint、typecheck、desktop 全过。
- 源码核对：内置四类处理不变，onMessage 在 switch 之后统一调用，坏消息/旧 socket/stop 后不交出。
- 结论：通过。新造检查为一次性注入，已丢弃。
