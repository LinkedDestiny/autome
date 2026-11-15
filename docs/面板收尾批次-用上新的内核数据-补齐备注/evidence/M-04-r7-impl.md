# M-04 · 实现 #7

结论：实现完成，待独立审计。单元验收通过；浏览器三项与部分 related 环境验收受沙箱限制，**没有取得完整验收命令全绿证据**。按任务 §6 约束 7 和设计「总-Q1」将受阻项交审计轮/宿主执行，未修改断言或增加条件跳过。implementation-round 为 7/65，未关闭任何里程碑。

## 实现与修复

- 项目任务行透传 queue_position / after / blocked_by；待开始列排队优先、按位置升序，其后草稿按编号倒序。多任务队列提供原生 details / select / button，通过项目 action 发送唯一的 top / before / after 参数，原文显示错误，离线与提交中禁用。
- 新建页选择同项目未结束任务；详情页显示当前前置状态，按内核可编辑边界显示保存/清空依赖。需求历史实现会话也参与只读判断；候选排除自己。after 输入使用现有白名单的格式和 8 项上限，清空发 []。没有增加内核方法、没有改变写白名单。
- 卡片显示「等 T-n 合并」，取消/失败前置使用现有 warning token 提示；沿用现有 task 事件重验。前置状态变化但本任务尚未启动的详情页实时更新仍属 M-10。
- 新增 3 条真实内核浏览器用例，私有临时仓库、显式本检出二进制、既有 teardown 回收；B-4 同时覆盖详情页清空和保存。B-2 的第二项目 parallel=1，以占位会话守住真正启动顺序。
- related 检查发现旧字段投影断言和滚动区白名单未更新：前者加入三个实际新增字段；后者精确加入 task-notes-panel / dependency-choice-list，保留逐项与集合相等断言。labels 检查发现组件直接使用阶段词，改用 nodeLabel，文案不变。新增测试的参数集合类型诊断已修正。
- 上轮审计 M-03 通过，没有审计退回检查要逐字搬入。本轮发现的三个现有回归守卫均原样运行得红，再同步实现/精确契约后变绿。

## 命令、结果与用例数

原始输出均在 `.autome/output/`，这里仅记判据。未运行 ci-local.sh、ci-quick.sh 或整套 lint/typecheck 门禁。

1. 改前同组基线：`(cd apps/web && npx vitest run test/board.test.ts test/task-new.test.ts test/task-card.test.ts test/test-groups.test.ts)`，35/35（10+9+12+4），`m04-baseline.txt`。
2. 最终契约原命令：
   `cargo build -p automed -p autome-cli && (cd apps/web && npx vitest run test/board.test.ts test/task-new.test.ts test/task-card.test.ts test/queue-deps.test.ts test/test-groups.test.ts) && npm run build --prefix apps/web && (cd apps/web && npx playwright test e2e/controls.spec.ts -g "B-2|B-3|B-4 命令行排队")`。
   Rust 构建、Web 构建通过；vitest 5 文件 **50/50**（13+11+14+8+4），相对同组基线 **+15**（board +3、task-new +2、task-card +2、queue-deps 新增 8）。相比上一里程碑 M-03 契约基线 73 为 -23，仅因为换了里程碑与文件范围，不代表删除测试。日志 `m04-final-cargo.txt`、`m04-final-contract-unit.txt`、`m04-final-build.txt`、`m04-final-browser.txt`。
   Playwright 全局 setup 的首个进程扫描报 `spawnSync ps EPERM`，服务尚未创建。`--list` 确认 **3 个定义用例**（相对本轮同筛选改前 0 为 +3），实际执行 0，不报浏览器通过。最初尝试同样被此限制挡住（`m04-browser.txt`），最终代码再验仍受阻。
3. 相关测试命令：
   `(cd apps/web && npx vitest related --run app/components/board-data.ts app/components/task-action.ts app/components/task-card-model.ts app/components/task-card.tsx app/components/task-detail-data.ts app/components/task-new-action.ts 'app/routes/projects.$id.tasks.new.tsx' 'app/routes/projects.$id.tsx' 'app/routes/tasks.$id.tsx' app/components/queue-deps.ts app/components/queue-controls.tsx app/components/task-dependencies.tsx server/gates/write-gate.ts)`。
   初次 **388 个定义用例：320 过、44 失败、24 未执行**；56 文件，21 失败。代码断言失败 2 个（board-payload、testids）；其余失败/未执行来自 `ps EPERM` 或 `listen EPERM 127.0.0.1`，server 提前退出也为监听失败。详见 `m04-related.txt` 与 `m04-related-failures.txt`。
   修复后同命令加 `--project unit`：25 文件 **234/234**（`m04-related-unit-green.txt`）；相对全 related 的 388 少 154，原因是明确不再执行受环境影响的 kernel/exclusive 两组，未删定义、未当作全 related 通过。
