# M-09 r16 审计复验

- 验收命令：3 文件 55 用例通过（summary 17、overview 27、labels 11），与契约下限一致。
- 负向对照 1：把 currentOpen/currentClosed 的 tasks 过滤去掉 → overview-summary 3 条失败（移除任务不计、非等待状态不计、闭合区间按任务过滤）；已还原。
- 负向对照 2：数字分支去掉 waitMs===0 → formatSpan(0) → overview「等待为零显示 0 分」失败；已还原。
- 代码核对：waitingKind 只认 failed 与等待节点；currentOpen 同时要求任务在列表且当前等待；ended 与 closedMs 只算列表内任务。
- 还原后复跑 55 通过，git status 无源码改动。
- 快检的 ps/端口 EPERM 属环境问题，宿主快检已在 883dda3 通过。
