# M-03 · 实现轮 3/65

结论：实现与自动化子集通过，浏览器验收被环境阻塞；M-03 保持「开放」，不标待审。上一轮审计只复验 M-02 并通过，没有需要原样搬入的退回检查，本轮不适用审计缺陷红绿复现。

## 实现

- 左侧黄色待处理、青色进行中卡片与排队卡；结论句、两张虚线空状态卡；右侧四个今日数字块与最近八条合并。
- 服务端逐页增量消费全局账本，保留昨天以来的小记录和开放等待起点；序号回退重建，并发 loader 串行消费。今天取服务端时区的日界，涵盖 DST；取消任务和旧终结任务的当日活动照常统计。
- 共用等待口径明确包含四种人工等待与 failed，排除 paused/stopped；门禁只数 gate/fix_gate/merge_gate 的 gate.result。开放时长由 ClockProvider 每 30 秒现算，跨日重新验证，客户端保留服务端时区。
- task.get 只投影卡片需要的轮次、当前会话和待表态数，不把历史会话、文档正文带给浏览器。设计文档使用内核真实文件名构造受控 `?doc=` 链接（内核没有名为 `design` 的虚拟文档）；合并卡直达 `?tab=changes`。
- 读盘失败、超时和服务不可达明确显示缺失；部分项目不可读时，不把不完整任务集合当成全局零。
- 复用现有 w5、w7 与 w9 真实内核夹具，没有另建状态脚本。新用例的清理只对自己启动的 core 和任务操作，不依赖沙箱禁止的全机 `ps`。

## 验收命令与结果

1. 设计指定 Vitest：
   `(cd apps/web && npx vitest run test/overview.test.ts test/overview-summary.test.ts test/overview-states-real.test.ts test/clock.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts)`
   首次完整运行 71 个用例中 70 通过；真实运行夹具默认有多个并行槽，新增任务立即运行，修正夹具为项目并行 1 后，71/71 通过。清理曾遇到 `ps EPERM`，改成明确停止本夹具任务和 core 后通过。之后补充部分项目不可读场景，最终同范围 **72/72** 通过。
2. 与本轮直接相关子集：`test/shell-data.test.ts test/styles.test.ts test/live-routes.test.ts test/read-gate.test.ts`，**58/58** 通过。最终与第 1 项合并运行，**11 文件、130 用例全部通过，0 跳过**。日志 `.autome/output/m03-final-tests.txt`。
3. 指定浏览器：`(cd apps/web && npx playwright test e2e/layout.spec.ts -g "1512×944 总览$")`，**计划 1 条，实际执行 0 条**。全局 setup 在 `process-scan.ts` 调用 `ps` 时获 `EPERM`；清理同样被拒。未进入页面断言，未取得一屏或截图证据。原始日志 `.autome/output/m03-acceptance2.txt`。未修改浏览器夹具来绕过进程检查。
4. `npm run typecheck --prefix apps/web` 通过；`git diff --check` 通过。快检结果见下方补记。

### 用例总数与基线差

- 同范围起点基线：验收原有五个文件 **42** 条；本轮 **72** 条，**+30**，来源为 overview +10、clock +2、overview-summary 新增 14、overview-states-real 新增 4。相关四文件仍为 58 条，因此本轮最终子集总数 **130 对基线 100，+30**。
- 上轮 M-02 报告为 114 条 Vitest +3 条浏览器；本轮验收范围为 72 条 Vitest +1 条浏览器，名义变化 -42 / -2 是里程碑命令范围变化，并非删用例。本轮浏览器 1 条尚未执行，不能算通过。
- 初次读取/测试原始日志仅在 `.autome/output/`；证据不转存原始输出。

## 标待审之前的四项自审（已执行；浏览器阻塞使本轮不标待审）

