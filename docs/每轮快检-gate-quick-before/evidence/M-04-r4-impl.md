# M-04 · 实现 #4 · 验收证据

范围：设计 §8、§9、§13。上一轮审计 M-03 通过，没有审计发现的实现缺陷或待搬入的回归检查。本轮只推进 M-04，不修改任务协议副本、配置或全量门禁脚本。

## 实现与修复说明

- PromptSpec 传入当前生效的快检命令与最后一次 gate.result；需求实现 / Bug 修复轮追加快检句，其它角色保持原样。
- 简报按实际 kind 渲染快检、失败与超时；过期 head 不引用。当前切成 each_round 时只改说明文字，不改命令数据。
- 失败注入区分快检与完整门禁；保留原命令与日志尾部，需求修复轮不推进新里程碑。Bug 复核按实际结果说明复现加快检或切换前的完整门禁。
- v3/v4 风格旧模板补充一次更正；没有模式分段的旧模板追加实际结果说明。命令数据在模板切片与校验完成后插入，花括号不被解释。
- 发布协议源 v5 的两个 clarify 条目；新增 fix_gate_note 槽及 eval/eval_run 支持。静态短语清单 phrases.rs 同步两条必含内容（原来硬编码“只跑复现”和“没有宿主结果”，会拒绝新协议）；没有放松检查、没有修改 kernel-contract。
- 既有测试仅 PromptSpec 两处构造补 None；既有断言与旧金样均未修改。新增 10 个 lib 用例、3 个金样用例、4 份金样。

## 命令、结果和数量基线

