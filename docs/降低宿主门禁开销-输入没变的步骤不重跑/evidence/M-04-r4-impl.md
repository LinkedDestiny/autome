# M-04 · 实现 #4 · 快检相关测试选择与预算

结论：验收通过，报 `待审`；实现轮 `4/30`，只推进 M-04。上一轮 M-03 审计为通过，无审计退回缺陷，因此没有需要逐字搬入的审计复现检查。

## 实现与范围

- 新增 `scripts/ci-quick.sh`，加载完整的 `steps.sh`，通过 `inputs.mjs` 选择四个静态整步；键计算仍使用完整声明表，因此与完整门禁一致。
- `quick.mjs` 生成改动候选：集成目标、Web related、Desktop 全集、按 cargo metadata 类型选择的 crate 单元目标、protocol、fixtures、domain 的依赖方。提交后改动、未提交改动、未跟踪文件均纳入；按优先级排序并去重，rename 的两端均算改动。
- end_to_end 同时核对旧、新文件的 diff 范围，屏蔽 Rust 字符串/注释后映射测试函数体；辅助函数、函数签名、整函数删除及无法确认的改动明确未跑；diff 零行侧按前一行锚点映射，函数体首行插入/删除仍可选中。采用现有 Node 依赖而非另加 awk 解析管线，设计 C.3 同步此落法。
- lib.sh 在相关阶段每个候选开始前检查预算，已启动命令不中断；超额写汇总和 `not_run`。成功子集仅 `partial`，失败为 `failed`；Desktop 是整步，可 `ran/reused`，沿用时不做准备构建。准备步骤复用原步骤表，Web 以 apps/web 为 cwd 加载原配置。
- 同步技术设计 §8.1 快检建议；没有改内核、Web 并行配置或 `.autome/` 配置，M-05/M-06 保持开放。

## 验收命令与结果

| 命令 | 用例总数 / 结果 | 原始输出 |
|---|---|---|
| `node --test scripts/test/ci-quick.test.mjs` | 13，全部通过，退出 0；包含真实 CLI bins、domain lib、automed lib 三条 `--no-run` 编译检查 | `.autome/output/m04-accepted-quick.txt` |
| `node --test scripts/test/ci-summary.test.mjs scripts/test/ci-reuse.test.mjs scripts/test/ci-claims.test.mjs` | 26，全部通过，退出 0（7 + 18 + 1） | `.autome/output/m04-regression.txt` |
| `(cd apps/web && npx vitest run test/ci-config.test.ts)` | 9，全部通过，退出 0 | `.autome/output/m04-ci-config.txt` |
| `sh -n scripts/ci-quick.sh scripts/ci/lib.sh`、`node --check scripts/ci/quick.mjs`、`node --check scripts/ci/inputs.mjs`、`git diff --check` | 非用例检查，退出 0 | 简短输出，无原始产物 |

**用例总数与基线：** 本轮最终验收及直接相关回归共 **48 个不同用例，48 通过、0 失败、0 跳过**。同范围上一轮代码基线 35（summary 7 + reuse 18 + claims 1 + ci-config 9），净增 **13**，全部来自新 ci-quick 文件；其中设计核心 10 个，额外 3 个覆盖候选边界、参数/报告边界、整步键一致与沿用。与上一实现轮实际执行的 M-03 子集 20（6 + 2 + 12）相比，本轮执行总数 **+28**：换成原有脚本/Web 配置子集 35，再加入快检 13；不是 M-03 测试被删除。重复运行、下列负向实验及三次编译不重复加到 48。

开发过程：首轮 13 个中 11 通过、2 失败（测试 03 漏写 npx 的 vitest 参数；测试 09 误读 Vitest 5 resolveConfig 返回结构），见 `m04-quick.txt`。修正参数断言后的 03/09 子集 2 个仍有 09 失败，见 `m04-config.txt`；查本地类型声明与返回值后改为读取 `.test`，13 个全绿见 `m04-quick-green.txt`。补充 raw string 伪函数、报告路径不可写等边界后 08/12 两个用例全绿见 `m04-boundaries.txt`。最终完整重跑也覆盖 diff 零行侧锚点（首语句插入/删除）的新增断言，以及 Desktop 沿用无需构建、Cargo.toml 独立触发、删除 end_to_end 与最大合法预算的补充断言（后三项也先跑 08/11/12，3 个全绿见 `m04-final-boundaries.txt`）。

