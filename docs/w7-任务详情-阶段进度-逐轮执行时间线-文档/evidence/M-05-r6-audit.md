# M-05 审计复验（实现轮 6）

1. 验收命令：6 文件、110 用例全过（129s）。日志 `.autome/output/m05-audit.txt`。
2. 负向对照（均临时注入后还原，git status 干净）：
   - stop 规则加 `|| state==='active'`：task-ops 7 条失败；task-ops-real 仍 16 过 → 契约该句不实。
   - `RERUN_NODES.feature` 去掉 rebase：task-ops 2 条失败（角色过滤、Rust 源码核对）。
   - 写操作失败返回 `'操作失败'`：task-action 2 条失败。
3. 自造判别检查：复制 task-ops-real，每个状态再调一个表判定可用的操作（非 cancel），必须被接受。未注入时 16 过；stop 放宽时 Bug 矩阵在 `matrix-info/stop` 失败（内核「任务不在运行中」）。产品行为与内核一致。
4. 版本兼容：main 构建内核（38s）下 task-ops-real 16 过；task-merge-real 尚未存在。
5. 结论：通过；验证缺口记 B-08，不退回。提交若被拒由 Autome 兜底。
