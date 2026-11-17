# M-13 · 实现轮 20/65

## 修改

- `toOverviewDetail` 仅从内核任务文档清单选取 `<slug>-review.md` 与 `<slug>-adjudication.md`，不推测文件存在、不传磁盘路径。
- 批准设计卡片分别显示「查看评审」「查看裁决」；缺少哪份就显示哪份「结果摘要不可得」。文档链接进入现有 `tab=docs` 加载器，继续校验清单、拒绝路径并显示读取错误。
- 复用可换行的卡片元信息布局和蓝色胶囊；原批准按钮、轮次数和其它状态卡片保持原逻辑，不引入问题数或采纳数。
- 上一审计 M-12 已通过，没有退回检查需原样搬入。本轮新增 6 个回归用例。

## 验收与用例基线

- 契约命令：`(cd apps/web && npx vitest run test/overview.test.ts test/overview-summary.test.ts test/overview-states-real.test.ts test/labels.test.ts)`。
- 修改前同范围 48/48；修改后 54/54，0 失败、0 跳过，增加 6：四种文档存在组合各 1、状态及清单变化与失败回退 1、受控详情读取与拒绝边界 1。日志 `.autome/output/m13-baseline.txt`、`.autome/output/m13-accept.txt`。
- M-03 关闭时这四文件为 45（overview 17、summary 14、real 4、labels 10），本轮为 54，差 +9：之前轮次增加 overview 2、labels 1，本轮 overview 再增 6。M-03 报告的 72 是七文件范围，不能直接与本轮四文件比较。上一轮 M-12 验证技能/上下文，与本轮范围不同；本轮没有删减已有测试。

## 标待审前的四项自审

1. **情形表逐行**：两份均有→两条受控链接；仅评审→评审链接与裁决缺失提示；仅裁决→裁决链接与评审缺失提示；均无→两条缺失提示；四个参数化用例逐项通过。其它卡片没有新增入口或缺失文案；没有问题数/采纳数；已有批准按钮断言继续通过。
2. **失败分支**：任务详情读盘失败、超时、外部服务不可达均保留错误并显示摘要不可得；受控读取的读盘失败透传；文件从清单移除后拒绝访问；绝对路径、父路径被拒绝。空/缺失清单和无详情均回退。新功能没有独立授权或外部请求，复用文档清单授权。
3. **全部迁移**：清单无→有→无重新计算入口；批准等待与合并等待、实现、失败、排队、暂停、停止逐一渲染，只有批准等待有入口。此功能无持久化状态机或写通道；已有 summary 的所有等待状态×事件迁移用例通过。
4. **输入域边界**：零份、一份、两份文档；undefined 清单、无详情；中文、空格、`&` 文件名编码后可读取；空文档名、绝对路径、`../` 均不能触发读盘；私有磁盘路径不进入卡片 HTML。数值/日期域不适用于新增文档入口；原有零值、闰日、跨年和 DST 用例继续通过。

## 负向对照与环境

- 临时删除 `OverviewCard` 的 `view.reviewDocuments` 整段入口/提示 JSX，原样运行 `(cd apps/web && npx vitest run test/overview.test.ts)`：25 个用例，5 失败、20 通过。四种文档组合及状态/失败回退用例明确变红；日志 `.autome/output/m13-negative.txt`。随后恢复组件，未保留缺陷注入。
- 首次快检发现新增测试的可空 `document` 类型错误，已改为可选访问；没有修改断言语义。
- 首次快检浏览器准备与清理均报 `spawnSync ps EPERM`，浏览器用例未执行。不能据此声称视觉验收通过；本轮没有取得新卡片的浏览器截图。既有样式已检查：胶囊与元信息容器复用，容器有 `flex-wrap: wrap`。
- 恢复后的全部直接引用测试：`(cd apps/web && npx vitest related --run app/components/overview-card.tsx app/lib/overview-labels.ts app/lib/overview-model.ts server/overview.ts)`，24 文件、203 个用例，202 通过、1 失败、0 跳过；唯一失败为既有 `shell-live.test.ts > 3 listening development SSR contains the real project and needs-you count`，报 `listen EPERM: operation not permitted 127.0.0.1`。这是端口权限阻塞，不是断言失败；本轮新增及恢复后的 25 个 overview 用例全绿。日志 `.autome/output/m13-related.txt`。
- 同引用范围用例数 203，较本轮起点结构基线 197 增加 6（仅 `overview.test.ts` 新增六条，其它测试未改；197 为结构对照，未额外跑修改前整套 related）。203 与契约 54 有交集，不相加充当独立用例总数。

## 最终快检与交付

- 已原样运行 `sh scripts/ci-quick.sh --since 29793693094f1f1003c737a0075c474ef8082608`；最终日志 `.autome/output/m13-quick-final.txt`。32 步中只有 Web e2e 失败：准备/清理 `spawnSync ps EPERM`，浏览器没有执行用例。Format、Clippy、Web lint、Web typecheck、Web build、Build core 和准入的 Vitest 子集均通过，Vitest 实跑 182/182。
- 快检因 120 秒准入预算未跑 settings-real、task-new-real、load-context、shell-live 四文件；它们已由上面的 related 命令执行，其中只有 shell-live 的监听用例被环境阻塞。快检动态准入范围与上轮不同，182 不用于同范围增量；稳定契约差值为 54−48=6。
- 未运行全量门禁 `scripts/ci-local.sh`。`git diff --check` 通过。状态块保持 `status: 实现中`，`implementation-round: 20/65`，M-13 标 `待审`，交审计在允许 `ps` 和本地监听的环境复验阻塞项；没有标已完成，也没有启动下一会话。
