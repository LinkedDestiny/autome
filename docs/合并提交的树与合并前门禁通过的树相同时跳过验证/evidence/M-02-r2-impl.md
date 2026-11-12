# M-02 · 实现 #2（2/15）

## 范围与实现

仅推进 M-02；M-01 保持审计已完成，M-03 不推进。上一轮审计通过且没有退回缺陷，因此「逐字搬入审计判别检查、先红后绿」不适用。

- `scheduler.rs`：MainCheck 在创建验证检出前判断开关、检出与作业均不存在，再调用只读 `main_check_reuse`。最近 MergeGate 必须通过；沿用记录必须追到真跑结果；核对第二父提交、结果之前最近一次启动的节点/SHA/命令，以及 Git 树 ID。成功只写四字段 `gate.reused`，直接进入 Cleanup；不满足则沿用原路径。
- `store.rs`：新增按主体、种类与严格 `seq < before_seq` 取最近事件的只读查询。
- 新增 4 个 scheduler 测试、3 个 e2e。R1 改验默认复用；R2–R4 共用晚提交夹具使 MainCheck 真跑，原报警、准备及清理断言保留。其它原有 e2e 函数未改；`run_gated_task` 委托到缺省行为相同的参数夹具。

## 旧行为冻结

先只改夹具与 E3，未修改 `run_gate`。第一次运行以空列表捕获真实事件（断言红，日志 `.autome/output/m02-freeze-capture.txt`），随后冻结 14 项事件字面量：两轮 Gate、第一轮 MergeGate、自动合并请求/rebase、第二轮 MergeGate、自动合并收尾/merge.repo、MainCheck 启动/结果。E3 真跑通过 1/1，脚本计数为 gate=2、merge_gate=2、main_check=1，共 5。

冻结提交：`2f9620000d392e7772d08d8eb367e14786d80210`（`test(gate): freeze main check events with reuse disabled`）。通过日志 `.autome/output/m02-freeze-pass.txt`，提交日志 `.autome/output/m02-freeze-commit.txt`。最终 E3 函数体与该提交逐字比较相同。此处捕获红是冻结基线的方法，不是上一轮审计缺陷。

## 验收命令与总数

| 命令 | 实际结果 | 相对上一轮基线 | 原始输出 |
|---|---|---|---|
| `cargo test -p automed --lib scheduler::` | 91 通过，0 失败/忽略；变异还原后再次 91 通过 | 87 → 91，+4 个设计点名测试 | `.autome/output/m02-scheduler-all.txt`、`m02-scheduler-final.txt` |
| `cargo test -p automed --test end_to_end` | 115 通过，0 失败/忽略 | 112 → 115，+E1/E2/E3；R1–R4 未删用例 | `.autome/output/m02-e2e-all.txt` |
| `cargo test -p automed --test end_to_end main_check_reuse_` | 恰好 3 个通过；E1 4 次调用，E3 5 次调用 | 新增 3 个，不包含其它用例 | `.autome/output/m02-e123.txt` |
| `cargo test --workspace -- --list` | 注册总数 1223 | 上一轮 1216 → 1223，+7；这是列举，不是全量实跑 | `.autome/output/m02-workspace-list.txt` |
| `cargo fmt --all -- --check`、`cargo clippy -p automed --all-targets -- -D warnings`、`git diff --check` | 通过 | 静态检查，不计入测试用例数 | `.autome/output/m02-fmt-check.txt`、`m02-clippy.txt` |

本里程碑实际全组执行 206 个用例（91+115），上一轮对应基线 199，净增 7；没有删减或忽略旧测试。全工作区执行与 `sh scripts/ci-local.sh` 属 M-03，本轮未执行，不能以列举结果代替通过证据。

## 标待审之前的自审清单

以下逐项对照已完成的测试，并在写入清单后再跑新增用例作为变异还原的最终正向复验；结果追加于文末。

### 1. 情形表逐行

`main_check_reuse_decision_table` 使用真实 Git 与内存账本，各行比较返回负载并验证账本没有新增事件：

| 设计情形 | 检查与结果 |
|---|---|
| S1 真跑 MergeGate 成功 | 复用，四字段与真实合并 SHA、父 SHA、树 ID 完全相同 |
| S2 MergeGate 沿用 Gate | 找到同 SHA 真跑结果及启动记录，复用 |
| S3 更早通过、最近失败 | 不复用，不倒退选旧通过 |
| S4 无 MergeGate 记录 | 不复用 |
| S5 启动命令不同 | 不复用 |
| S6 Bug 命令 | `( repro ) && ( gate )` 复用；只有 repro、另一 repro、缺少 bug.reproduced 均不复用 |
| S7a 门禁后分支晚提交 | 第二父提交不同，不复用；E2 另验真跑 MainCheck 与清理 |
| S7b main 在检查后前进 | 第二父提交相同但合并树不同，不复用；独立 run_gate 测试真跑脚本并断言两个节点各调用一次 |
| S7c 旧轮通过、空提交 main 后 rebase | 实测新旧 head 树相同但 SHA 不同，不复用 |
| S8 不存在的 from_sha | 不复用、不抛 Git 错误 |
| S9 沿用没有真跑依据 | 缺少同 SHA 结果或结果失败，均不复用 |
| S10 结果命令与实际启动不同 | 以启动命令判断，不复用；真实运行测试在 A 运行期间改为 B，验证结果记 B 但启动为 A，MainCheck 真跑 B 失败并报警 |
| S11 启动缺失或配对错误 | 缺启动、节点错、SHA 错均不复用；结果之后的启动及其它任务事件不能串入配对 |

