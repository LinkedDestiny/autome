# M-01 · 实现 #9：rebase 后共享服务接线复验

## 范围与结论

本轮简报和审计 #7 均显示六项已经关闭，没有开放里程碑。用户另外明确要求 rebase 到默认分支，故本轮执行该修复，不新增里程碑。默认分支由 origin/HEAD 确认为 main，目标为 24fde3fc。git rebase main 已完成；git merge-base --is-ancestor main HEAD 返回 0，没有 merge 提交、没有切换工作分支、没有推送。

设计「风险与验证安排」要求解决产品源码冲突后重新报验。本轮把共享接线归入最小编号 M-01 复验；其余里程碑自身业务实现未改，保留审计关闭状态。M-01 因验收环境阻塞保持开放，不伪报待审。此非审计发现的实现缺陷，reopen 保持 0。implementation-round 为 9/30。

## 冲突解决

- app/load-context.ts：并列保留 taskEvents、taskDocs、admin 的导入与可选服务字段。
- server/http.ts：开发、生产两条 context 注入链路均传入三个服务；保留 /api/paths 与既有安全中间件顺序。
- server/main.ts：保留任务事件、文档服务的创建和 taskEvents.close；同时保留路径守卫、admin.register、Onboarding/env tick 发布、默认不可用终端与仅入口注入真实终端。
- test/write-gate.test.ts：specific 同时保留 task.set_after/task.move 和 project.confirm/project.remove；两个分支的测试全部保留，没有覆盖或删减断言。
- app/styles/app.css：后续 replay 另发生追加样式冲突；保留完整 W7 task 样式和 W9 添加项目样式，两组选择器不同。
- 后续 Onboarding、环境提交再次冲突于 main.ts，依次补入 publish 和 terminal，未丢弃 task 服务。

原始 rebase 输出：.autome/output/r9-rebase.txt、r9-rebase-continue.txt、r9-rebase-continue2.txt、r9-rebase-continue3.txt、r9-rebase-continue4.txt。接线对比：.autome/output/r9-wiring-diff.txt。

## 命令与结果、用例基线

1. 原样 M-01 验收：

`cd apps/web && npx vitest run test/paths.test.ts test/project-confirm.test.ts test/project-confirm-real.test.ts test/w9-http.test.ts test/write-gate.test.ts test/read-gate.test.ts test/gateway.test.ts test/gateway-http.test.ts test/load-context.test.ts`

9 文件、总计 **130** 例：110 通过、20 失败、0 跳过。相对 M-01 上次审计基线 120 例增加 **10**：w9-http 后续里程碑增加 6、M-06 write-gate 增加 1、main 的依赖与移动操作 write-gate 增加 3。本轮没有新增或删除用例。20 例全部在启动监听时遇到 listen EPERM（w9-http 9 + gateway-http 11），不是断言证明了业务缺陷。日志：.autome/output/r9-m01.txt。

2. 接线相关扩展子集：以上 9 文件，再加 test/env-ops.test.ts test/env-page.test.ts test/env-real.test.ts test/w9-layout.test.ts test/overview.test.ts test/shell.test.ts test/labels.test.ts test/styles.test.ts test/task-events.test.ts test/task-docs.test.ts test/task-actions-http.test.ts test/task-detail-real.test.ts，使用相同 npx vitest run 命令。

21 文件、总计 **199** 例：175 通过、21 失败、3 跳过。相对上一轮 M-06 的 106 例增加 **93**：main 写门禁的 3 例，加本轮扩大到共享服务及 W7 的 90 例；这是选取范围变化，不是本轮新增测试。除上述 20 个监听失败外，task-detail-real 的清理验证因 spawnSync ps EPERM 失败；task-actions-http 的 beforeAll 因监听 EPERM 失败导致 3 例跳过。task-detail-real 其余 3 例通过。另有 Vite 文件监视 EMFILE 警告。日志：.autome/output/r9-tests.txt。没有隐藏、删除、放宽这些失败检查。

3. git diff --check 通过；main 是 HEAD 祖先。没有运行 ci-local、宿主快检或全量测试。load-context 既有用例内部执行的生产构建和开发/生产 SSR 验证通过。

## 标待审之前的自审清单（已执行，但环境项未通过，因此不标待审）

1. **情形表逐行**：M-01 无独立表格，按设计 §2、§3 与 M-01 测试清单逐项核对。目录/文件过滤、排序与显示路径通过；不存在目录与文件不可读作目录通过；..、NUL、绝对逃逸通过；符号链接逃逸与断链通过；49/50/51/60 上限通过；隐藏前缀通过；HOME 符号链接通过；最深祖先通过。confirm 同路径 probe→add、外部/链接拒绝零调用、整个 HOME 拒绝、无成员工作区拒绝、skip 成功及失败保留项目、内核错误原文、probe 读守卫均通过。真实内核的单仓库、工作区、重复注册、外部路径不改变项目及回包形状 5 例通过。HTTP 安全头与写入口 20 项因监听限制未取证。
2. **失败分支**：路径读盘失败由 paths 的 file/ 输入覆盖，通过；空输入 check 拒绝而 complete 列 HOME，通过；内核拒绝、超时 504、不可达 503 由 project-confirm 的错误序列覆盖，通过；Host/Origin 拒绝需要监听，未取证。终端不可用、前置条件不满足、额外 command 键拒绝由 env-ops 覆盖，通过。
3. **全部迁移**：未注册→probe→注册（单仓库/工作区）通过；probe 拒绝→不 add 通过；已注册→重复拒绝通过；注册→skip 成功、注册→skip 失败但保留项目通过。读 probe 有守卫/无守卫两条兼容通道通过。HTTP 注册→移除与拒绝保持状态的检查受监听阻塞。共享接线的开发/生产 SSR 均通过；env 首次报告、状态变化、重复报告由 env-ops 覆盖通过。
4. **输入域边界**：空串、非字符串、缺少 HOME、511/512/513 长度、NUL、..、反斜杠、同前缀兄弟目录、单元素、大小写、49/50/51/60 项均通过；write-gate 四键、布尔、非路径 id 和 main 新增任务编号前导零边界通过。日期/闰日/跨年不适用：本轮接线及路径接口无日期输入域。

上一轮审计结论为通过，无待搬入的缺陷判别检查，红绿回归要求不适用。环境失败不伪称产品回归红灯，也不改测试绕过权限。

## 后续

需在允许 127.0.0.1 监听及 ps 的环境复跑上述失败子集，并完成 M-01 原样验收后报待审。H-01..H-04 保持人工验收，不声称本轮完成。共享接线复验由 M-01 集中承载，本轮不推进其它里程碑。
