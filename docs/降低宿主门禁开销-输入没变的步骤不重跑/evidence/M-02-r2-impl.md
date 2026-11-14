# M-02 · 实现 #2

## 范围与实现

本轮只推进 M-02（B·步骤声明输入与按内容沿用，脚本侧）。上一轮审计 M-01 通过，无退回缺陷，因此没有需逐字迁入的审计回归检查。

- `steps.sh` 声明全局输入、免检输入、10 个检查步骤与 3 个准备步骤；`ci-local.sh` 保留入口和原有 package 三步。
- `lib.sh` 负责声明、执行、日志、报告与准备步骤；`inputs.mjs` 负责临时 index 快照、工具链/内容键、工作区外快照验证、计划和归属守卫。使用 Node 内置库，不新增依赖。
- 每一步执行通过后重新核对输入；变化或读取失败只报告 `partial`。失败步骤报告 `failed`，沿用报告 `reused`，准备步骤不报告。原有失败步骤汇总行逐字保持。
- 实际工具链与依赖锁内容纳入键；二进制/目标目录覆盖变量清除并记录。外部快照使用 realpath 检查，指向工作区内文件的外部符号链接也拒绝。
- README 与 technical-design §8.1/§16 同步。宿主导出、账本入账与时间线仍属 M-03，本轮未实现或声称验证这些行为。

## 命令与结果

原始输出均在本 worktree `.autome/output/`，不放进任务文档。

- 验收命令：`node --test scripts/test/ci-reuse.test.mjs scripts/test/ci-claims.test.mjs && (cd apps/web && npx vitest run test/ci-config.test.ts)`，退出 0。ci-reuse **18** 例、ci-claims **1** 例、ci-config **9** 例，合计 **28** 例全部通过、0 跳过。日志：`m02-final-scripts.txt`、`m02-final-web.txt`；脚本耗时 73.663 秒，Web 0.251 秒（仅本次环境口径）。
- 直接相关回归：`node --test scripts/test/ci-summary.test.mjs`，退出 0，**7** 例全部通过，0 跳过；日志 `m02-summary.txt`，耗时 2.009 秒。
- 本轮正式验证合计 **35** 例。与同范围上一轮基线 **16**（summary 7 + config 9）相比 **+19**：新增 reuse 18 + claims 1。若和上一轮 M-01 的整条验收 **22** 例相比是 **+13**，原因是新增 19、本轮未重复运行未改动的 gate_failure_window 6；不是丢失或条件跳过测试。相对设计最初预期本轮 28 的总数（12+1+9+7）多 **6**，来自下述用例 13–18 的状态/失败/边界补强。
- `sh -n scripts/ci-local.sh scripts/ci/lib.sh scripts/ci/steps.sh` 与 `git diff --check` 均退出 0。
- 下述自审清单中的全部适用项均已按对应测试执行并通过；完成清单后将 M-02 标为待审。

## 标「待审」之前的自审清单

### 1. 情形表逐行

逐行执行设计 M-02 契约，对应 `ci-reuse.test.mjs`：

| 情形 | 检查与判据 |
|---|---|
| ① 只改 Web | 精确断言只有 Web 四步执行，其余六步沿用 |
| ② 只改 Rust | 精确断言 Format、Clippy、Rust 测试、Desktop、Web test 执行；Contract 与 Web 静态三步沿用 |
| ③ 只改 docs | 0 步执行 |
| ④ 修改门禁 | 所有步骤执行 |
| ⑤ 顶层新增路径 | 所有步骤执行，归属检查退出 1 并列出 `newtop/x` |
| ⑥ 强制全跑 | `--all` 与 `CI_LOCAL_ALL=1` 分别检查 |
| ⑦ 工作区内快照 | 直接路径与工作区外符号链接均拒绝，日志说明原因 |
| ⑧ 旧宿主无变量 | 全部执行，无报告 |
| ⑨ 失败修复重跑 | 第一次报告 ran/failed；只修失败步骤输入后，第二次报告 reused/ran，日志包含来源与输入键 |
| ⑩ 工具链变化 | PATH 内 node 仅 `-v` 改变，全部失效 |
| ⑪ 构建准备 | Rust 沿用、Web 执行时先 Build core；全部沿用时不构建；Web 配置测试另断言真实步骤表声明 |
| ⑫ 二进制覆盖 | 四个覆盖变量全部清空并写日志，设与不设时键相同 |

