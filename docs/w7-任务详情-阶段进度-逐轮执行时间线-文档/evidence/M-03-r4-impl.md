# M-03 实现 #4

结论：已实现时间线与日志组合；环境限制使验收不全绿，M-03 保持开放，不标待审。implementation-round 为 4/30。

## 改动与事实核对

- 新增纯函数 `timelineEntries` 与 `TaskTimeline`，路由组合时间线并显示事件截断 / 读取失败提示。会话按 `session.started` 的 seq 锚定，缺失锚点时按 started_at 回填；摘要按逐会话快照差、Bug 业务结果与会话结束转换生成。
- 门禁以首事件 seq 为执行身份，排队、开始、结果更新同一条目；同提交重跑新建条目，旧结果保留；重复排队 / 开始将悬挂执行标为中断。只有 repro_check 与随后的业务复现事件关联，fix_gate 不把失败显示为符合预期。
- SessionLog 仅新增可选 defaultOpen，缺省仍折叠；时间线向运行中的会话传 true。稳定会话 key 与相同父元素保留组件状态，未改 W8 控制器或订阅协议。
- 源码核对：SessionLifecycle 的停止值是 killed；merge.rebased 使用 repos[].repo/onto/head，merge.repo 使用 commit 或 blocked。实现与测试按真实字段修正，未改内核。
- 新增测试 16 例：模型 10、真实内核时间线 3、真实日志 3。原始 JSON 使用 M-01 已抓取的 w7 夹具，合成边界在测试内明确标注。
- 上轮审计 M-02 已通过，未退回缺陷；本轮没有需要逐字搬入的审计判别检查。

## 命令与结果

1. 本里程碑原定验收命令：
   `cd apps/web && npx vitest run test/task-timeline.test.ts test/task-timeline-real.test.ts test/task-log-live.test.ts test/session-log-view.test.ts test/session-log-model.test.ts test/session-log-controller.test.ts`
   退出码 1；6 文件、37 例，33 通过、1 失败、3 未执行。原始输出 `.autome/output/r4-accept.txt`。
   - 模型 10 与 W8 原有 21 全过。
   - 真实内核需求 / Bug 的 loader → 时间线 → HTML 两例通过，实际假 CLI 推进到 await_merge。
   - 时间线收尾用例与 afterAll 在 `spawnSync ps EPERM` 失败；core.stop 已执行，但不能声称进程无残留。按 teardown 约束保留临时目录，不绕过检查。
   - 日志文件 beforeAll 先做端口探针即 `listen EPERM 127.0.0.1`，尚未启动夹具进程，3 例因 hook 失败未执行；清理也被 ps 禁令阻断。不是代码主动 skip。
2. 最终直接相关测试子集：
   `cd apps/web && npx vitest run test/task-timeline.test.ts test/session-log-view.test.ts test/session-log-model.test.ts test/session-log-controller.test.ts test/task-page.test.ts test/styles.test.ts test/labels.test.ts`
   退出码 0；7 文件、54 例全过。输出 `.autome/output/r4-final-subset.txt`。包含补充后的门禁名称矩阵、复现关联截止边界、闰日与跨年时差。
3. 仅对本轮变更的 8 个 TS/TSX 文件运行 `npx eslint ... --max-warnings 0`，最终退出码 0，输出 `.autome/output/r4-eslint.txt`。初次检查发现两处 AggregateError 缺少 cause，已修正并复验；未运行项目 lint / typecheck 门禁。
4. `git diff --check` 退出码 0。端口、ps 的独立探针分别记录 `.autome/output/r4-listen.txt`、`.autome/output/r4-ps.txt`，均为 EPERM。

用例数量对比：上一轮 M-02 验收基线 43 例；本轮 M-03 原定验收 37 例，差 -6 来自验收文件集合切换，不是删掉原有用例。M-03 的已有 W8 基线为 21，新增 16，合计 37，恰与设计预期相同。本轮额外相关守卫 23 例，合并去重覆盖 60 例；最终可运行子集 54 全过，加已实跑的真实页面 2 例，共取得 56 例通过证据，剩余 1 例进程检查失败、3 例端口受阻。没有用重复运行累计用例数。

按本轮明确指令未运行 Web 全量、项目快检或 ci-local；不以较旧任务文件里的全量要求覆盖本轮限定。完整验收命令之后仅修改日志测试的首段等待与异常 cause，以及模型测试补充断言；监听测试仍未能实跑。

## 标待审之前的自审清单（已执行，存在阻塞，故不标待审）

1. **情形表逐行**
   - 需求九次会话顺序、中文阶段 / CLI：模型第 1 例与真实需求 HTML 通过。
   - 快照变化、设计批准状态、协议失败 / 崩溃：模型第 2 例通过；第一轮实现仍是 M-01 开放 → 待审，不误读当前已完成。
   - Bug 定位 / 无法复现、复核通过，初始复现与修复回归区分：第 3 例与真实 Bug HTML 通过。
   - 门禁排队 / 开始 / 结果、同提交重跑、准备命令排队分离：第 4 例通过。
   - rebase 冲突文件、自动 rebase、合并、合并后命令、准备命令、锁事件、复用 / 跳过：第 5 例通过；真实合并事件的字段读取另按内核源码核对。
   - 金额缺省 / 零值、耗时、被停止 / 运行中：第 6 例通过。
   - 稳定键、只追加、截断时回填、空列表 / 未知事件：第 7 例通过。
   - 默认展开 / 折叠、读取失败与截断提示：第 8 例 SSR 通过；真实日志订阅因端口禁令未取证。
   - 十种用户操作：第 9 例通过；重试 / 冷却 / 守卫 / 协议失败：第 10 例通过。
2. **失败分支**
   - 门禁非零、超时、重复开始造成中断、复现不可执行 127、复现超时、复现命令通过但未复现：第 3、4 例通过。
   - 业务复现事件未到只显示退出码，下一门禁开始后不误关联：第 3、7 例通过。
   - 会话协议失败 / 崩溃显示全文：第 2 例通过。事件读取失败显示错误且保留会话：第 8 例通过。
   - 授权拒绝不适用：本里程碑没有写操作；磁盘读取不适用：复用 W8，不新增日志读盘路径。外部服务不可达不适用：无外部服务。真实日志拒绝行为由原有 W8 测试覆盖且 21 例通过。
3. **全部迁移**
   - 空 → 排队 → 运行 → 通过 / 非零失败 / 超时；无排队直接开始；缺失开始的孤立结果；排队 → 新排队、运行 → 新开始、运行 → 新排队，旧执行中断；结束 → 同提交新执行：模型第 3、4、7 例检查通过。
   - repro_check 结果 → reproduced / repro_missed 的三种 why；下一 started 截止旧复现关联；fix_gate 失败不误判：通过。
   - 会话运行 / 已结束 / killed 与新会话默认展开：模型和 SSR 通过。旧组件展开保持、真实滚动位置仍属 H-03，未声称浏览器实测通过。
4. **输入域边界**
   - 空 sessions / events、单会话、缺失 metrics、cost_usd 为 0、缺失锚点、未知事件、相同提交重复执行：通过。
   - 闰日与跨年一秒差：通过。输入是内核时间戳 / seq / 金额，不接收用户数字或日期表单；前导零、最大用户输入不适用，无此输入域声明。

## 后续与环境阻塞

下一轮首先在允许 localhost 监听与 ps 的环境原样运行 M-03 验收命令；不得删掉清理断言或改成 skip。当前不足以标待审。H-03 的真实展开与滚动、视觉截图未取证；页面最终两栏与高度约束由 M-06 推进，本轮不提前关闭它。
