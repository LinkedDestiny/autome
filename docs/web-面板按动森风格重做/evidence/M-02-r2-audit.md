# M-02 r2 审计证据

## 负向对照（逐条注入、跑验收 vitest、还原）
- `selectedDriver` 的过滤改成 `d.self`：shell「两个驱动只显示在推进的那个…」等 2 个失败。
- `publishDrivers` 去掉快照去重：eventbus「驱动摘要变化时发一条」「不变时不发」2 个失败。
- `/loop` 的 `byKind` 去掉 `config.`：live-routes「Loop 和技能收到 config.changed…」1 个失败。
- 三处均已还原，`git status` 干净。

## 验收命令
- vitest 9 个文件，114 个用例全过（契约下限 107）。
- playwright `layout.spec.ts -g 总览`：3 条通过（本沙箱可跑，实现轮因 EPERM 未能验证的浏览器三例已补验）。

## 新造检查
- 上述三处缺陷注入即判别检查，通过后丢弃。

## 结论
通过。
