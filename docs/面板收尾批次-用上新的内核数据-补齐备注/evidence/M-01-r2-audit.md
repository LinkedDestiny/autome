# M-01 审计复验（实现轮 r2）

## 验收命令（自己跑）
- `cargo build -p automed -p autome-cli` 通过；vitest 7 个文件 **59 个用例全过**（与契约预期 59 一致；task-detail-real 6 个在真实内核上跑过）。
- `test ! -e apps/web/server/task-events.ts` 成立；`grep createTaskEvents` 无命中。

## 负向对照（注入后均还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| `changesView` 改回按「只删不增」计数 | task-changes「deleted_count 不把六个只删行…」失败 |
| `readTaskEvents` 不带 `subject` | task-history 2 个失败（每次调用带 subject、游标推进） |
| 终止条件改为「游标 ≥ latest_seq」 | 测试 worker 因无限分页 SIGABRT，用例无法通过（被拦住） |
| 时间线忽略 `session.ended` | task-timeline 2 个失败（四种结束方式、事件优先） |

## 说明
- 第一次把终止条件写成「短页 或 游标≥latest_seq」未被拦，那是等价于当前实现的弱变异，不是契约里的错误实现；契约原话的变异已被拦住。
- 操作者备注：简报无此节。
- 人工项：无；沙箱未阻断本轮命令。
- 全量门禁由宿主合并前运行，本轮未跑。
