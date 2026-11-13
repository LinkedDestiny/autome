# M-03 · 实现 #3 · 待审证据

## 范围与实现

- 对应任务 A-R1/A-R2/A-R3/A-R4/A-V5/A-V6，设计 §A「转换」与 M-03 验收契约。上一轮审计 M-02 通过，未退回缺陷，因此无须搬入审计判别检查。
- `BugDoc` 新增可选缺陷声明及二级节正文提取，拒绝未知值/重复键，旧序列化数据缺字段按缺省读取。
- `BugProgress` 新增任务级修测试次数与打开标志；常量上限 2。修复/打回复核声明后直接定位，不设置修复预算、不启动修复轮；达到上限给出 `TestFixLimit` 与 CLI 恢复提示。
- `ReproDone.paired` 与纯函数 `judge_test_fix`：基线失败且 HEAD 绿进 FixGate；基线失败且 HEAD 不绿回正常修复预算；基线不复现沿用第一次定位、第二次等信息规则，不关闭修测试模式。打开模式而缺少成对结果明确协议失败，不能当普通复现放行。
- 域通过 `Transition.test_fix_event` 携带 Requested/Settled 副作用，保持 `Action` 的启动动作不变；字段缺省/空值省略兼容旧数据。M-05 将此意图与状态一起落账并从事件恢复进度。本轮 automed 构造点只补禁用缺省值，不提前启用协议能力。
- 四项缺陷说明内容完整性、实际成对执行、快照与落账属于 M-05；协议标记与 prompt 属于 M-04，本轮未修改。

## 命令、总数与基线

原始输出均在 `.autome/output/`，这里仅列结论。

| 命令 | 基线 → 本轮 | 结果与输出 |
|---|---|---|
| `cargo test -p autome-domain test_fix_` | 0 → 9，+9 | 9 通过，0 失败/忽略；`m03-build.txt` 为新增测试前 0，最终 `m03-accept-final.txt` |
| `cargo test -p autome-domain` | 478 → 487，+9 | 原 468 库 + 4/5 集成 + 1 ignored；现 477 库 + 4/5 集成 + 1 ignored，即 486 通过、0 失败、1 原有忽略；`m03-baseline.txt` / `m03-domain.txt`。既有用例未删除或改为忽略 |
| `cargo test -p autome-cli test_fix_` | 0 → 1，+1 | 新增上限提示和下一步命令用例通过；`m03-cli.txt` |
| `cargo test -p automed --lib handoff_` | 3 → 3，+0 | 原库 3 用例通过，覆盖新增域字段对调度器调用点的编译兼容；`m03-compat.txt` |

本轮不同用例合计 491（487 域 + 1 CLI + 3 automed），实际通过 490，原有忽略 1；同口径改前 481，增加 10，全部来自域 9 与 CLI 1。验收子集与域全量中的相同用例不重复计数。上轮 M-02 审计范围为域 `scope_patch` 13、automed `handoff_` 库 3 + e2e 1；不能将不同筛选范围直接相减，本轮另实测域改前基线。未重跑 M-02 e2e，不将它计入本轮通过数。

执行了 `cargo fmt --all` 格式化与 `git diff --check`；没有运行宿主快检组合、clippy 或 `scripts/ci-local.sh`。没有启动真实 autome/automed 服务、没有接触真实账本。

## 标待审之前的自审清单

### 1. 情形表逐行

本里程碑的情形表为设计 §A 转换列表，逐行对应：

