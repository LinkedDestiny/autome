# M-13 实现轮 #23

结论：验收通过，交独立审计。implementation-round：23/65。上一轮审计关闭 M-12，没有本里程碑的退回检查需要搬迁。

## 实现

- 从 task-detail-payload.test.ts 抽出共享夹具 test/fixtures/task-detail-60.ts；两个测试使用同一构造。会话 label 改为含序号的中文标签，以明确断言最新会话出现，而不是匹配所有会话共有的“实现”。60 会话、30 里程碑、63 文档、每会话 8 事件保持不变。
- 新增 exclusive 组 task-detail-ssr.test.ts，沿用 ensureBuilt 与 createLoadContext，以生产 createRequestHandler 直接执行请求，不监听端口、不启动真实内核。
- 完整读取 arrayBuffer，检查未压缩字节 ≤ 153600；同时检查 HTTP 200 和最新会话标签。数据请求实际路径为 /tasks/T-1.data。
- 无生产代码变更，不需下调 20 会话窗口。

## 命令与结果

基线：`(cd apps/web && npx vitest run test/task-detail-payload.test.ts test/test-groups.test.ts)`，8 例通过（4 + 4），日志 `.autome/output/m13-baseline.txt`。

验收：`npm run build --prefix apps/web && (cd apps/web && npx vitest run --silent=false test/task-detail-ssr.test.ts test/task-detail-payload.test.ts test/test-groups.test.ts)`，构建成功，3 文件 11 例全部通过、失败 0、跳过 0。较同范围改前基线 **+3**，全部来自 SSR 新增三例，原 payload 4 与分组守卫 4 未删减。上一轮 M-12 验收属于不同测试集合，不混作本轮基线。日志 `.autome/output/m13-final-build.txt`、`.autome/output/m13-final.txt`。`--silent=false` 仅展开成功用例测量日志；不带该参数的契约命令也已通过，见 `m13-green.txt`。

| 场景 | 原估算口径（字节） | 生产响应实测（字节） |
|---|---:|---:|
| 首次文档，60 会话 | 45602（loader JSON + shell JSON + 静态 HTML） | 54213 |
| 同 URL 数据请求 | 24010（loader JSON） | 20684 |
| from=session-40，65 会话 | 53314（文档估算） | 61974 |

估算与真实 turbo-stream 编码不同，不能互相替代。三项都为 200、含最新标签；限额均为 150×1024 字节。

## 负向对照与恢复

1. 临时把 loader 返回对象加回 `rawTask: data, rawEvents: history.events`，重建后运行 `(cd apps/web && npx vitest run test/task-detail-ssr.test.ts)`：三例全部因超过 153600 字节失败，分别为 239596、187242、261557 字节，HTTP 均为 200。日志 `.autome/output/m13-negative-raw.txt`。随后原样恢复 loader。
2. 临时在测试替身注入 `services.gateway.read.mockRejectedValue(new Error('M-13 injected route failure'))`，重建并运行同一三例：全部因状态 500 不等于 200 失败；错误页仅 6614、156、6614 字节，证明小错误页不能误过。日志 `.autome/output/m13-negative-error.txt`。随后恢复测试。
3. 两项变异均已还原，再重建并跑全部 11 例通过；生产 loader 的 git diff 为空。首次开发时曾误猜页面标签格式导致三条断言失败，已改成夹具独有标签进行精确检查，没有放宽状态或大小断言。

## 标待审前自审清单

1. **情形表逐行**：G.4 首次文档、同 URL 数据响应、钉住后新增五会话三行均实际执行，数值见上表；共享夹具、exclusive 分组由源码与 test-groups 4 例核对。timeline=all 原例仍检查完整 60 个日志控件。
2. **失败分支**：G.4 / M-13 点名的回包膨胀与错误页两个负向分支已逐一执行并失败，恢复后全绿。拒绝授权、磁盘失败、超时、外部服务不可达不适用：本轮不增加这些产品路径，替身无外部服务。
3. **全部迁移**：本轮不新增状态机。窗口从首次 60 会话到钉住 session-40，再增加到 65 会话由 payload 既有例及 SSR 新例覆盖；文档、数据两个响应通道均测。浏览器实时状态迁移属于既有 M-05，本轮未修改。
4. **域边界**：固定夹具输入域为 60 / 65 会话、30 里程碑与窗口起点 session-40，均执行；150 KB 上限实际比较为 ≤ 153600 字节，负向大回包验证超限拒绝。全部历史分支的 60 条检查仍过。空串、闰日、前导零等不适用，本轮没有新增日期解析或用户输入域。

## 收尾

`git diff --check` 通过。只跑本里程碑验收及负向对照，未运行 ci-local、ci-quick 或全量门禁。没有起面板、内核或浏览器，没有访问真实账本，无需清理此类进程；所有构建与测试子进程均已结束。静态快检与合并前全量门禁由宿主执行。
