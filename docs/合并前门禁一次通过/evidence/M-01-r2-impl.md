# M-01 实现轮 2（快检修复）

本轮为 2/30，只修复宿主点名的预算截断清理问题，不推进新里程碑。进入本轮时 M-01 实际为开放，继续保持开放；没有审计文件，也没有需移植的审计临时检查。宿主失败用例已在项目测试体系内，保留原样。未跑全量门禁或整套浏览器验收。

## 根因与修改

`reapRegistered()` 用同步 `Atomics.wait` 阻塞父进程事件循环，发信号后 Node 无法及时处理子进程退出，复查可能仍看到未回收的进程，继而误报清理失败并保留登记根。新增“截断报告等待退出回调后再复查和删除登记根”回归：信号送达后仅在事件循环获得机会时完成退出；修复前确定失败，修复后通过。该检查验证机制，不冒充宿主真实进程证据；宿主日志尾部未提供具体失败断言，最终同因判断以宿主复验为准。

改用异步 delay，并逐一更新全部现有调用点（报告器 onEnd、globalTeardown）等待清理完成。清理仍独立于 Playwright 的过期 deadline；不忽略残留或扫描错误。增加扫描拒绝时保留登记与根、报告失败的负向控制。设计中的同步执行描述同步修正为等待异步清理完成。

## 命令、结果与用例总数

原始输出在 `.autome/output/`；重复运行不累计用例总数。

- 修复前原样运行宿主失败步骤：`sh -c "cd apps/web && npx vitest run --project 'exclusive' 'test/gate-reporter-run.test.ts'"`，5 个用例均被 `spawnSync ps EPERM` 阻断；日志 `m01-r2-red.txt`。没有把环境阻断说成宿主缺陷复现。
- 新回归先红：`cd apps/web && npx vitest run test/gate-reporter.test.ts`，14 个，13 通过、1 失败；失败为退出回调得不到执行后报告出现“清理失败”。日志 `m01-r2-regression-red.txt`。
- 修复后专项：`(cd apps/web && npx vitest run test/gate-reporter.test.ts test/gate-reporter-run.test.ts test/ci-config.test.ts test/test-groups.test.ts)`，36 个，31 通过、5 失败、0 跳过；失败全部为 ps EPERM，日志 `m01-r2-acceptance.txt`。较上轮基线 34 增加 2，来自退出回调回归与扫描失败负向控制；原运行器 5 个保持不变。
- 单独绿检：`(cd apps/web && npx vitest run test/gate-reporter.test.ts test/ci-config.test.ts test/test-groups.test.ts && npx playwright test --list > ../../.autome/output/m01-r2-list.txt && grep -q 'Total: 29 tests in 5 files' ../../.autome/output/m01-r2-list.txt)`。日志 `m01-r2-green.txt`；31 个通过，浏览器清单 29 条（增量 0，不计作执行通过）。
- 修复后再次原样运行上述宿主失败步骤，日志 `m01-r2-runner-after.txt`；仍 5 个 ps EPERM，无法在当前沙箱取得真实清理绿证据。
- 按本轮最后指令执行同一条快检：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`。5 步、失败 0、17 秒；日志 `m01-r2-quick.txt`。Format、Clippy、Web install、lint、typecheck 通过；明确输出“未提供本轮起点，未跑改动相关的测试”。因此该通过不能替代宿主带上下文的快检，最终以提交后 Autome 重跑为准。

## 标待审前的四项自审（本轮保持开放）

1. **情形表逐行**：本轮相关的正常结束、快检低余量失败、合并前低余量只报告、预算截断四行，原报告器逻辑检查通过；新增截断清理回调与重复清理通过。真实运行器五行已逐项尝试，均受 ps EPERM 阻断。整套/子集真实清理与其他上轮未齐验收没有宣称完成。
2. **失败分支**：扫描失败保留根与登记、写入非用例错误且使报告失败，新增检查通过；截断后异步退出回调通过；原非用例错误/动态断言失败检查通过。拒绝业务授权、外部服务不可达不适用（清理路径不涉及）；读盘失败等上轮缺口未在本次窄修复扩展。
3. **全部迁移**：登记→TERM 整组→事件循环处理退出→复查空→删除根与登记→幂等再次清理通过，且断言无需 KILL；扫描失败→保留登记/根→报告失败通过。真实 TERM 超时→KILL、setup 失败与完整运行的进程迁移仍需允许 ps 的环境复验，不标待审。
4. **输入域边界**：单登记、空登记（幂等第二次调用）、200ms 有界等待均执行；原 0 上限、80% 恰等/超出、截短与空套件检查通过。日期、闰日等不适用，无日期输入。更完整清理时限边界仍属 M-01 未齐范围。

## 交付边界

真实清理与整套验收仍待宿主环境；不改 skip、不削弱原运行器断言、不标已完成、不启动下一会话。提交尝试失败时由 Autome 兜底。
