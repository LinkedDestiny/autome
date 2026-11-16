# M-01 实现轮 1

结论：实现已落地，验收未齐，保持开放。implementation-round 为 1/30；没有前轮审计，不存在需逐字移植的审计回归检查。未运行 ci-local.sh、ci-quick.sh 或任何项目全量门禁。

## 改动

- 新增 limits、budget、gate-reporter、reap；配置接入声明上限 720 秒与 deadline 截短。
- 总耗时、余量、结构化报告及 digest；只有 CI_E2E_STAGE=quick 的完整运行余量低于 20% 才改判失败。
- 用例超时、源码字面量断言超时、动态附件和 harness 计时汇总。三处表达式断言改用 timedExpect；AST 守卫检查所有 spec。
- setup 创建根时登记、服务 spawn 后登记 pid/组；teardown 与 reporter 独立调用幂等回收，扫描失败保留登记并作为非用例错误。
- 新增 12 个报告器测试和 5 个非 unit 运行器测试；真实报告样本保存到 apps/web/test/fixtures/pw-gate/report-margin-low.json（本次 duration=3789.687ms，声明上限 4000ms，passed、complete=true、margin.low=true、enforced=false）。

## 命令与结果

原始输出均在 `.autome/output/`。

1. `npm ci --prefix apps/web --offline` 成功（m01-install.txt）。初始 npx 时本 worktree 无 node_modules，改用离线锁文件安装。
2. `(cd apps/web && UPDATE_PW_GATE_FIXTURE=1 npx vitest run test/gate-reporter.test.ts test/gate-reporter-run.test.ts test/ci-config.test.ts test/test-groups.test.ts)`：34 个用例，29 通过、5 失败、0 跳过（m01-acceptance.txt）。失败均为 `spawnSync ps EPERM`：前四项运行器已运行但无法执行 leftoverProcesses 断言；第五项在服务启动前检查进程权限时被阻断。
3. `(cd apps/web && npx playwright test --list > ../../.autome/output/m01-list.txt && grep -q 'Total: 29 tests in 5 files' ../../.autome/output/m01-list.txt)` 成功，29 条 / 5 文件。
4. `npm run build --prefix apps/web && cargo build -p automed -p autome-cli && npm run e2e --prefix apps/web`：前两项成功（m01-build.txt）；e2e 退出 1，setup 的 leftoverProcesses 遇到 ps EPERM，29 条均未进入用例执行（m01-e2e.txt）。不能提供整套通过、两次 prepare 实测耗时或无残留证明。
5. 改动文件的 `npx eslint ... --max-warnings 0` 成功（m01-eslint.txt）。最终逻辑子集、真实 --list、`npx tsc --noEmit` 结果见 m01-static-final.txt。

用例总数与基线：上一实现轮不存在；以本轮开始的专项基线 ci-config 13 + test-groups 4 = 17 为比较基线，本轮 34，增加 17，来自 gate-reporter 12 + gate-reporter-run 5。浏览器清单仍 29（增量 0），实际执行 0，不将清单计数计作通过数。重复运行不累加用例数。

## 标待审前的四项自审（已执行，未全部通过，所以不标待审）

1. **情形表逐行**：余量充足、快检余量不足、合并前门禁余量不足、80% 恰等、不限时、截短按声明上限比较、截断不比较、用例过半、字面量断言过半、动态断言失败附件、源码预算守卫、harness 与结构化字段均有逻辑检查并通过。真实运行器正常/两种余量口径/poll/截断清理五行均已尝试，受 ps 权限阻断未通过。合并前口径样本真实生成；不能用此替代整项验收。
2. **失败分支**：动态断言抛错仍记录耗时、非用例错误改判失败、未执行用例字段检查通过；真实 setup 失败和清理扫描失败已在 e2e 日志观察到。预算截断后杀进程/无残留尚未取得通过证据。授权拒绝、外部服务不可达不适用（本里程碑不含业务授权或外部服务）；源码读盘失败有错误报告分支，尚缺独立用例。没有把环境失败改成 skip。
3. **全部迁移**：passed→passed、passed+快检余量不足→failed、passed+合并前余量不足→passed、timedout→不做余量比较、非用例错误→failed 均覆盖。登记→TERM→等待→KILL→复查→删除，以及 setup 失败/正常/整套/子集的清理迁移，无法在此沙箱取得完整验证；仍为开放项。首次测试暴露 ps 不可用后，已在服务夹具启动前加入权限检查，避免后续继续启动不可验证清理的服务；早期尝试的无残留结果无法确认。
4. **输入域边界**：0 上限、恰好 80% 与多 1ms、截短生效上限、空套件、单用例、动态 1ms、数字下划线均有检查。日期、跨年、闰日不适用（无日期输入）。deadline 过期夹至 1ms；该配置边界及清理时限/不存在登记的更多异常分支仍需补强。设计负向变异对照本轮未执行，不能声称已验证。

## 后续完成条件

在允许 ps 的宿主完成 5 个运行器测试及整套浏览器验收、补齐上述失败/边界与清理状态检查和设计负向对照，再决定是否待审。报告器 setup 失败时的清单回退属于 M-03，不在本轮扩展。只有取得 M-01 验收通过证据后才改为待审；本轮不推进其它里程碑、不增加 reopen。

最终补充：逻辑子集 29/29、真实 --list 的 29 条断言、tsc 均退出 0（m01-static-final.txt）。运行器夹具临时目录与 outputDir 已统一移到 `.autome/output/`，相关 ESLint 与 tsc 退出 0（m01-final-code-check.txt）；最后的运行器复验见 m01-runner-final.txt。全套验收统计仍为上述 34 个，不因重复子集增加总数。
