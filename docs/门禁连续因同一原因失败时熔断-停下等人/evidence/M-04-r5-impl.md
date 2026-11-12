# M-04 · 实现轮 #5

## 范围与修复说明

推进最小开放里程碑 M-04。上一轮 M-03 审计结论为通过，没有审计退回缺陷或须逐字搬入的判别检查。本轮只在现有 `end_to_end.rs` 增加 3 个用例与共用夹具，使用真实 Git、worktree、包装脚本、后台门禁及账本；只替换模型。没有修改已有测试或产品代码。

脚本和计数文件在测试仓库外；失败输出每次改变绝对路径、端口、耗时和时间，计数条件使用默认上限 3。所有原始输出与临时负向对照脚本位于 `.autome/output/m04-*`。

## 验收命令与结果

- 基线：`cargo test -p automed --test end_to_end -- --list`：80 个用例，与上一轮 M-03 审计的 80 一致；日志 `m04-baseline.txt`。
- `cargo test -p automed --test end_to_end same_failure`：3 通过，0 失败、0 忽略，80 filtered out；日志 `m04-focused.txt`。
- `cargo test -p automed --test end_to_end`：83 通过，0 失败、0 忽略；日志 `m04-e2e.txt`。总数 80 → 83，差 +3 完全来自本轮三个新增 e2e，没有删改旧用例。
- 负向对照逐一临时修改生产分支，各运行相应单用例：`cargo test -p automed --test end_to_end same_failure_three_times` / `same_failure_alternating` / `same_failure_rerun`；三次均 1 失败、82 filtered out，退出 101，均是运行期断言失败而非编译错误。日志 `m04-negative-{normalization,fingerprint,discard}.txt`。
- 对照分别取消关键行归一化、忽略指纹相等条件、把熔断后 rerun 的 `DiscardGateRuns` 换成 `RunCoreStep`。归一化对照得到 protocol 而非 gate_same_failure；后两项均无法到达 await_merge。每次运行后立即还原源文件；最终再跑定向 3 个全部通过，日志 `m04-restored.txt`。
- `cargo fmt --all -- --check`、`git diff --check` 通过；`git diff --numstat -- crates/automed/tests/end_to_end.rs` 删除行数 0。既有门禁用例随 83 个全量测试通过。

## 标待审之前的自审清单

### 1. 情形表逐行

| M-04 情形 | 实际检查与结果 |
|---|---|
| 连续同错三次 | 三条 gate.result 的计数依次 1/2/3，指纹全相等，只有第三条 tripped=true；failed 原因为 gate_same_failure、count=3、summary 含 ws，log 等于末次事件路径且文件存在；通过。 |
| 熔断后不再实现 | 账本 list_sessions 的 impl 恰好 3；再次 settle 后全部会话总数与结果条数不增加；通过。 |
| 交替错误 | 四条失败事件指纹相邻不同、same_failures 均为 1、tripped 均 false；第 2–5 个实现 prompt 含上一次完整 tail；第五次通过且无 fingerprint；5 个实现会话，最终 await_merge；通过。 |
| 修环境后 rerun | 仓库外置 environment-fixed，经 task.rerun_from(gate) 重跑，第四条 gate.result 通过、sha 与失败时相同，无 fingerprint；实现会话仍为 3，最终 await_merge；通过。 |

### 2. 失败分支

实际执行脚本退出 1 的重复失败与交替失败两条路径；未达阈值照常退回实现且保留日志注入，达到阈值转失败。三条负向对照分别证明归一化、同错判定、旧作业丢弃缺失会被用例拦下。

拒绝授权、读盘失败、超时、空输入、外部服务不可达：本里程碑不适用；M-04 的三个验收情形不涉及这些输入，门禁用本地 shell 模拟缺依赖，不依赖外部服务。配置边界、超时指纹及其它节点状态机属于此前里程碑，不冒称本轮重新专项取证。

### 3. 全部迁移

本里程碑覆盖的迁移均实际执行：Gate 失败且未达上限 → Implement（同错的前两次与交替的四次）；Gate 同错达上限 → Failed；Failed 再 tick → 保持失败且无新会话；Failed + rerun(gate) → Gate → 通过后 Audit / Retro / Rebase / MergeGate → AwaitMerge；第五轮修好后 Gate 通过同样走到 AwaitMerge。用最终节点、事件结果和会话记录共同验证。Bug、extend、其它节点拒绝等迁移由 M-02/M-03 验收，不属于 M-04 三个 e2e 的范围。

### 4. 输入域边界

实际覆盖默认上限 3 的第 1、2、3 次（阈值前与恰到阈值）、首次失败计数 1、错误交替始终回到 1、通过结果无计数字段、同 head 用户重跑。路径/端口/耗时/时间每次不同但同错归一化相同；ws/zod 不同错误保持不同。前导零、闰日、跨年、空串、最大值：本里程碑不适用，M-04 没有日期或数值解析输入域；1–10 配置域属于 M-03。

## 交付边界

M-04 仅标待审，由审计轮独立关闭；状态 implementation-round 为 5/25。M-05 的读者、文档和 workspace/clippy 全量门禁仍留待后续实现轮。宿主 `sh scripts/ci-local.sh` 按任务要求由 Autome 调度，本轮未启动下一会话。
