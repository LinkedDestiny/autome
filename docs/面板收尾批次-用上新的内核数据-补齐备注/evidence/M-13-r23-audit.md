# M-13 审计复验（实现 #23 之上）

结论：通过。

## 验收命令
`npm run build --prefix apps/web && (cd apps/web && npx vitest run test/task-detail-ssr.test.ts test/task-detail-payload.test.ts test/test-groups.test.ts)` → 退出 0，3 文件 11 用例全过（ssr 3 / payload 4 / groups 4，与契约一致）。

## 实测字节（生产构建，未压缩）
- 文档 /tasks/T-1（60 会话）：54213 B，200
- 数据 /tasks/T-1.data：20684 B，200
- 钉住 from=session-40 后 65 会话：61974 B，200
均 ≤ 150 KB，且断言正文含最新会话标签「实现会话-N」。

## 负向对照（均已还原，git status 干净，build 已重建）
1. loader 返回整份 task.get 与事件并取消 20 窗口：文档 295328 B、数据 216436 B、钉住 261553 B，三条全部因超 150 KB 失败。
2. loader 在读到 task.get 后抛错：三条均 500（7172/597/7172 B），因「200」断言失败——字节小不是因为页面渲染了。
还原后同一命令复跑 11 用例全过。

## 其他
- 宿主快检已通过（简报）。未跑全量门禁。
- 会话未起任何面板/内核/浏览器进程。
- 操作者备注：简报无该节。