B.1 输入逐步对照：Format=crates；Clippy/Rust=crates+protocol；Desktop=apps/desktop+crates+protocol；Contract=dispatch.rs；Scripts=空局部输入；Web lint/typecheck/build=apps/web；Web test=apps/web+crates+protocol+plugins+Desktop renderer；全部叠加全局输入。真实归属守卫检查仓库现有路径。跨目录已归属输入仍须维护者同步声明的限制已写入文件头与文档。

### 2. 失败分支

- 快照缺失、格式错误：用例 14，各自全跑；工作区内快照拒绝：用例 07。普通文件读取异常被捕获并输出不采信原因。
- 工作区快照读盘失败：用例 18 写坏临时仓库 index，退出 2，无计划行；不会产生通过证据。
- 检查命令失败：用例 09，报告 failed、门禁退出 1；修复后才记录 ran。
- 准备步骤失败：用例 17，后续检查仍执行，整体退出 1，报告不把准备步骤记为通过；目录存在时条件准备不执行。
- 检查执行期间输入变化：用例 13，命令成功但仅报告 partial，不写 ran。
- 拒绝授权、网络服务不可达、宿主超时：不适用，本里程碑没有授权/网络调用或宿主超时管理；命令非零退出路径由 09/17 覆盖，超时收报告属于 M-03。

### 3. 全部迁移

- 无通过 → 执行成功 → ran；无通过 → 执行失败 → failed：09。
- 匹配通过 → reused；输入变化 → 重新执行：01/02/04/05/09/10。
- 匹配通过 + 强制全跑 → 执行：06。
- 不可信/缺失/损坏/空快照 → 全跑：07/14/18。
- 执行成功 + 输入变更 → partial：13。
- 准备：后续有执行 → 运行；后续全沿用 → 不运行：11；条件目录存在 → 不运行，缺失且命令失败 → 整体失败但继续：17。
- 相同输入、提交身份变化 → 键不变：16；真实 index 不被快照写入污染：16。
- 只计划 → 不执行、不写报告：14；归属守卫 → 无遗漏退出 0、有遗漏退出 1：claims/05。

### 4. 输入域边界

- 空局部输入：Scripts 与用例 12；空快照：18；单文件输入、删除：15；未跟踪文件：01/02/05；未暂存修改与不污染 index：16。
- 含空格/中文文件名、目录段精确匹配（`stable` 不包含 `stable-other`）、忽略的 node_modules 文件：15。
- 步骤名含 Tab 拒绝：18；缺失输入路径按 absent 参与键，新增/删除会改变键：01/15。
- 日期、闰日、跨年、前导零、数值最大值：不适用；本里程碑不解析日期/数值域，来源时间仅原文显示。

## 负向对照

在当前工作区逐项短暂注入缺陷，测试结束后 finally 恢复文件，全部以断言失败（非语法错误）退出 1：

1. Web test 漏掉 crates → 用例 02 红。
2. 未归属哈希固定为空 → 用例 05 红。
3. 放开工作区内快照 → 用例 07 红。
4. 保留四个覆盖环境变量 → 用例 12 红。
5. 真实 Build core 从 prep 改成可沿用 step → ci-config 的构建声明断言红。

命令与完整输出：`.autome/output/m02-negative.mjs`、`m02-negative.txt`、`m02-negative-*.txt`。这些是本轮自审的负向对照，不是上一轮审计退回；最终原始实现已还原并重新验收。

## 开销与边界

夹具同一组检查第一次执行 Stable 与 Changing 两步；只修 Changing 后只执行 Changing，Stable 沿用；全部键匹配时 Build core 不执行。真实步骤表计划验证 docs-only 为 0/10 执行、Web-only 为 4/10 执行、Rust-only 为 5/10 执行。此为决策/执行证据，不冒充真实完整门禁墙钟节省数据。完整门禁与空闲/负载基准依用户边界留给宿主/后续里程碑。

未运行 `sh scripts/ci-local.sh` 全量门禁、cargo fmt/clippy、Web lint/typecheck；用户明确由宿主统一执行快检，本轮只运行 M-02 验收与直接相关的汇总测试。没有修改 `.autome/` 配置、规则或启动脚本，只写规定的 output 原始输出/临时验证产物；没有改账本、切分支、建 worktree 或推送。
