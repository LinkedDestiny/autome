# M-04 实现轮 #5

结论：待审。已实现门禁用例清单、既往失败记录提取与首次修复简报注入。真实进程验收的 `ps EPERM` 环境阻断按任务约束 5 交审计复验；不删除断言、不增加跳过，不将受阻项记成通过。

## 实现与修复

- `e2e-catalog.mjs` 用真实 Playwright `--list --reporter=json` 获取全部用例，不运行 globalSetup；输出闭合 cases 段和准备步骤词（含 harness 及引用的 fixture 文件名）。缺依赖、子进程失败、30 秒超时均打印未取得清单并退出 0。macOS 临时目录符号链接曾使入口比较不相等，已改按 realpath 比较，缺依赖断言复验通过。
- 快检在 Web install 之后、相关测试准入预算之前收集；沿用 Web 静态步骤时仍收集。报告器输出本次选择的清单，并保持原 digest 日志调用与首行原文路径契约。
- `gate.result.cases` 只存日志末 256 KiB 内完整闭合段的有序并集，上限 2000；简报用本任务最近带 cases 的结果。旧日志没有清单时按通用 spec/test 文件名式匹配，并明确说明。
- 只读取设计 Backlog 与 `*-audit.md`；列表续行、表格行、空行分隔段落按记录单元匹配名字与失败词。编号两侧加边界，B-3/B-31/BL-13 与 C-2/C-2b 不串。没有编号用完整标题。
- 简报小节位于本里程碑之后，摘要至多 300 字节且围绕失败词，最多 60 条/12 KiB；完整记录写入 `.autome/output/gate-history/<任务>-<轮>.md`，保留源路径和起始行；无命中与读写失败都有明确提示。
- 触发条件是需求实现轮 + GateFailed 注入 + 最新 gate.result.node 为 merge_gate。两条端到端验证事件存储、真实简报、完整提取文件及后续快检失败不再注入；不改变 Bug Loop、调度或车道。
- T-98 夹具按设计点名的 10 个源行加 BL-13 原文保存；测试核实原按行规则恰有 8 条命中，完整记录提取全部包含这 8 条。
- 上轮审计 M-03 通过，无退回缺陷检查需要逐字迁入；本轮主动负向对照另记如下。

## 命令、数量与结果

基线 HEAD：`3253d4db`。本轮主验收 **65 条**，相同命令范围的上一轮基线 **51 条**，差 **+14**：history +9、prompt +1、端到端 +2、catalog +2。主验收 63 通过、2 因 ps 权限受阻，0 跳过。

| 命令 | 本轮/基线 | 结果 | 输出 |
|---|---:|---|---|
| `cargo test -p automed --lib gate_history` | 9/0 | 全部通过 | `.autome/output/m04-history-final.txt` |
| `cargo test -p automed --test prompt_rendering` | 48/47 | 全部通过 | `.autome/output/m04-prompt.txt` |
| `cargo test -p automed --test end_to_end gate_history` | 2/0 | 全部通过 | `.autome/output/m04-e2e-final.txt` |
| `(cd apps/web && npx vitest run test/e2e-catalog.test.ts test/test-groups.test.ts)` | 6/4 | 分组 4 通过，catalog 2 的内容断言通过但后置 ps 检查受阻 | `.autome/output/m04-web-acceptance.txt` |

去重后本轮所跑测试共 **1189 条**，相同范围上一轮基线 **1175 条**，差 **+14**（来源同上）；1180 通过、9 环境受阻、0 跳过。筛选重跑和负向变异不重复计数。

直接相关补充检查：