首入判定测试分别运行 fresh、已有真实验证检出、已有作业记录、开关关闭四种情况：仅 fresh 复用且不创建检出/作业，其余继续收集真实路径结果；均进入 Cleanup。

E1/E2/E3 对照：E1 无 MainCheck started/result/setup 事件、恰好一条 reused、from_sha 等于 rebase head 和第二父提交、无报警/验证检出；E2 树不同真跑且 verify_record/检出删除；E3 字面量与旧实现一致且不出现复用。

### 2. 设计点名的失败分支

- 无通过记录、最近失败、沿用依据缺失/失败、命令不匹配/缺少复现、启动缺失/不匹配：上述 S3–S6、S9–S11 已逐项执行。
- Git 读取失败：不存在的 from_sha、缺失合并提交、普通提交、仓库不存在均返回 None；额外删除第二父提交的松散对象，保留合并记录对它的引用，验证取树失败仍返回 None。
- 账本读取失败：文件 SQLite 打开后移除 events 表，调用判定返回 Err，不把读取失败当作通过。
- MainCheck 门禁失败：R2 与运行中改命令测试均验证报警；R2 还验证 Done、仪表盘及关闭报警。验证检出 setup 失败：R4 保留报警、skipped、不启动门禁和清理断言。R3 保留准备先于门禁的时序断言。
- 拒绝授权、外部服务不可达：不适用，新增逻辑仅查询本地账本与 Git，没有授权或外部服务调用。
- 新增逻辑没有计时等待或超时状态；既有 setup 超时/作业生命周期测试随 scheduler/e2e 全组执行，未改变超时处理。

### 3. 全部状态迁移

| 路径 | 执行依据与结果 |
|---|---|
| MergeGate 通过 → 合并 → MainCheck 复用 → Cleanup → Done | E1、R1；首入与命令不变正对照直接断言 Cleanup |
| main 前进 → Rebase → MergeGate 重跑 → 合并 → MainCheck | E1/E3，核对 rebase head 与通过记录；两者分别复用/真跑 |
| MainCheck 不复用 → 作业运行 → 通过 → Cleanup → Done | E2/E3；首入已有检出/作业、关闭开关与 main 前进的 run_gate 测试 |
| MainCheck 不复用 → 失败 → 报警 → Cleanup → Done | R2；真实命令切换测试断言失败和 Cleanup |
| MainCheck 验证检出准备失败 → skipped/报警 → Done | R4，原断言保持 |
| 没配门禁或 workspace 无单一合并提交 | 分支仍位于新增逻辑之前，相关原有 Bug/workspace/setup 合并用例随 115 个 e2e 通过 |

不新增状态或通道。熔断的原有 `same_failure_*` 测试未修改，随全组通过；MainCheck 复用不参与其它节点计数。

### 4. 输入域边界

- 配置域为布尔：本轮 fresh/default 与显式 false 已真跑；true/false/缺省及非布尔解析由 M-01 已关闭测试负责，没有改动解析。
- 事件域：空记录、单条结果、多次通过后失败、无启动、缺少同 SHA 结果、其它任务事件、结果之后的启动均已检查；严格 seq 上界防止后来的启动污染历史配对。
- Git 输入域：空 SHA、不存在 SHA、普通非合并提交、相同树不同 SHA、相同第二父提交但不同树、缺失仓库、缺失父对象均已执行。决策表合计 24 个变体，另加账本故障检查。
- 日期/闰日/跨年/数值最大值：不适用，新增接口不解析日期或数量；SHA 不作数值解析，不存在前导零归一化。

## 负向对照与还原

执行 `.autome/output/m02-mutations.rb`，逐次修改一处生产逻辑、运行对应项目测试、要求出现真实测试失败（不接受编译失败），每次以原文重置，最后 ensure 还原。11 个变体全部退出 101 且命中断言失败：

- a：忽略开关 → E3 红；b：比较提交 SHA → E1 红。
- c：不比较树、错比第二父提交的树、忽略第二父提交绑定，三种分别被决策表拦住。
- d：提前建立验证检出 → E1 红。
- e：接受最近失败结果 → S3 红。
- f：沿用事件不追溯真跑 → 决策表红。
- g：Bug 不比较启动命令 → 决策表红。
- h：以 result.command 代替 started.command → 决策表红。
- i：无 started 时放行 → 决策表红。

摘要 `.autome/output/m02-mutations-summary.txt`，各次原始输出 `.autome/output/m02-mut-*.txt`。变异均未保留。还原后完整 scheduler 91/91、fmt、clippy 通过；E3 函数与冻结提交逐字相同。

## 清单写入后的最终复验与结论

`cargo test -p automed --lib main_check_reuse_ && cargo test -p automed --test end_to_end main_check_reuse_` 退出 0，分别 4/4、3/3 通过，0 失败/忽略。前者执行上述 24 个决策变体、账本故障、首入四路径、main 前进与命令切换正负路径；后者执行 E1–E3。日志 `.autome/output/m02-self-review-unit.txt`、`.autome/output/m02-self-review-e2e.txt`。此为还原后的重复复验，不增加用例总数。

四项自审完成，M-02 可标待审，交独立审计复验；本轮不关闭里程碑。无环境阻塞，未执行项仅为后续 M-03 的全工作区实跑与 ci-local。提交按本轮授权在当前分支执行，不推送，不启动下一会话。
