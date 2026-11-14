# M-13 · 实现 #22

结论：待审。仅推进 M-13；上一审计通过 M-12，没有 M-13 退回检查需要搬运。

## 实现

会话摘要仅复制并投影结构化 reason.log，复用 logName；共享 failureText 保持不变，command、summary 不截断。完整原因写入条目的 failures，并以默认收起的原生 details 展示。两种会话结束触发共用逻辑，原始事件不变。

指定验收中的 labels 守卫在改动前发现项目页硬编码「已取消」（M-11 遗留）；改为现有 STATE_LABELS.cancelled，显示不变。没有修改已完成里程碑状态或证据指针。

## 命令与结果

- 基线：`cd apps/web && npx vitest run test/task-timeline.test.ts test/labels.test.ts`，27 条，26 通过、1 失败、0 跳过。失败为 labels 的共享词汇守卫，输出 `.autome/output/m13-baseline.txt`。
- 新测试红：`cd apps/web && npx vitest run test/timeline-failure-path.test.ts`，6 条均失败，输出 `.autome/output/m13-red.txt`。
- 最终验收：`cd apps/web && npx vitest run test/timeline-failure-path.test.ts test/task-timeline.test.ts test/labels.test.ts`，3 文件、33 条全部通过，0 失败、0 跳过，输出 `.autome/output/m13-green.txt`。
- 总数相对 M-12 后、改动前同范围基线 27 增加 6，全部来自新增测试文件。未与上一轮打包测试的不同命令范围混算。
- 负向对照：分别恢复摘要直接调用 failureText、移除完整详情、原地改写 reason.log，运行新文件，均退出 1；每次还原源码，最终再次运行指定验收通过。输出 `.autome/output/m13-negative-{summary,details,mutation}.txt`。
- 未运行全量门禁或宿主快检；未起服务、未打开账本。UI 证据为 React 静态渲染断言，不宣称真实浏览器验收。

## 标待审之前的自审清单

1. **情形表逐行**：门禁连续失败、门禁超时、准备失败分别验证精确摘要及完整原因；空/缺失日志、纯文件名/中文空格路径、原始事件及共享函数不变共 6 条通过。准备命令含路径仍完整；summary 中与 log 相同的路径也不裁剪。渲染检查完整原因位于默认折叠的 details 中。
2. **失败分支**：设计点名的连续失败、超时、准备失败各有独立用例，均通过。授权、读盘、外部服务不可达不适用，本次为事件展示纯投影，无新增 I/O。
3. **全部迁移**：每种测试输入均分别运行 session_ended 与 bug_session_ended → failed；现有 task-timeline 测试覆盖运行、结束、停止和其它状态摘要，全部通过。没有新增状态机。
4. **域边界**：缺失 log、空串、单个文件名、含中文空格的目录和文件名均表驱动检查通过；原事件深比较与冻结 reason 检查通过。数字日期边界不适用，未改时间及数字处理。