所有原始输出在 `.autome/output/`，未运行 `sh scripts/ci-local.sh` 或其它项目全量门禁。

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test -p automed --lib -- gate_quick_prompt gate_quick_brief before_merge_prompt brief::tests::before_merge` | 退出 0；25 通过，0 失败 / 跳过 | `m04-lib-final.txt` |
| `cargo test -p automed --test prompt_rendering` | 退出 0；31 通过，0 失败 / 跳过，旧金样字节不变 | `m04-golden-final.txt` |
| `cargo run -p automed -- protocol eval protocol/` | 退出 0；8 项通过、0 不通过，既有条文溯源警告 1 项（42 行） | `m04-protocol-final.txt` |
| `cargo test -p automed --lib -- protocol::phrases` | 退出 0；5 通过 | `m04-phrases-final.txt` |
| `cargo fmt --all -- --check` | 退出 0 | `m04-fmt-final.txt` |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | `m04-clippy-final.txt` |

改前同口径实跑基线：lib 15、金样 28，共 43（`m04-baseline-lib.txt`、`m04-baseline-golden.txt`）。本轮主验收 56，比该基线 **+13**：新增 lib 10 + 金样 3。phrases 的 5 个既有用例是直接相关补验，不计为新增。上一轮 M-03 验收为 12 个，本轮主验收 56 与它相差 +44，原因是验收范围从宿主 Bug 调度切换到 prompt / brief，不能把这个差称为新增测试数。协议静态项、fmt、clippy 不计入 Rust 用例数。

## 负向对照

以下是本轮按验收契约主动构造的缺陷注入，不是上一轮审计遗留缺陷。均恢复产品代码后复验为绿，未改断言或金样。

| 注入 | 原样检查 | 红 / 绿 |
|---|---|---|
| 简报不看 kind（快检分支置 false） | `cargo test -p automed --lib -- gate_quick_brief_actual` | 退出 101，1 个失败：缺少“本轮宿主只跑了快检”；还原后主验收通过 |
| fix_gate_note 固定返回旧常量 | `cargo test -p automed --lib -- gate_quick_prompt_review_follows` | 退出 101，1 个失败：缺少实际快检命令和退出码；还原后主验收通过 |
| 用 replace 在模板检查前填 fix_gate_note | `cargo test -p automed --lib -- gate_quick_prompt_commands_are_data` | 退出 101，1 个失败：命令占位符被解释 / 拒绝；还原后主验收通过 |

日志：`m04-negative-brief-kind.txt`、`m04-negative-fixed-note.txt`、`m04-negative-data-replace.txt`；可重放注入脚本 `m04-negative.rb`。最初开发中的 Rust 生命周期、角色枚举名称以及 policy 局部变量编译错误均已修正；缺陷对照最终记录是断言失败，不以编译失败代替反例。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §8 情形 | 已执行检查与结果 |
|---|---|
| before_merge + quick，本轮 head 快检 | `gate_quick_brief_actual_result_and_timeout_in_both_modes`：成功、失败、超时均准确说明；新 audit 金样通过 |
| before_merge + quick，旧 head / 无结果 | `gate_quick_brief_stale_missing_and_legacy_results`：无当前结果新文案，旧结果不泄漏 |
| before_merge 无 quick，本轮完整结果 | `brief::tests::before_merge_same_head_preserves_success_and_failure`：T-61 原文通过 |
| before_merge 无 quick，本轮快检结果（移除配置） | `gate_quick_brief_actual_result_and_timeout_in_both_modes` 使用未配 quick 的配置仍识别实际快检 |
| each_round 最后一条按 kind 渲染 | 同一快检用例验证切换前说明；旧 full / 无 kind 的 3 个 before_merge 测试与兼容对照通过 |
| 实现 / 修复轮快检句，其它角色不变 | `gate_quick_prompt_round_sentence_only_for_impl_and_fix`：6 个角色与 Bug 3 步；其它角色字节对照通过 |
| Bug 复核 quick / full + 当前 quick / repro / 无结果 | `gate_quick_prompt_review_follows_result_across_mode_changes` 与 commands 用例：命令、退出码、兼容说明通过 |
| v3/v4 分段旧模板 / 无分段旧模板 | `gate_quick_prompt_legacy_templates_correct_once`：quick/full 各路径，更正一次或实际说明一次，无未填槽 |
| 新协议两个说明槽与行为评估 | eval/eval_run 两个新用例、protocol eval 及旧金样全通过 |

### 2. 失败分支

- 需求快检检查失败、快检超时：`gate_quick_prompt_feature_failure_uses_actual_kind_and_current_policy` 逐个执行，包含配置仍生效、移除 quick、切 each_round 三种策略；保留失败命令、tail、待审状态和轮次要求。
- Bug 复现失败、快检失败、FixGate 超时：`gate_quick_prompt_bug_failure_names_repro_and_quick` 三种 tail 执行；保留回归测试不可修改提示。
- 完整 MergeGate 失败后当前有 quick：feature_failure 用例的 full 对照检查“项目门禁”及快检/全量两种重跑说明。
- 非法位置的 fix_gate_note：eval 用例验证无分段与 each_round 段均拒绝；合法 before_merge 通过。
- 拒绝授权、读盘失败、外部服务不可达：不适用，M-04 修改的是给定结果的纯文字渲染，没有新增授权、文件读取或外部服务调用失败分支。

### 3. 全部迁移

M-04 不修改状态机。涉及说明的模式 / 模板迁移均执行：quick → each_round（更正恰好一次）、full → before_merge + quick（说明切换前完整运行）、quick → 移除 quick（仍依实际结果）、无 quick / 无结果（旧字节）；新模板、分段旧模板和无分段旧模板各有覆盖。需求和 Bug 两种失败注入均覆盖。

### 4. 输入域边界

- 空命令、含 `{slug}` / `{fix_gate_note}` / `{gate_line}` / 未知槽 / shell 变量的命令：commands 用例通过，数据原样保留。
- 无结果、Null、缺 sha、无 head、空 head、旧 head：brief / prompt 新旧用例通过，不编造运行结果。
- sha 长度 1、10、超过 10：既有 brief 用例执行；退出码 0、1、7、124，超时 5 / 30 分钟在新用例执行。
- 日期、前导零数值、闰日、跨年与数值最大值：不适用，本里程碑未引入日期或数值解析输入域；超时值读取既有结果，校验归 M-01。

## 交付边界

不推进 M-05。真实 UI 人工验收与四段快检耗时测量归 M-05；本轮不声称完成。只运行上述相关检查与下面记录的端到端补验。

端到端补验：`cargo test -p automed --test end_to_end -- gate_quick_feature_failing_quick_returns_to_implement_with_its_tail gate_quick_bug_failing_quick_returns_to_fix_with_its_tail` 退出 0，2 个通过、0 失败 / 跳过（54.92 秒，`m04-e2e-final.txt`）。真实 Git / worktree / 宿主作业路径中的需求和 Bug 快检失败仍正确返回并携带命令与日志尾部。

本轮最终执行的互不重复 Rust 用例总数 **63 = 25 + 31 + 5 + 2**。相同范围改前为 **50 = 15 + 28 + 5 + 2**，差 **+13** 全来自本轮新增。后两组 7 个用例为既有直接相关补验，未新增或修改断言；上一轮 M-03 的 12 个验收与本轮总数差 +51 是范围切换与新增的合计，不是新增量。负向对照复跑不重复计数。

结论：自审四项执行完毕，M-04 可标待审，交独立审计关闭；全量门禁未运行。
