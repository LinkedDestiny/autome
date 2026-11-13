# M-05 实现 #6

结论：操作区与写操作已实现；验收有环境阻塞，M-05 保持开放，不标待审。实现轮为 6/30。上一轮 M-04 审计通过，没有退回缺陷或需要逐字搬运的审计检查。

## 实现与修正

- 新增表驱动 `availableOps`，覆盖需求/Bug、角色禁用、重跑节点、预算原因、T-70 授权与撤销、未消费表态条目。归档 Bug 不提供改建。
- 路由 action 统一经 `gateway.write`；14 种意图映射到已有 13 种 RPC。错误原文返回；取消要求 `confirm=yes`；重跑重新读取当前任务和配置检查节点；旧内核拒绝撤销授权，防止其忽略 cancel 后直接合并。
- 操作区紧邻失败提示；带输入的操作用 details 展开；采用既有 WriteGate，离线/提交中禁用；意见与补充信息复用 MAX_PROSE_LENGTH。组件命名为 `task-ops-view.tsx`，避免与 `task-ops.ts` 同名编译产物冲突。
- `task.convert` 源码证实是创建新需求并取消原 Bug，不是原地改类型；成功后留在本页并显示新任务 id，集成测试读回新旧两个任务。
- 仅修改 Web 与本任务文档；未改 Rust、写入口白名单、W8 内部、协议或配置；未运行项目门禁、全量测试、build、cargo fmt/clippy。

## 命令、总数与结果

1. M-05 原验收命令：
   `cd apps/web && npx vitest run test/task-ops.test.ts test/task-action.test.ts test/task-ops-view.test.ts test/task-ops-real.test.ts test/task-actions-http.test.ts test/write-gate.test.ts`
   最后完整执行：6 文件、**110 用例，106 通过、1 失败、3 未执行**。日志 `.autome/output/m05-acceptance-verified.log`。失败用例为收尾检查 `spawnSync ps EPERM`，同样导致 afterAll 失败；HTTP 三条在 beforeAll 因 `listen EPERM 127.0.0.1` 未执行。未使用 skip/删除断言隐藏阻塞。
2. HTTP 夹具新增不加载项目的监听预检，避免权限拒绝时仍启动 Vite watcher。改后单跑 `cd apps/web && npx vitest run test/task-actions-http.test.ts`：总数 3，beforeAll 同样 EPERM，3 未执行；`.autome/output/m05-http-final.log`。因此最后完整验收后的这项修改没有取得 HTTP 通过证据。
3. 直接相关回归：`cd apps/web && npx vitest run test/task-ops.test.ts test/task-action.test.ts test/task-ops-view.test.ts test/write-gate.test.ts test/labels.test.ts test/task-page.test.ts test/styles.test.ts`：**7 文件 114 用例全部通过**；`.autome/output/m05-related.log`。
4. 对本轮新增/修改 TS 文件运行 ESLint 子集，0 错误；使用 TypeScript API 按仓库 tsconfig 收集本轮文件诊断，**0**（不是宿主全量 typecheck 门禁）；`.autome/output/m05-lint.log`、`.autome/output/m05-types-final.log`。`git diff --check` 通过。
5. 纯网络预检返回 EPERM；ps 也被沙箱拒绝。所有真实测试仍执行公共 teardown 的 task.stop/core.stop；因无法枚举进程，**不声称没有残留进程，也不删除无法确认安全的临时目录**。真实浏览器视觉确认未执行，沿用 H-01/H-07。

**总数差异：**上一轮 M-04 审计基线为 16 用例；本里程碑验收 110，差 **+94**，来自换掉上轮 16 条文档验收、加入本轮 66 条新测试，并包含既有 write-gate 44 条（66 + 44 − 16 = 94），不是同一套全量测试增长 94 条。与 M-05 设计预期 110 相比差 0。相关回归 114 中有 91 条与 M-05 重叠；本轮最终命令集合去重为 **133 条**（110 + labels 8 + task-page 3 + styles 12），其中 129 通过、1 环境失败、3 因环境未执行。重复运行不重复计数。

## 夹具红绿记录

- 初跑 `.autome/output/m05-real.log`：并行 32 被内核拒绝（合法范围 1..=5）。改为项目并行 5，先造暂停/停止矩阵行，释放槽位后再验证控制操作。
- `.autome/output/m05-real-fixed.log`：extend/convert 断言失败。预算夹具停在错误状态，convert 错以为原任务变类型。改为走到裁决耗尽设计预算，并校验新需求与原 Bug。
- `.autome/output/m05-final-acceptance.log`：预算仍未耗尽。读取真实 task.get（`.autome/output/m05-budget-probe.json`）发现 current-milestone=M-02 但表里缺 M-02，实际为 intake 的协议失败。补全表，并为矩阵各行增加预期状态/失败原因断言。
- `.autome/output/m05-acceptance-verified.log`：**真实内核 15 条行为用例全部通过**，只剩第 16 条收尾的 ps 权限失败。未把环境失败算绿。

## 标待审之前的自审清单（本轮执行，但因阻塞未标待审）

### 1. 情形表逐行

纯函数每行断言完整集合；重跑行同时断言完整节点清单。下列 33 行均通过（task-ops 中 38 条用例还包含另外 5 条交叉/条件检查）。

