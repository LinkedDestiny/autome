# M-03 r4 审计复验

## 负向对照（均实跑，验收命令 `npm test --prefix apps/web -- test/labels.test.ts test/task-columns.test.ts`）
| 注入 | 变红 |
|---|---|
| 需求「实现」段删 `gate` | labels 2、6；task-columns 6 |
| `retro` 同时写入「审计」与「合并」 | labels 2；task-columns 6 |
| `columnOf` 把 `stopped` 归 `running` | task-columns 1、4、5 |
| `AWAITING_NODES` 去掉 `not_a_bug` | task-columns 1、2 |
全部还原，`git status --porcelain` 为空。

## 复验
- 验收命令：2 文件 13 用例全过。
- lint=0，typecheck=0，build=0；Web 全量 39 文件 343 用例全过，无 skipped（静态枚举 329，运行时参数化 343 = 上一轮 330 + 13）。
- 夹具 `w5-project-get.json` 含 draft/queued/active/failed/stopped，counts 含 awaiting_user=1，与核对式一致。
- 宿主门禁（提交 6fad296）`ci-local.sh` 退出码 0。
- desktop 未改（W5 不改 desktop），未单独复跑。

## 结论
通过。无实现缺陷、无验证缺口。
