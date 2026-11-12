# M-01 r1 审计复验

结论：通过。

## 独立复验
- 验收命令（本机可监听）：cargo build 通过；指定三文件 57/57 通过（write-gate 43、gateway 12、gateway-task-card 2，含真实内核 HTTP）；lint、typecheck 通过；全量 `npm test` 34 文件 312/312 通过，312 = 基线 301 + 11。
- 实现轮因沙箱监听 EPERM 未取证的 HTTP 部分，本轮已在可监听环境取得证据。

## 负向对照（均已还原，git status 干净）
| 注入 | 结果 |
|---|---|
| brief 用 `value.length` 代替 UTF-8 字节 | 1 失败：brief 字节边界用例 |
| 去掉 task.create 的 512KB 载荷例外 | 5 失败，含 A-01 真实内核 60KB 用例 |
| 写白名单去掉 task.reply | 4 失败，含 A-02 真实内核用例 |
| 写白名单去掉 task.start | 2 失败（write-gate、gateway 转发） |
「网关丢掉 brief」一条：网关不按字段挑选参数，原样转发，由 gateway.test 转发用例覆盖，未单独注入。

## 措辞
409 只表示越过门禁、进入内核分支；正文逐字保存归 M-03，成功使用归 M-04。无人工项。
