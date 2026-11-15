# M-04 · r26 · 实现修复证据

本轮只处理宿主在 a92adbebdb 上报告的 Web e2e 全局准备超时，不推进新里程碑，不改变任何里程碑状态。implementation-round 为 26/65。

## 失败与修复范围

- 失败步骤：`e2e/harness/fixture.ts::prepare` 调用 `bash w7-flow.sh ROOT CHECKOUT`，60,000 ms 后 `spawnSync bash ETIMEDOUT`；浏览器用例尚未开始，并非 B-3 断言再次失败。
- 同类检查：task-detail-real、task-docs-real、task-timeline-real、task-merge-real、web-stop-gate 对同一脚本已使用 150,000 ms；task-ops-real 的组合脚本另分配 150,000 + 170,000 ms。此次将唯一 60 秒的 e2e 调用对齐到既有 150 秒预算，保留有限超时、原脚本、数据量、失败传播及清理逻辑。
- 机制证据只能证明调用预算不一致；不能证明宿主脚本实际能在 150 秒内结束，也不能排除宿主存在其他慢操作。真实耗时和整体通过结论留给宿主复验，不将替身结果冒充真实执行。
- 上一轮审计通过，无审计判别脚本需要搬入。新增项目回归 `test/e2e-fixture.test.ts` 直接调用生产 prepare，以子进程替身模拟耗时，保留原错误文本 `spawnSync bash ETIMEDOUT`。按现有分组守卫归入 kernel。

## 命令与结果

工作目录除注明外为仓库根；日志均在 `.autome/output/`。

1. 原始准备路径尝试：使用 Node 导入 `prepare`，创建临时 root，先执行现有 `leftoverProcesses`，finally 执行 `clearProcesses`。进程检查被 `spawnSync ps EPERM` 阻断，未启动夹具与内核。日志 `r26-repro.log`。这不是原超时的本机复现证据。
2. 修复前：`cd apps/web && npx vitest run test/e2e-fixture.test.ts`，总数 4，2 失败、2 通过；60,001 / 150,000 ms 的合法完成被旧预算拒绝。日志 `r26-red.log`。
3. 修复后：`cd apps/web && npx vitest run test/e2e-fixture.test.ts test/test-groups.test.ts`，2 文件共 8 例通过，0 失败、0 跳过。日志 `r26-green.log`。首次联跑发现 shell 标记不允许归 unit，已按既有规则改归 kernel，未修改守卫。
4. 相关浏览器子集：`cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3'`，全局 setup 的 `ps EPERM` 阻断，实际执行 0 例；清理也报告同一权限限制。日志 `r26-browser.log`。服务与内核均未启动，未接触真实账本。
5. 改动文件类型检查：`cd apps/web && npx tsc --ignoreConfig --types node --noEmit --strict --module nodenext --target ES2023 --allowImportingTsExtensions --skipLibCheck test/e2e-fixture.test.ts e2e/harness/fixture.ts test-groups.ts`，退出 0，日志 `r26-types.log`。仅检查本轮文件与其导入，未运行项目 typecheck 门禁。
6. `cd apps/web && npx eslint e2e/harness/fixture.ts test/e2e-fixture.test.ts test-groups.ts`，退出 0，日志 `r26-eslint.log`。

用例总数：本轮正式相关子集 8 = 新增预算/错误回归 4 + 原有分组守卫 4；同范围上一轮基线为 4，净增 4。上一实现轮另选业务子集 50 例，本轮选择范围不同（8 相比 50 为 -42：不重跑其余 46 个业务例，新增 4），不表示删减测试。Web e2e 仍为原有 29 例，增量 0；本轮仅选择 B-3，未获得浏览器执行通过证据。红绿复跑不重复计入用例总数。

## 自审清单

1. 情形表逐行：共享脚本完成→继续创建草稿与取消卡片并返回 projectId，两条通过；超预算→立即抛错且不创建卡片，通过；命令失败→保留错误且不创建卡片，通过。设计 B-1/B-2/B-3 产品行为本轮不修改，真实浏览器覆盖受权限阻断。
2. 失败分支：超 150 秒与子命令失败均有回归；无新增授权、读盘或外部服务分支。ps 拒绝仍明确失败，不加跳过、不绕过检查。
3. 全部迁移：prepare 成功→卡片初始化、prepare 超时/失败→不执行后续 CLI，均检查调用次数；未新增产品状态迁移。
4. 输入域边界：60,001 ms（超过旧预算）、150,000 ms（新预算上限）、150,001 ms（超新预算）已执行。无新增用户输入域，日期/字符串边界不适用。

未运行 ci-local、ci-quick 或整套项目门禁。快检与全量结果以 Autome 提交后及合并前复验为准。
