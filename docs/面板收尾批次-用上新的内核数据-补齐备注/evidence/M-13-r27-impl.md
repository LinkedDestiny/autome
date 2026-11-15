# 实现 #27：D-1 运行中状态的观察屏障

本轮是宿主门禁修复轮，挂在最近的 M-13 下记录；不推进新里程碑，不改里程碑表，保留审计已确认的状态。implementation-round 为 27/65。

## 失败与依据

- 宿主 da78fdf7fb 的 Web e2e：D-1 在 `await expect(inColumn('running')).toBeVisible();` 失败；19 通过、1 失败、9 未执行，共 29 例，另有 2 个非用例错误。提供的尾部没有那 2 个错误的详情，本轮不臆测其原因。
- 上一审计 #18 已通过，没有新增退回检查需要搬运。宿主失败断言本来就在 `apps/web/e2e/flows.spec.ts`，本轮逐字保留并先原样执行。
- 原样命令：`npm run e2e --prefix apps/web -- e2e/flows.spec.ts -g 'D-1'`。初始化 `leftoverProcesses` 执行 `ps` 被 EPERM 拒绝，退出 1，未进入 D-1；日志 `.autome/output/r27-d1-before.txt`。不能把环境失败称为原缺陷复现。
- 因果证据：`setup.ts` 功能实例 tick 为 100 ms；`live-routes.ts` 重验合并窗口为 1000 ms；`flowSteps` 原来只给 Bug 步骤加屏障，需求的前四步都是写文档后立即结束（`w7-flow.sh`）。因此需求可能在浏览器再次取数前抵达等批准，中间“运行中”没有稳定的观察窗口。此时加长可见性超时也无法找回已经消失的状态。这是可确认的夹具竞态，但由于浏览器受阻，不能声称排除了宿主运行中的其它原因。

## 修复与同类检查

- `flowSteps` 增加缺省关闭的 `holdFirstStep`；仅 D-1 开启。第一步沿用 Bug 的放行文件协议和 600 次轮询上限，未放行退出 93。
- D-1 在原“运行中可见”断言成功后写放行文件，再执行原来的等批准、通知、批准、合并、完成和 `loads === 1` 断言。没有改 UI、内核、重验、超时或跳过用例。
- 检查所有 `flowSteps` 调用：controls 的备注、依赖前置和 BL-01 保持缺省无屏障；D-2 保持每步各自放行。需求后续步骤无新增阻塞。

## 验证与用例总数

1. 先加回归测试、尚未修夹具时：`cd apps/web && npx vitest run test/e2e-fixture.test.ts`，8 例，6 通过、2 失败。第一步没有轮询就写完正文；未放行时返回 0 而非 93。日志 `.autome/output/r27-barrier-red.txt`。这是夹具行为的红测，不是浏览器 D-1 的红测。
2. 修复后：`cd apps/web && npx vitest run test/e2e-fixture.test.ts test/test-groups.test.ts`，12 例全通过，无跳过。日志 `.autome/output/r27-barrier-green.txt`。同范围原基线 8 例（fixture 4 + 分组守卫 4），现为 fixture 8 + 守卫 4，净增 4：第一步观察后放行且第二步自由执行、未放行有界退出、feature 缺省无屏障、Bug 缺省每步有屏障。上一审计的 M-13 验收 11 例是另一范围，本轮没有重跑或冒称覆盖；宿主全量 29 条 e2e 数不变。
3. `cd apps/web && npx eslint e2e/flows.spec.ts e2e/harness/fixture.ts test/e2e-fixture.test.ts --max-warnings 0`，退出 0。日志 `.autome/output/r27-eslint.txt`。
4. 修后原流程子集：`npm run e2e --prefix apps/web -- e2e/flows.spec.ts`，退出 1，仍在初始化因 `ps EPERM` 阻断，D-1/D-2 两例均未执行，不能报通过。日志 `.autome/output/r27-flows-after.txt`。交宿主原命令复验，相关 controls 默认行为由宿主全量 e2e 验证。
5. `git diff --check` 通过。未运行全量门禁或快检；它们以 Autome 提交后的执行结果为准。

## 四项自审清单

1. **情形表逐行**：需求默认流程不持有（通过）；D-1 第一轮持有（经过三次观察才放行，通过）；需求后续轮自由执行（通过）；Bug 全步骤持有（两步默认屏障检查通过）。设计总-Q2 的功能 100 ms / 生产 3000 ms 配置未改。
2. **失败分支**：放行文件始终缺失，599 次模拟 sleep 后第 600 次检查退出 93，正文不执行（真实 shell 测试通过，sleep 替身立即返回以避免等待一分钟）。拒绝授权、外部服务、配置读盘等产品失败分支不适用：本轮仅测试夹具变更。进程枚举拒绝在浏览器初始化实遇，保留失败而不绕过。
3. **全部迁移**：首步等待 → 观察三次 → 放行 → 正文完成，以及等待 → 上限耗尽 → 93 两条均通过；第二步直接完成通过。产品 todo → running → needs_you → done 的原断言完整保留，但真实浏览器迁移本轮未执行，待宿主。
4. **输入域边界**：可选项省略与 true、step 1 与 step 2、feature 与 bug、放行存在与持续缺失均覆盖；数字日期、空正文与最大业务输入不适用，未改这些输入域。

## 环境与交付

浏览器两次都在 spawn 服务之前被进程扫描拦住，没有启动面板、内核或浏览器。回归只启动同步等待收割的短命 shell，临时目录由 afterEach 删除；未接触真实账本。未调整全量门禁预算；全量耗时与原非用例错误仍需宿主复验。