- `cargo test -p automed --lib`：1038/1029，全部通过，包含上表 history 9；`.autome/output/m04-lib.txt`。
- `(cd apps/web && npx vitest run test/gate-reporter.test.ts)`：20/20 全过；`.autome/output/m04-web-recheck.txt`。
- `(cd apps/web && npx vitest run test/gate-reporter-run.test.ts)`：7/7 均在 ps 扫描受阻；`.autome/output/m04-real-final.txt`。清单/状态断言与完整进程清理验收分开，不称整体通过。
- `node --test scripts/test/*.test.mjs`：68/68 全过，`.autome/output/m04-scripts.txt`。补强已有安装顺序与沿用测试后再次运行 `node --test scripts/test/ci-quick.test.mjs`：15/15 全过，`.autome/output/m04-quick-final.txt`，不新增用例数。
- Rust 格式检查、`cargo clippy -p automed --lib -- -D warnings`、本轮 TS/MJS 文件 eslint 和独立严格 tsc（含 allowJs/checkJs）通过；日志 `m04-fmt-final.txt`、`m04-clippy.txt`、`m04-eslint-final.txt`、`m04-types.txt` 均在 `.autome/output/`。`git diff --check` 通过。
- 未运行项目快检门禁或 `sh scripts/ci-local.sh` 全量门禁；脚本测试只在临时夹具仓库执行替身命令。

## 标待审之前的四项自审

1. **情形表逐行**：B-3 清单闭合/多段并集/清单缺失、文件名/编号/无编号标题/准备词、Backlog/审计/实现证据排除、列表续行/表格/段落、有匹配/无匹配、短摘要/完整提取、首轮修复/后续快检修复均有通过断言。真实 list 数量等于 Playwright 总数且准备词齐全；缺 node_modules 明示失败但退出 0。两项后置 ps 检查保留并交审计。
2. **失败分支**：设计文件读盘失败由删除设计文件后 collect 返回错误验证；没有清单退回文件名式、缺 node_modules 失败不阻断命令均已执行。报告器真实 setup 失败/超时清单逻辑执行后，残留复查受 ps 拒绝，逐条见下。授权/外部服务不可达不适用：此功能不新增授权或外部服务。没有把操作系统拒绝当成测试成功。
3. **全部迁移**：MergeGate 失败→Implement 有记录；同次修复后 Gate 快检失败→Implement 无记录，两条端到端已通过。未注入 GateFailed 的既有提示词 golden 保持不变；Bug 不走需求简报入口，全库测试通过。清单无→有、多段→去重限长、无命中→明确空结果均已跑；无新增状态机。
4. **输入域边界**：0/单条/多段/未闭合/无效空字段/2001 条限为 2000；编号 B-3/B-31/BL-13、C-2/C-2b 以及紧邻字符；60/61 条、中文 300 字节、12 KiB 截断、末条完整原因不丢均通过。日期、闰日、跨年不适用：没有日期输入。前导零编号按原字面匹配，不做数字归一化。

## 负向对照与恢复

- 同时去掉编号边界、把读取范围改为所有 `.md`，9 条中边界与仅审计两条按预期失败：`.autome/output/m04-negative.txt`。恢复源码后 history 9 全绿。
- 把 merge_gate 条件放宽为任何最近结果，后续快检修复端到端按预期失败：`.autome/output/m04-negative-trigger.txt`。恢复源码后端到端 2 全绿。
- 所有变异已还原。以上是本轮主动判别检查，不是上一轮审计退回项。

## 交审计的环境受阻项

完整命令：`(cd apps/web && npx vitest run test/e2e-catalog.test.ts test/gate-reporter-run.test.ts)`。

均报 `spawnSync ps EPERM`，共 9 条，未跳过：

- 真实 list 的清单数量等于 Playwright 总数并包含准备步骤
- 缺 node_modules 明示未取得清单并退出零
- 真实运行包含 setup/teardown
- 快检过余量线改写真实退出码
- 合并前门禁过余量线退出零并保留真实样本
- 真实 poll 两秒预算一点五秒过半
- 预算截断后回收登记的服务及临时根
- 真实总超时三类清单与未执行用例
- globalSetup 无 onBegin 时用 list 找回全部选择的用例
