# M-09 实现轮 #16

## 范围与修复

按 E-Q4 在每次汇总返回时建立当前任务映射；开放等待及 waiting 只纳入当前仍等待的任务，闭合区间记录任务身份，并按当前列表过滤 closedMs / ended。缓存不因移除而删除，重新加入恢复统计。零等待显示「0 分 · 今天没有等待」，仅开放等待使用「正在等你，今天暂无结束的等待」，已有结束项沿用分类次数。

上一轮审计 M-08 通过，没有退回检查需搬运。本轮基线发现 labels 既有守卫因 task-stage.tsx 的「已完成」字面量失败；仅将其替换为已导入的 STATE_LABELS.done，显示不变，未推进另一里程碑。原检查已在项目测试体系，基线红、修复后绿。

## 命令与结果

- 基线及正式验收：`(cd apps/web && npx vitest run test/overview-summary.test.ts test/overview.test.ts test/labels.test.ts)`。基线 50 用例（49 通过、1 失败），修复后 55 用例全部通过，0 跳过；比上一轮代码同口径基线增加 5：summary 新增 3、overview 新增 2。四条既有 summary 用例补齐当前任务夹具，原时间交集、计时增长与真实事件断言保留。原始输出 `.autome/output/m09-baseline.txt`、`m09-accept.txt`。
- 直接相关回归：`(cd apps/web && npx vitest run test/task-stage.test.ts)`，17 用例通过，较上一轮该文件 17 用例差 0。两组共 72 个不同用例，基线 67，净增 5。
- 负向对照：临时移除开放与闭合过滤，summary 17 用例中新增 3 条失败；临时让零值直接走 formatSpan，overview 27 用例中新增零值条失败。分别保留 `.autome/output/m09-negative-filter.txt`、`m09-negative-zero.txt`，源文件均已恢复。
- 未运行全量门禁、浏览器或人工 H-07；这些不作为本轮通过证据。

## 标待审之前的自审清单

1. **情形表逐行**：E-Q4 开放区间过滤、closedMs/ended 过滤、waiting 同口径、移除再加入均由新增 summary 三条通过；五种等待状态逐一覆盖。零值、仅开放、仅闭合、开放与闭合并存分别由新增 SSR 两条覆盖；缺失数据仍显示暂不可用。花费与门禁原断言保留并通过。
2. **失败分支**：原有「读盘失败、超时与服务不可达呈现缺失而非零，恢复后重试」及「部分项目读盘失败…」通过；本里程碑不新增授权入口，拒绝授权不适用。空任务列表在新增过滤用例中通过。
3. **全部迁移**：原有五种等待状态 × 同状态/另一等待/离开等待的 waitingTransition 矩阵通过；新加当前列表存在→移除→重新加入、当前状态等待→非等待→等待均通过。非等待覆盖 implement、未知节点、done、cancelled、paused、stopped、queued。
4. **输入域边界**：空列表、单任务、多任务身份隔离，零毫秒与单个开放区间，跨午夜交集、闰日、跨年、DST 23/25 小时、当天起点/次日起点均由本轮验收用例通过。ID 不作数值转换；前导零、最大数值非本次输入域要求，不适用。

## 最终检查与环境限制

- 负向对照恢复后再次运行正式验收，55/55 通过，输出 `.autome/output/m09-final.txt`；`git diff --check` 通过。
- 已运行指定命令 `sh scripts/ci-quick.sh --since 22a2ace594b4d1a2fd2edff9e66bbc94ac04fde8`。脚本执行 47 步、沿用 0 步、失败 6 步，用时 144 秒；lint、typecheck 通过。失败步骤为 web-status-driver-real（spawnSync ps EPERM）、board-live-listen / gateway-http / gateway-task-card / gateway-task-lifecycle（listen EPERM 127.0.0.1）、Web e2e（清理时 spawnSync ps EPERM）。原始输出 `.autome/output/m09-quick.txt`。该脚本自动选择的环境检查未通过，不宣称快检成功；交宿主复跑。不另跑全量门禁。