1. **情形表逐行**
   - B-1：空闲、仅等待、运行与排队、等待加运行排队四组合；零项省略，驱动推进/空闲文案，overview #8 通过。
   - B-2：批准设计、批准合并、补充信息、确认不是 Bug、失败、暂停和停止胶囊；摘要、待表态有/无、事实条、已等时间和目的地，overview #10–11、#17 与真实 failed/awaiting 通过。
   - B-3：实现轮次、里程碑百分比、当前里程碑、本轮时长、累计用时与成本；依赖等待与槽位排队（含依赖失败/取消），overview #12–13 及真实 running 通过。
   - B-4：空闲两张虚线卡、四数字块与最近合并区域保留，overview #14 与真实 idle 通过。
   - B-Q1：按日交集、门禁白名单、取消/旧终结活动、无计费、开放等待增长、今天 done、最近八条倒序、单/多项目底部链接、分页与回退；overview-summary #1–9、#11–12 全部通过。结果胶囊六条优先级在 overview #15 通过。
   - B-Q3：时间格式和 UTC/上海/纽约时区、闰日跨年、23/25 小时日、空输入等检查通过。浏览器一屏检查受 EPERM 阻塞，未宣称视觉通过。
2. **失败分支**
   - 读盘失败、超时、外部服务不可达：overview-summary #13–14 验证缺失提示和恢复重试；overview #17 验证卡片失败不编造轮数；overview #4 验证离线双区域。
   - 空输入：空任务、空事件、空时间、空/保留字符 ID 的测试通过。
   - 拒绝授权：本里程碑没有新增写操作，不适用；现有 read-gate 相关子集全过，全局 events.since 仍只从服务端 core 调用，未放开浏览器白名单。
3. **全部迁移**
   - overview-summary #10 遍历每个等待状态进入、不变、到其它等待、到 paused/stopped/done/cancelled/implement/未知 await 状态；同状态不重置起点，退出闭合，全部通过。
   - not_a_bug 经 reply 离开单独验证；跨日切片和 now 超过 dayEnd 触发重新验证的纯函数通过；账本序号回退与并发消费通过。
4. **输入域边界**
   - 零/单里程碑、零/单合并任务、超过八条、0 元已计费/无计费、空时间/非有限时长、前导零与保留字符 ID、日界毫秒、闰日、跨年、DST、1000/1001 分页边界均有断言并通过。
   - 没有新增用户数值输入或可写配置，输入最大值校验不适用；进度条继续复用已有夹到 0–100 的共用组件。

## 守卫改动

- `testids` 内部滚动区域白名单仅追加 `overview-column-body`、`overview-merged-list`，仍检查精确集合，不放宽任意区域。
- `shell-data` 字段白名单追加统计/队列所需的七项，仍检查精确键集合与私有 request 不外泄；metrics 只投影两项结果指标。
- 旧空状态断言替换为设计指定文案；保留所有状态、链接编码和阶段去重检查。颜色、字体、图标、中文文案守卫未放松。

## 剩余工作

在允许浏览器夹具执行 `ps` 的宿主环境重跑指定浏览器命令，取得 1512×944 一屏证据后再报待审。当前状态保持开放，reopen 不增加（这是环境阻塞，不是审计退回）。

## 快检与最终静态检查

- 已按宿主要求运行 `sh scripts/ci-quick.sh --since 007ce513952e0bbab9959815c2ac2310bbb98adf`。结果：执行 87 步、失败 1 步；唯一失败为浏览器夹具/清理的 `ps EPERM`，其它超出 120 秒快检预算的步骤由脚本明确记为未跑。没有运行 `scripts/ci-local.sh`。日志 `.autome/output/m03-quick.txt`。
- 最终代码补齐项目读取失败分支后，再跑 `npm run lint --prefix apps/web` 与 `npm run typecheck --prefix apps/web` 均通过；最终 130 用例日志已经包含这次改动。日志 `.autome/output/m03-final-static.txt`。
- 本轮首次失败夹具残留的已知 T-3 已通过该临时账本的 `autome stop T-3` 停止；后续真实内核四状态测试正常停止自有任务/core 并删除夹具。浏览器 setup 的进程扫描失败记录保留，未擅自全机杀进程或删除其它会话资源。