| 情形 | 检查与结果 |
|---|---|
| 修复声明有缺陷且次数未满 | `test_fix_fix_declaring_a_defect_starts_locate_without_a_fix_round`：0/1 次直接定位，动作不是 Fix，budget 不变，原修复计数即使耗尽仍可修测试；通过 |
| 打回复核声明有缺陷 | `test_fix_review_declaring_a_defect_starts_locate`：定位与 Requested.by=FixReview；通过 |
| 次数达到上限 | `test_fix_limit_reached_fails_with_a_readable_reason`：2/3/u32::MAX 在 Fix、FixReview 都失败，无新事件或会话；通过 |
| 旧协议声明 | `test_fix_old_protocol_treats_the_key_as_unparseable`：Locate/Fix/FixReview 与 Unparseable 转换逐值相等；通过 |
| 修测试定位完成 | `test_fix_settled_results_route_to_fix_gate_fix_or_the_miss_rule`：进入 ReproCheck；通过 |
| Accept | 同上：Settled(Accept) + FixGate，门禁通过进 FixReview；通过 |
| NeedsFix | 同上：Settled(NeedsFix) + Fix/GateFailed，正常预算与耗尽失败；通过 |
| NotReproduced | 同上：第一次定位携带基线退出码及原因、第二次 AwaitInfo，不发 Settled，模式不关闭；通过 |

### 2. 失败分支

- 未知/空缺陷值、重复键：解析测试拒绝并点名字段；正文与围栏中的键不误识别。缺节/空节保留 None/空串交给 M-05 守卫，而非擅自吞掉。
- 错误声明轮次、复核通过却声明缺陷、角色关闭：定向测试分别拒绝为 Protocol/Config，不发 Requested。
- 上限、修复预算耗尽、打开修测试模式却缺成对结果：定向断言失败且无错误放行。
- 复现超时、退出码 0、126/127：分别核对 timed_out/exited_zero/not_runnable；HEAD 超时即使退出 0 仍 NeedsFix。
- 读盘失败、外部服务不可达、授权 I/O：不适用，本轮纯域不读文件/不访问外部服务；实际执行失败由 M-05 测试。

### 3. 全部迁移

- 新路径 Fix/FixReview → Locate；Locate → ReproCheck；ReproCheck → FixGate/Fix/Locate/AwaitInfo；FixGate → FixReview 均在上述定向用例实际执行。
- 新拒绝路径 Fix/FixReview → Failed(TestFixLimit)、错误结论 → Failed(Protocol)、定位角色关闭 → Failed(Config)、NeedsFix 预算耗尽 → Failed(FixBudget)、缺 paired → Failed(Protocol) 均有断言。
- 旧协议开关关闭或模式关闭时，即使提供 paired 仍按旧 ReproDone 判定；未声明键的既有 468 个域库用例全数保留并通过。
- 状态事件仅输出意图，不在纯域修改传入进度；Requested attempt 为 used+1，Accepted/NeedsFix 才发 Settled，NotReproduced 不发，守住模式开启的事件推导约定。

### 4. 输入域边界

- 次数 0/1/2/3/u32::MAX，修复预算未满/已满，声明缺失/空值/未知值/重复值，中英文冒号、正文/代码围栏、空节/缺节/节截止均执行。
- 成对退出码：基线 i32::MIN/-1/0/1/126/127/i32::MAX × HEAD i32::MIN/0/1/126/127/i32::MAX × 两侧超时布尔，共 168 组，三种判定与设计一致。
- 旧 BugDoc 与 ReproDone 序列化缺新增字段可读取。原 fix-round 前导零、最大值、溢出边界由域原有测试继续通过。
- 闰日、跨年不适用：输入无日期字段。

## 负向对照

依次临时改动产品实现，运行对应原样测试，均退出 101（单用例 0 通过/1 失败），随后恢复原代码；最终同一验收命令 9 个全绿。

| 临时缺陷 | 原样判别命令 | 红证据 |
|---|---|---|
| `>= TEST_FIX_LIMIT` 改成 `>` | `cargo test -p autome-domain test_fix_limit_reached_fails_with_a_readable_reason` | `m03-negative-limit.txt` |
| 声明分支改走 `start_fix` | `cargo test -p autome-domain test_fix_fix_declaring_a_defect_starts_locate_without_a_fix_round` | `m03-negative-route.txt` |
| 禁用旧协议声明检查 | `cargo test -p autome-domain test_fix_old_protocol_treats_the_key_as_unparseable` | `m03-negative-legacy.txt` |

本轮为需求域实现，不是退回缺陷轮；上述红证据是设计规定的缺陷注入对照，不冒充上轮审计缺陷复现。