| 类型 | 行 | 结果 |
|---|---|---|
| 需求 | 草稿；排队；带 resume 排队 | 各行通过：start/cancel 或仅 cancel |
| 需求 | 运行角色；门禁等内核步骤 | 各行通过：pause/stop/cancel；角色遍历整理、设计、评审、裁决、实现、审计、复盘 |
| 需求 | 等批准；等合并 | 各行通过：approve/reject/cancel 与 merge/cancel，无 stop |
| 需求 | merging/main_check/cleanup | 三节点分别检查通过：pause/stop/cancel |
| 需求 | 暂停；停止 | 各行通过：resume/stop/cancel 与 resume/cancel/rerun |
| 需求 | 崩溃；设计预算；实现预算 | 各行通过：失败均可 cancel/rerun，仅预算可 extend |
| 需求 | gate 熔断；merge_gate 熔断 | 各行通过：extend，重跑列表加入出事门禁 |
| 需求 | 协议失败；完成；取消 | 各行通过：cancel/rerun；终结无操作 |
| Bug | 草稿；排队 | 各行通过：start/cancel 或仅 cancel |
| Bug | 定位/修复/复核；复现/修复门禁 | 五节点分别通过：pause/stop/cancel |
| Bug | 补充信息；不是 Bug；等合并 | 各行通过：reply/cancel；reply/convert/cancel；merge/cancel |
| Bug | 暂停；停在复现检查 | 各行通过：resume/stop/cancel；resume/cancel/rerun |
| Bug | 修复预算；fix_gate 熔断；崩溃 | 各行通过：前两项 extend，熔断加入 fix_gate；崩溃不 extend |
| Bug | 完成；取消；审计关闭草稿 | 各行通过：终结无操作；开始置灰并显示理由 |

额外检查：关闭角色过滤节点并解释；T-70 授权/撤销及终结/合并中拒绝；表态不看任务状态；Rust 重跑臂和预算臂解析后集合逐项相等；所有置灰项有中文理由、归档 Bug 不提供 convert。全部通过。

真实矩阵覆盖 **12 个停点**：需求草稿、等批准、崩溃、设计预算、整理运行、暂停、停止、已取消；Bug 草稿、等补充、不是 Bug、定位运行。每个停点先断言预期状态，再逐个调用表判不可用的操作，断言拒绝且状态不变；有 cancel 的行调用 cancel 验证接受。decide 独立于状态不在矩阵比较，需求 reply 是 reject 的等价入口；旧内核 merge_cancel 禁止调用，防止被当合并。

**证明限制：**真实矩阵尚未构造第二项目的排队停点、运行门禁、完成、实现/修复预算和门禁熔断等其余停点；这些只有纯函数/源码覆盖，不能声称完成全状态真实交叉核对。设计「矩阵夹具不稳定」允许记录未构造行；本轮保守保持开放，后续可补强这些停点。没有直接改 SQLite 伪造状态。

### 2. 失败分支

- 内核 409、网关 400、离线 503、超时 504：假网关逐字透传各跑一次通过；真实 automed 对已有任务 start 拒绝，与 action 返回原文相等。
- 未确认取消、未知意图、空白意见、超长信息、非法重跑节点、旧内核撤销、非法表态：各检查不写网关，全部通过。
- 坏 Host/Origin：已写入真实 HTTP 路由测试，两路径各 14 种意图；本轮监听 EPERM，**未取证**，不以纯函数替代。
- 读盘失败不适用：M-05 不新增文件读取（文档读取属于已完成的 M-04）。外部服务不可达对应网关离线测试；不引入新外部服务。

### 3. 全部迁移

- 所有状态到可用集合的映射见上述 33 行；全部 14 意图精确映射一次 write；取消 GET 确认到 POST 的组件测试通过。
- 真实 start、approve、reject、reply、merge、pause、resume、stop、cancel、rerun、extend、convert 各有独立用例，全部通过并读回状态；convert 额外读回新任务为 feature/draft、原 Bug cancelled。
- 授权/撤销只有 T-70 纯函数与旧内核安全拒绝检查，main 内核真实兼容复验留给设计指定的审计轮；表态 UI/真实 decide 归 M-06，本轮只测 action 映射，不宣称 M-06 完成。
- 真实状态矩阵未覆盖停点如上一节，列为后续补强，不宣称逐格完备。

### 4. 输入域边界

- 轮数空串、0、负数、小数、指数、Infinity、超过 u32 最大值：拒绝且不写；1、前导零 01、4294967295：action 映射通过。此处最大值只证明参数域，不证明内核现有预算加法不溢出。
- 意见空白、信息 8000/8001 UTF-16 字符、取消确认缺省、未知/原型键 intent、非法节点与表态枚举：通过。
- 日期、闰日、跨年不适用：本里程碑没有日期输入；重跑单节点 rebase 与空/非法节点由角色过滤和 action 用例覆盖。

## 后续

M-05 保持开放；补强尚缺的真实状态停点，在允许监听和 ps 的环境复验同一验收命令，并按设计审计 T-70 兼容性。不得以这次结果替代宿主快检/合并门禁，也不启动下一会话。
