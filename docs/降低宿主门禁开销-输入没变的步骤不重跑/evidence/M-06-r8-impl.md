# M-06 实现 #8

## 结论与修复

本轮只推进 M-06，implementation-round 为 8/30。修复审计 #7 的内联样式缺陷：未核实摘要改用 timeline-unverified 类，CSS 仍使用原错误色 token。同步时间线测试验证警示类、CSS 颜色声明及已核实条目不带警示类。全仓 TSX 原有样式守卫保持逐字不变，未删减测试。

M-06 保持开放（reopen 保持 2）：本会话不能完成监听和进程检查，尚无全套验收通过证据，不标待审，不宣称 V-5 通过。不是协议失败。

## 命令与结果

原始输出均在 .autome/output/m06-r8/。

- 审计原命令：`cd apps/web && npx vitest run test/styles.test.ts`。修改前 styles-red.log：12 个，11 通过、1 失败，失败为「TSX 没有内联样式」原断言 `expect(tsx).not.toMatch(/style=\{/);`。修改后原样重跑 styles-green.log：12/12 通过。回归已存在于项目测试体系，无需复制同一用例。
- `cd apps/web && npx vitest run test/styles.test.ts test/task-timeline.test.ts test/test-groups.test.ts`：unit-final.log，28/28 通过。第一次修复后暴露时间线旧内联颜色断言（unit-green.log），已改为类与颜色规则的联合断言，并验证无警告分支。
- `node --test scripts/test/web-groups.test.mjs`：groups.log，1/1 通过，内部三个 project 的既有 28 个 Web 用例通过。
- `node --test scripts/test/web-build.test.mjs scripts/test/web-pidfile.test.mjs`：fixtures.log，4/4 通过，构建共享及参数转发回归不变。
- `cd apps/web && npx vitest run test/test-groups.test.ts test/core-sidecar.test.ts test/eventbus-live.test.ts test/logtail-live-cleanup.test.ts test/project-confirm-real.test.ts`：integration.log，总数 35，27 通过、1 失败、7 被 beforeAll 失败跳过，另有一个套件初始化失败。eventbus-live 有 EMFILE 警告及明确的 `listen EPERM: operation not permitted 127.0.0.1`，logtail-live-cleanup 为 `spawnSync ps EPERM`。未新增条件跳过。
- `ps -axo pid=,ppid=,command=`：ps.log，operation not permitted；无法给出无残留证明。
- `git diff --check`：通过。未运行完整门禁、fmt、Clippy、lint 或 typecheck，按本轮指令留给宿主。

## 用例总数与基线差

本轮未增加或删除用例，全量静态预期仍为上一审计的 111 文件 / 840 用例，差 0；没有声称本轮已全量跑到该数字。样式 12、时间线 12、分组 4，共 28，与相同子集基线差 0。里程碑集成子集原 30，本轮加选既有 project-confirm-real 5 个，命令选择总数为 35（+5 来自扩大诊断范围，不是新测试）。脚本共 5 个，差 0，其内部 Web 28 个是重复执行，不能累计成新增覆盖。

## 四项自审清单

1. **情形表逐行**：D.1 unit/kernel 同调度组且 worker 上限一致、exclusive 后置串行、所有文件唯一归属、资源测试不进 unit、配置不按环境增减文件：分组 4 项与跨三组 runner 通过。共享构建：web-build 2 项通过。D.2 空闲/负载交替各三次、五次连续全绿及残留：监听/ps 被拒，未取得；未启动已知无法通过的长基准。D.3 包装器参数和版本探针：web-pidfile 2 项通过；前后各二十次运行未取得。
2. **失败分支**：未核实条目红色提示与无未核实条目的普通显示均由时间线测试通过。设计要求的全量失败及资源清理分支实际遇到监听权限拒绝、进程查询拒绝，见 integration.log。外部服务不可达、读盘失败、空输入不是本次 CSS 修复新增分支，不适用；不将环境拒绝算产品验收通过。
3. **全部迁移**：本次 CSS 不引入状态机；时间线现有 12 项覆盖运行/结束/重试等事件，未核实计数正值与零值均通过。分组先并行后 exclusive 的真实启动检查通过；完整负载下资源生命周期未完成，保留开放。
4. **域边界**：未核实计数 1 与 0、空时间线及无步骤报告均通过既有时间线测试。日期、最大值等不属于本次样式变更输入域，不适用；文件归组完整性由 test-groups 检查通过。

## 偶发失败调查与剩余工作

project-confirm-real 本次在同一四 worker 配置的集成子集中 5/5 通过，但不据此否认上轮缺陷。读取审计原始 fr.log 后发现它记录了该用例失败标题，失败区却只有 styles 的详细断言；「缺少 Git」来自 CLI 输出，不能仅凭日志邻近位置归因到 project.probe。captureReplies 涉及初次内核、项目注册、重启假 CLI 和异步预算报警多个阶段，当前证据不足以锁定是哪一阶段产生差异。下一轮应保留完整断言差异和阶段证据，在可监听宿主对照串行/四 worker 全量，不能通过放宽 shape 断言解决。

ENOTEMPTY 写入者仍未定位，保留有界重试。参数转发和版本探针回归通过，只能证明这两个包装器错误已修复，不能证明 ENOTEMPTY 根因。空闲性能仍归 H-01，负载口径与连续五次也未取得。本轮不改设计验收标准。
