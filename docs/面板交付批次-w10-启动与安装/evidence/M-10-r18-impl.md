# M-10 · 实现 #18

结论：修复会话时间线的节点投影，模型与 SSR 回归通过；浏览器验收被 `spawnSync ps EPERM` 阻塞，M-10 保持开放，reopen 仍为 1。尚未取得 20 条全过、残留检查和总耗时 ≤180 秒证据，不标待审。

## 修复与审计回归

- 上轮审计的 D-2 检查已经位于项目测试体系 `apps/web/e2e/flows.spec.ts`；本轮逐字保留整个用例，包括 `[data-testid=timeline-entry][data-node=${node}]` 的断言，没有换选择器或放宽超时。
- 修改前执行 `cd apps/web && npx playwright test e2e/flows.spec.ts -g D-2`：setup 的进程扫描报 `spawnSync ps EPERM`，未进入用例。这是环境失败，不冒充缺陷红灯。日志：`.autome/output/m10-r18-red-e2e.txt`。
- 源码核对发现更完整的根因：`crates/automed/src/scheduler.rs` 写入 `session.started` 的字段是 `kind`，没有顶层 `node`；`task.get` 的会话也提供结构化 `kind`。旧代码仅取事件的 `payload.node`，因此即使事件已经到达，仍可能得到空节点。未修改内核。
- 时间线现在从会话 `kind` 取节点：Bug 使用 step；角色 plan→design、impl→implement，其余保持角色值；intake/onboarding 保持自身值。旧数据没有 kind 时仍兼容事件 node，完全无依据时不伪造节点。无需反查中文标签，也无需依赖事件历史是否截断。
- 新增 3 条项目回归，使用既有真实 Bug 回包夹具覆盖 locate/fix/fix_review 的运行态、空事件和事件到达、SSR 的 data-node；另覆盖全部需求角色与系统会话、旧数据边界。
- 修改前单跑 `test/task-timeline.test.ts`：15 条，2 失败、13 通过；修复后同文件 15 条全部通过。红绿日志分别为 `.autome/output/m10-r18-red-unit.txt`、`.autome/output/m10-r18-green-unit.txt`（后者同时包含预检）。浏览器原检查修后由下面原验收命令覆盖，但仍在 setup 阻塞，未宣称原 D-2 已绿。

## 命令与结果

原始输出均在 `.autome/output/`，以下只记结论。

| 命令 | 结果 | 日志文件（上述目录内） |
|---|---|---|
| `cd apps/web && npx vitest run test/task-timeline.test.ts test/e2e-preflight.test.ts`（修改前） | 15 条通过，0 失败、0 跳过 | m10-r18-baseline.txt |
| 同一 vitest 命令（修复后） | 18 条通过，0 失败、0 跳过 | m10-r18-green-unit.txt |
| `npm run build --prefix apps/web` | 生产构建通过 | m10-r18-build.txt |
| `cd apps/web && npx eslint app/components/task-timeline-model.ts test/task-timeline.test.ts --max-warnings 0` | 通过 | m10-r18-lint.txt |
| `cd apps/web && npx tsc --noEmit` | 通过 | m10-r18-types.txt |
| `cd apps/web && /usr/bin/time -p npm run e2e` | 退出码 1，setup/teardown 进程扫描 EPERM；real 0.63 秒，不是套件性能结果 | m10-r18-e2e.txt |
| `cd apps/web && npx playwright test --list` | 4 文件、20 条 | m10-r18-list.txt |
| `git diff --check` | 通过 | 无输出 |

用例总数：本轮所选范围 **38 条 = 18 vitest + 20 Playwright**，实际执行 18 条，全部通过；20 条浏览器用例在 setup 前置阶段被阻塞，不算通过或主动跳过。相对实现 #17 的 23 条范围 **+15**：纳入既有时间线 12 条，新增回归 3 条；预检 3 条和浏览器 20 条均不变。相对本轮修改前同一单测集合 15 条 **+3**，均来自新增回归。上轮审计的浏览器基线是 20 条（12 通过、1 失败、7 未运行），本轮发现数差 **0**，实际没有进入浏览器用例。未跑全量门禁和宿主快检组合命令。

## 标待审前的四项自审（当前不满足标待审条件）

1. **情形表逐行**：D-1 网页需求草稿→开始→批准→合并、D-2 CLI Bug 逐轮→合并、D-3 CLI 批准五秒内换列、D-4 离线→补齐，均保留原项目用例并随验收尝试运行；全部被 setup 的 ps 权限阻塞。D-2 的节点投影另经真实回包模型与 SSR 红绿回归验证。D-7 发现 20 条成功，总耗时与清理尚未验收。
2. **失败分支**：实际检查夹具前置失败，ps EPERM 明确报错且不被吞成成功；预检的缺浏览器、缺构建、齐全 3 条通过。时间线无事件、无 kind、无节点依据分支已跑。设计点名的内核异常退出恢复 D-4、整页 reload/遗漏 events.since/错误 tick 的负向对照需要浏览器，未运行。拒绝授权、外部服务不可达、读盘失败不是本次投影新增分支，不适用；不代表整套产品已覆盖。
3. **全部迁移**：单测实际覆盖 Bug locate/fix/fix_review 在事件缺失→到达时节点与会话键不变；需求 plan/review/adjudicate/impl/audit/retro、intake/onboarding 的映射均通过。完整需求 todo→running→needs_you→running→await_merge→done、Bug locate→repro_check→fix→fix_gate→fix_review→await_merge→done，以及网页/CLI→账本→WebSocket→页面和在线→离线→恢复补齐均已有 e2e 断言，本轮运行阻塞，未验证通过。
4. **域边界**：空事件、缺 kind、旧事件 node、完全缺节点依据、与节点无关的自定义中文标签均已通过；既有时间线测试的空输入、闰日、跨年、零金额一起通过。此改动不引入数值输入域，前导零/最大值不适用。3000ms tick、5000ms 更新、170 秒总超时配置未放宽，180 秒验收上限尚待实际测量。

## 后续

在允许 ps、监听与 Chromium 的执行环境重跑原验收命令；如仍超时，按设计 R-6 压缩夹具，保留 D-3 的生产 3000ms tick 与所有断言。本轮没有凭静态推测宣称时限已满足，没有启动下一会话。
