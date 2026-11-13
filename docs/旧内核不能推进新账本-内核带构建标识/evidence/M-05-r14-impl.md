# 实现 #14：Web 门禁组合夹具预算修复

## 范围与原始证据

本轮为门禁修复轮，不推进新里程碑；M-01 至 M-07 保持已有审计状态。implementation-round 为 14/35。上一审计没有未闭合缺陷，本轮没有需要移植的审计临时检查；复现检查本身已在项目测试体系内。

只读定位宿主日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-84/merge_gate-2ec1e891438a.log`：Web 共 110 文件、828 用例，812 通过、16 跳过，唯一失败套件为 `task-ops-real.test.ts`。其 beforeAll 的 `bash .../w7-states.sh` 在 170694ms 失败；W9 HTTP 已通过。提取结果位于 `.autome/output/r14-host-failure.txt`。

原样执行 `npm run test --prefix apps/web -- test/task-ops-real.test.ts`，本机同样在 170013ms 失败、16 例全部跳过，原始输出 `.autome/output/r14-red.txt`（红）。另外清理出现 `spawnSync ps EPERM`，这是本机沙箱限制，与宿主初始化超时分开记录。

只读检查临时账本 dashboard 与事件，以及 fixture 的 state-ids，确认任务持续从 intake/review 推进至等待审批等状态，基础流程完成后仍在构建扩展矩阵，未发现死锁。诊断文件为 `.autome/output/r14-dashboard*.json`、`r14-dashboard.txt`、`r14-events.json`；全部仅访问本次临时 AUTOME_HOME。

## 修改与验证

`w7-states.sh` 先 source 完整 `w7-flow.sh`，再建立额外 23 个任务。其他直接调用基础流程的测试给它 150 秒；原组合夹具却只有总计 170 秒。本轮把组合脚本有限预算设为 `150_000 + 170_000`，hook 在其上增加 60 秒容纳 30 秒内核握手及失败清理。所有操作断言、状态矩阵、真实内核和进程泄漏检查保留，不跳过用例、不改产品行为。同类检索确认 w7-states.sh 只有这一个调用方；其它基础流程调用方无宿主失败证据，不扩大修改。

修复后执行：

`npm run test --prefix apps/web -- test/task-ops-real.test.ts test/task-ops.test.ts test/task-ops-view.test.ts`

结果见 `.autome/output/r14-after.txt`：3 文件、58 用例，57 通过、1 失败、0 跳过，总耗时 133.02 秒。原套件 15 个业务用例全部通过，剩余「收尾无存活进程」及 afterAll 因 `spawnSync ps EPERM` 失败。初始化超时未再出现，但这次耗时也低于原 170 秒，说明耗时有波动，不能把单次成功解释为确定性的性能改善。增加预算针对组合夹具的时间分配，宿主仍须复验该 16 例及无泄漏检查；本机没有取得完整套件绿证据。

用例总数：本轮 58 个唯一用例，较实现 #13 的 58 差 0，但选择范围不同：本轮为真实操作 16 + 操作模型 38 + 操作视图 4；没有新增、删除或改写测试用例。与宿主原套件 16 例相比差 0，另运行相关子集 42 例；未运行其余宿主用例，不宣称 828 例全绿。修复前后重复执行的 16 例不重复计数。

`git diff --check` 通过。未运行全量门禁或宿主快检。全量结果以宿主合并前重跑为准，快检以提交后宿主重跑为准。

## 四项自审清单

1. **情形表逐行**：需求状态矩阵覆盖 draft、等待审批、会话失败、预算耗尽、active、paused、stopped、cancelled；Bug 矩阵覆盖 draft、待补充、不是 Bug、active。两个真实矩阵及 42 个模型/视图检查通过；不可用操作拒绝且状态不变的断言原样保留。
2. **失败分支**：初始化超时原样复现；内核拒绝原文透传 action 通过；失败清理路径确实执行但 ps 被沙箱拒绝，未取证无泄漏。授权、读盘失败、远端不可达等产品新分支不适用，本轮仅调整测试预算。
3. **全部迁移**：真实 approve/reject/reply/merge/start/pause/resume/stop/cancel/rerun/extend/convert 全部通过；取消与转换的终态检查保留。未改变产品状态机；进程收尾断言未通过，待宿主复验。
4. **域边界**：不适用新增产品输入域，本轮没有日期、数值或文本输入修改。超时仍是有限正数，脚本上限 320 秒、hook 上限 380 秒，握手仍为 30 秒；未引入无限等待或固定 sleep。

## 交付

保留设计文档中 M-05 已关闭里程碑的审计证据指针（宿主此前已将其恢复到审计结论），本轮修复证据由 retro 指向本文件；更新轮数与 next-action。宿主需复验初始化耗时与进程清理，再运行快检和合并前门禁。H-01 至 H-03 人工验收不在本轮范围。不启动下一会话、不推送远端。