4. 相关 UI 守卫：`(cd apps/web && npx vitest run test/labels.test.ts test/task-action.test.ts test/task-page.test.ts test/styles.test.ts)`，36 个用例，labels 1 红（`m04-ui-regression.txt`）。修后追加 `test/board-payload.test.ts test/testids.test.ts`：**53/53**（`m04-ui-green.txt`），+17 来自增加两个现有文件（6+11），定义数未变。
5. 三个变异逐一注入、运行并在 finally 还原：移除 queueOrder → board 失败；交换 before/after → queue-deps 参数断言失败；新建 after 固定为空 → task-new 提交断言失败。日志 `m04-negative-board.txt`、`m04-negative-direction.txt`、`m04-negative-create-after.txt`，三个退出码都是 1；还原后的契约 50/50。浏览器 top 丢参负向对照因相同 setup 限制未执行，不宣称已验证。
6. 仅对 21 个本轮改动 TS 文件运行 ESLint 与 TypeScript Compiler API 的语法/语义诊断（沿用项目 tsconfig，不替代宿主全量 typecheck）：均 0 问题；日志 `m04-lint-changed.txt`、`m04-types-final.txt`。`git diff --check` 通过。

## 标待审前自审清单

1. **情形表逐行**：B-2 队列位置透传、排序、草稿倒序、单元素无菜单、多元素只有排队有菜单由 board 11–13 通过；top/before/after 组装及原生可聚焦元素由 queue-deps 1、7 通过。B-3 候选范围/排序/排除自己、新建携带 after 由 task-new 10–11 通过；当前依赖状态、可编辑/只读原因、保存/清空由 queue-deps 3–5、8 通过；等待/取消/失败文案由 task-card 13–14 通过。B-4 复用既有事件路径，真实五秒时限未实跑；B-2/B-3 真实调度链也未实跑，见下方交接。
2. **失败分支**：非排队参照、授权拒绝、离线、超时在 queue-deps 2 检查原文返回；依赖环拒绝在 5 检查；离线按钮禁用在 7 检查；9 项、空串/坏编号拒绝在 6、新建 11 检查。HTTP 真实安全头和端口分支受 listen EPERM，不能用替身证明通过。读盘失败不适用：本轮新增流程无文件读取。
3. **全部迁移**：queue-deps 3–4 覆盖 feature/bug 草稿及初次排队、需求批准前五节点 × active/queued-resume/paused/stopped/failed、历史 impl 只读、批准后七节点、done/cancelled；5 覆盖设置和清空。B-2 占位→排队→移首→释放槽→新顺序启动、B-3 前置未完成→合并完成→依赖启动、B-4 CLI→事件→页面与页面保存/清空已写进浏览器测试，因 setup 权限未执行。不将状态表单测冒充真实迁移证据。
4. **输入域边界**：依赖数量 0/1/8/9；空串、错误前缀/非数字、逗号合并字符串、前导零 T-001（沿用白名单格式，最终由内核查存在性）均覆盖；队列空/单元素/多元素、草稿 T-010 数字排序覆盖；move 缺失/重复参数/top=no 拒绝覆盖。闰日/跨年不适用：本轮没有新增日期输入或计算。

## 环境受阻的独立复验清单

审计轮/宿主在有进程表和端口权限的环境运行上方完整契约命令与不带 `--project unit` 的完整 related 命令。浏览器待验名逐条如下：

- B-2 调序后内核按新顺序启动（含 screenshot 与真实 Q2 先启动；再做丢掉 top 的负向对照）。
- B-3 依赖任务在前置合并前不开始、合并后开始（含前置推进各阶段检查）。
- B-4 命令行排队与依赖五秒内出现（生产 3000 ms tick，无重载；详情页保存/清空）。

受阻 related 文件：kernel 的 task-detail-real、task-docs-real、task-merge-real、task-ops-real、task-timeline-real 的进程回收证明；exclusive 的 board-live-listen、gateway-http、gateway-task-card、gateway-task-lifecycle、older-kernel-real、shell-live、uploads-http、w9-http、logtail-live、notifier-live、task-actions-http、task-live-real、task-log-live、server。精确用例名见原始错误索引，不删测试、不减少宿主复验范围。

本轮没有手工启动服务；浏览器在起服务之前被进程扫描前置检查挡住。related 既有夹具调用 task.stop/core.stop 后因 ps 扫描失败保留临时目录（process-leak.ts 的既有顺序），**无法独立证明没有残留进程**，需宿主扫描复验。没有接触真实账本、launchd 或其它 worktree。真实视觉检查与 H-03/H-05 人工观感未执行。