真实命令发现并修复的问题：`npx vitest related --run -- test/ci-config.test.ts` 在本地 Vitest 5.1.0 返回 0，但输出 No changed files found，实际零用例（`m04-related-real.txt`）；去掉 `--` 才得到 9/9（`m04-related-without-separator.txt`）。候选已去掉分隔符，前导短横路径以 `./` 保护；09 增加执行真实候选并断言 9 个通过。回注旧分隔符后该项目测试退出 1（`m04-related-regression-red.txt`），还原后最终 13 个通过；同一组 ci-config 的 9 个用例不重复计数。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 C.2 行 | 检查 | 结果 |
|---|---|---|
| 集成测试 `.rs` | 01：CLI cli/new；未触碰 automed 不出现；已提交、未提交、未跟踪均纳入 | 通过 |
| end_to_end | 08：两个函数体按 first/second 选中；helper、签名、整个函数删除、raw string 内伪函数均未跑整套；函数体首条语句纯插入/删除仍选中该测试 | 通过 |
| Web 源码/测试 | 03：一条 related，source/test 路径去前缀；空格/单引号/中文原样传参；09：真实 cwd 下配置 timeout/hookTimeout=60000、fileParallelism=false | 通过 |
| Desktop | 04：Build core 输出确实存在且先于 Desktop，报告 ran；13：整步沿用且不构建 | 通过 |
| crate src/build/Cargo 归属 | 02、10、11：automed lib、CLI bins、build.rs 与 source 去重；metadata 目标实际编译 | 通过；单独改 Cargo.toml 的实际计划也选中 automed lib |
| protocol | 11：automed lib protocol + prompt_rendering | 通过 |
| 非 rs fixture | 11：所属 crate --tests | 通过 |
| domain 依赖方 | 10：domain lib + automed lib + CLI bins，三条均实际 --no-run 退出 0 | 通过 |
| 静态步骤、起点回退、无相关改动 | 07：无起点仍跑四步，base / --since 回退及 round 优先；11：docs 不选测试；13：全门禁键相同 | 通过 |
| 预算和子集报告 | 04/05/06/12：partial、failed、not_run；第二候选不启动；零预算无候选执行 | 通过 |

### 2. 失败分支

- 相关测试失败：06 注入 cargo 失败输出，退出 1，汇总保留 changed_case，报告 failed，未发布通过。
- 预算耗尽：05 第一条耗时 2 s、预算 1 s，第二条未启动并报告 not_run；12 零预算全部未跑。执行中的候选不打断，符合 C.3。
- 缺失起点/空改动：07 缺失三种起点时仍跑静态并说明未跑；11 docs-only 无候选。
- 无效输入或读/写失败：12 无效 revision、非法预算、缺少 --since 值、未知参数、报告路径为目录、路径含 Tab 均非零退出，不伪造通过。端到端文件删除/无法读取通过保守未跑分支处理。
- 准备失败与后续执行：原 `ci-reuse` 17 保持通过；报告读取失败、快照失败原 14/18 保持通过。
- 拒绝授权、外部服务不可达：不适用，本里程碑没有授权或远程业务服务分支；依赖安装仍交给原 prep 执行器，其非零结果由同一失败分支处理。
- 宿主硬超时：不适用本轮脚本验收，脚本不终止已启动候选，内核 timeout 不在 M-04 修改范围；本轮用慢命令实际验证不打断。

### 3. 全部迁移

- 候选待执行 → 成功 partial（04）/失败 failed 且整次退出 1（06）/超预算 not_run（05、12）。
- 无起点或不可细分 end_to_end → 明确 not_run（07、08）；无相关路径 → 无测试候选（11）。
- 整步待执行 → ran（04、07）；命中快照 → reused（13）；失败 → failed、输入执行中改变 → partial、准备失败 → 总体失败由原 reuse 09/13/17 验证；完整步骤既有状态没有另写一套。
- --plan → 仅打印，不执行/不写报告（12）；运行 → 逐条写报告（04–07、12、13）。

### 4. 输入域边界

- 预算：默认 120、1、0、000；最大安全整数接受；空串、负数、小数、非数字、超安全整数拒绝（05/12）。
- 改动集：无测试改动、单文件、多文件、同目标重复、未跟踪/已提交/删除、空格/单引号/中文、Tab 拒绝（01/03/08/11/12）。
- 起点：缺失、有效 round/base/--since、冲突时 round 优先、无效 revision（07/12）。
- Rust 目标：只有 bin、lib、依赖方；测试函数体边界、函数签名、删除与字符串中伪函数（08/10）。日期、闰日、跨年不适用：此阶段输入域没有日期。

## 负向对照

`node .autome/output/m04-negative.mjs` 退出 0，表示六个注入分别使项目测试退出 **1**，均检查到断言失败，非语法或环境失败。另有上述 Vitest 分隔符回归红绿，合计 7 个缺陷注入实验。脚本用 finally 恢复每次注入；最终验收在恢复后执行。

| 注入 | 对应验收 | 红证据 |
|---|---|---|
| partial 改成 ran | 04 | `m04-negative-partial.txt` |
| 候选开始前预算条件反转（会错误启动/跳过候选） | 05 | `m04-negative-budget.txt` |
| 给改动集加入未修改 automed 文件 | 01 | `m04-negative-scope.txt` |
| helper 改动时改跑整套 end_to_end | 08 | `m04-negative-whole-e2e.txt` |
| Web 改为 npm exec --prefix，失去 cwd | 09 | `m04-negative-web-cwd.txt` |
| CLI 生成 --lib | 10 | `m04-negative-cli-lib.txt` |

以上原始文件均位于 `.autome/output/`，不放任务目录。

## 执行限制与偏差

- 未运行完整门禁 `sh scripts/ci-local.sh`，未跑全量 Rust/Web 测试；真实内核 e2e 与宿主导出起点留在 M-05。
- 本轮按用户最新要求不安排 fmt/clippy/lint/typecheck 门禁，交宿主。编辑文档时曾因命令引号错误触发 shell 命令替换，误启动一次新快检入口；输出被展开到编辑表达式，随后编辑命令语法失败，未保存门禁输出，**不以该次误运行声称任何门禁通过**。已改用不展开的 heredoc；最终文档由该方式写入。尝试检查进程时 ps 被沙箱拒绝，未据此作残留结论。
- 只修改当前 worktree，无分支切换、远端推送或下一会话启动。配置文件与固定协议均未改。
