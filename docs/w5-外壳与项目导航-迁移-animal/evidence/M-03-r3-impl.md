# M-03 · 实现 #3

## 实现范围与基线

仅新增 labels.ts、task-columns.ts、两份项目测试与真实 project.get JSON 夹具。上一审计 M-02 通过，没有退回缺陷，故无须逐字搬入审计缺陷检查。阶段、状态、类型与列名集中定义；Bug 使用共享合并阶段；未知节点显示原值，未知状态不入列。

上一轮运行基线 Web 330，本轮新增 13 个用例，预期运行总数 343（+13）；设计 M-03 契约的 329 是静态枚举数，沿用上一轮已说明的参数化展开差 14，不修改验收契约。desktop 未改，本轮 M-03 契约未要求复跑，上一轮 104（103 通过、1 skipped），不声称本轮通过。

## 真实夹具采集

命令：`bash .autome/output/m03-capture.sh "$PWD/.autome/output/m03-fixture3" "$PWD"`，退出码 0。脚本从 `sed '/# ===== 1-3:/,$d' apps/web/test/fixtures/w4-ledger.sh` 取得现有真实 CLI 夹具的初始化与辅助函数，再执行：

- `feature_steps waiting; A task new waiting; run` 到等待批准设计；`fakedir failed`，写入 `exit 3`，`A task new failed; run` 制造失败（retry attempts=0/backoff_seconds=0）。
- `fakedir stopped`，步骤写入 `sleep 120`，`A task new stopped; A run --once; A stop stopped`。
- 对 active、active-two、active-three 各建阻塞步骤 `sleep 120` 并 `A task new`，占满三个槽位；`A run --once; A task new queued; A task new draft --draft`。
- `A rpc project.get "{\"project_id\":\"$PID\"}" > "$WT/apps/web/test/fixtures/w5-project-get.json"`，逐字保存 stdout，未改写 JSON。采集后逐个 `A stop` 释放三个阻塞任务。

夹具包含 8 个任务：draft 1、queued 1、active 普通节点 3、active 等人节点 1、stopped 1、failed 1。核对式：running=3，needs_you=awaiting_user(1)+failed(1)+stopped(1)+paused(0)=3。之前两次采集暴露了短命假会话已退出的竞态；最终以阻塞步骤取证，未使用失败采集结果。

## 标待审前的自审清单

1. **情形表逐行**：task-columns 第 1 条检查设计的 14 种 JSON；draft/queued/queued.resume → todo；active 的 4 个等人节点 → needs_you，其余 18 节点（含三种门禁/验证节点）→ running；paused/stopped/failed → needs_you；done → done；cancelled → null。labels 第 2、3 条检查需求七段、Bug 五段、唯一覆盖及共享合并；task-columns 第 6 条另用独立预期表逐一验证所有阶段归属。
2. **失败分支**：未知节点（含空字符串、__proto__、toString）原样回退且 index=-1；未知/空 state 不入列；draft 和无 resume 的 queued 无阶段。由 labels 第 4 条、task-columns 第 3、6 条覆盖。授权、读盘、超时、外部服务不可达不适用：本里程碑生产函数为纯映射，不访问这些服务；源码读取失败则测试直接失败。
3. **全部迁移**：不适用内核状态迁移实现，本轮不实现状态机或通道；对显示映射的全部状态载体，task-columns 第 6 条穷举需求 15 节点与 Bug 13 节点 × active.node/paused.resume/queued.resume/stopped.at/failed.at，共 140 组，并覆盖 done、draft、queued、cancelled、未知状态；归列按全部 22 active 节点与其余状态检查。
4. **域边界**：空任务数组返回四列 0；单 stopped 返回 needs_you=1；未知/空字符串与无节点输入均验证。日期、数值上限、前导零不适用：输入域为状态字符串与任务数组，没有日期或数值解析。

## 负向对照与还原

运行 `node .autome/output/m03-negative.mjs`，四组都退出码 1：

- 移除需求实现段 gate：labels 第 2 条失败（另触发第 6 条）。
- retro 同时放入审计与合并：labels 第 2 条失败（另触发 stage 载体检查）。
- stopped 改入 running：task-columns 第 1、5 条失败（另触发单元素边界）。
- AWAITING_NODES 移除 not_a_bug：task-columns 第 1、2 条失败。

脚本用 finally 还原原文。原始日志在 `.autome/output/m03-negative-{gate,retro,stopped,not-a-bug}.txt`。首次定点测试还修正了测试解析器的 serde 大写字母规则及带字段变体的空格识别；没有修改内核。

## 验收结果

- `npm test --prefix apps/web -- test/labels.test.ts test/task-columns.test.ts`：退出码 0，13/13，skipped 0；日志 `.autome/output/m03-target-final.txt`。
- `npm test --prefix apps/web -- test/labels.test.ts test/task-columns.test.ts test/core-sidecar.test.ts`：退出码 0，31/31，skipped 0；日志 `.autome/output/m03-regression-final.txt`。首轮全量的旧源码守卫把断言对象的 todo: 列名误判为跳过测试选项，现使用计算属性写法；守卫文件与正则未改。
- `npm run lint --prefix apps/web`：退出码 0；`npm run typecheck --prefix apps/web`：退出码 0；`npm run build --prefix apps/web`：退出码 0。分别见 `m03-lint-final.txt`、`m03-typecheck-final.txt`、`m03-build.txt`（均在 .autome/output/）。
- 首轮 `npm test --prefix apps/web`：退出码 1，343 个（+13），312 通过、12 失败、19 未执行；其中 1 个为上述已修复的源码守卫误判，11 个为 gateway-http 监听 EPERM。eventbus-live 7、notifier-live 3、server 9 在环境初始化失败后未执行，不能称 skipped 0。完整日志 `.autome/output/m03-web.txt`。
- 最终 `npm test --prefix apps/web` 复跑：退出码 1，39 文件、343 用例（相对 330 基线 +13，全部来自本轮两份测试），313 通过、11 失败、19 未执行，耗时 267.87 秒。失败仅 gateway-http 的 11 个监听用例（listen EPERM）；eventbus-live 7、notifier-live 3、server 9 初始化受 EMFILE/EPERM 阻断。源码守卫恢复通过。日志 `.autome/output/m03-web-final.txt`。未达到全量 skipped=0，留给具备监听能力的审计环境复验，未削弱测试。

未改已有测试断言、未加 skip、未改内核/desktop/协议。纯映射验收通过；全量监听环境限制需审计轮复验。状态字段及五行里程碑表已核对，只有 M-03 从开放改为待审。

## 提交与清理

本轮改动已在当前分支提交；负向注入全部还原后执行 `git status --porcelain`，输出为空（`.autome/output/m03-status-after-commit.txt`，0 行）。未推送、未启动下一会话。本条取证记录随同一提交补入。
