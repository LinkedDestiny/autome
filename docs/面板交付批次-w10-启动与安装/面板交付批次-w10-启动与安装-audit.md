# 审计结论 · M-01 · 审计 #1

结论：**实现缺陷**，退回开放，reopen 1，领域 launcher。收敛模式 normal。

失败命令：`cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-stop-gate.test.ts test/core-supervisor.test.ts`
输出尾部：`Test Files 1 failed | 3 passed (4)`，`Tests 4 failed | 36 passed (40)`；失败全在 web-stop-gate.test.ts：
`TypeError: Cannot read properties of undefined (reading 'pid')`（test/web-stop-gate.test.ts:22）与 `expect.poll() must be called inside a test`（test/fixtures/web-launcher.ts:49）。

违反：M-01 验收契约「vitest 4 个文件全过」（A-2 门禁分支无证据）。
根因：测试在门禁子进程出现之前取 pid（门禁进程比 merge_gate 状态晚起，推迟 8 秒后出现）；fixture 清理在测试体外调用 expect.poll。
下一轮实现轮：改 setup 轮询等待 gate.sh 进程出现；cleanup 不用 expect.poll；再跑 4 个文件全过，并核对 web-launcher 第 10、11 条与 web-instance 第 14 条。
复验过的部分：web-instance 14、web-launcher 12、core-supervisor 基线全过；`run.sh card panel` 29/29；两条负向对照有效。证据：docs/面板交付批次-w10-启动与安装/evidence/M-01-r1-audit.md
