# M-06 · 实现 #6 · 待审

范围：设计 §6.5、§6.9、§7、§10；只推进 M-06。上轮审计 M-05 通过，没有需要逐字搬入的退回检查。

## 改动

- 五份模板增加 before_merge 分段；each_round 原行不变。新增五份 make check 金样。
- protocol/v3 G-1..G-5、五个行为 eval（含脚手架、fixture、grader 阳性对照）、8 条短语约束及 README 说明。补登记编译种子 evals.rs，否则新增目录不会进入二进制种子。
- §10 允许的已有测试调整：两处版本断言 v2→v3；金样刷新分支先按 EachRound 选段，断言不变；with_behavioral_entry 与 a_changed_run_covers_the_new_cases_and_the_three_baselines 的合成版本改为 v90，保留 C-90 内容和断言强度。
- 未改 bug-protocol.md（留给 M-07）、loop-protocol.md、session-protocol.md、项目配置或本任务冻结协议。

## 验收命令及数量差

所有原始输出位于 `.autome/output/`，未转存到任务目录。

| 命令 | 结果 | 上轮基线与差 | 日志 |
|---|---|---|---|
| `cargo test -p automed --test prompt_rendering` | 20 通过，0 失败/忽略 | 17→20，+3：角色金样、Bug 金样、其它模板字节不变 | m06-prompts-green.txt |
| `cargo test -p automed --lib protocol::` | 67 通过，0 失败/忽略 | 66→67，+1：五份种子两模式选段 | m06-protocol-final.txt |
| `cargo test -p automed --test end_to_end protocol_before_merge` | 2 通过，0 失败/忽略 | 0→2：需求及 Bug 角色 prompt 实际调度 | m06-e2e.txt |
| `cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` | 1 通过 | 1→1，无新增 | m06-frozen.txt |
| `cargo run -q -p automed -- protocol eval protocol/` | 退出 0；8 项通过、0 不通过；19 个 eval，10 个 CHANGELOG 条目 | eval 14→19，+5 对应五角色；静态警告为未被 CHANGELOG 覆盖的 42 行工作清单，不是失败 | m06-eval-final.txt |

上述 Rust 验收集合共 90 个，对应上轮同范围 84 个，净增 6。完整测试清单中 automed lib 675（+1）、end_to_end 123（+2）；没有宣称运行这两个全量集合。

12 份旧金样按契约 for 循环逐份与 merge-base main 比较，退出 0，无改动；`cargo fmt --all -- --check` 与 `git diff --check` 退出 0。

## 标待审之前的自审清单

1. **情形表逐行**：plan 不把全量写入验收；impl 只跑本里程碑与相关子集；audit 明确缺全量不阻止结论并覆盖简报通则；fix 复现加相关子集；fix-review 明示无宿主结果且不要求全量。五行分别经新金样与种子两模式测试通过，需求/ Bug e2e 验证实际调度。review/adjudicate/locate/locate-card/intake/intake-card/onboarding 与旧金样一致，retro 两模式一致。each_round 12 份旧金样逐字节保持。
2. **失败分支**：临时篡改 audit 旧模式文字、fix-review 新模式恢复全量要求、plan 漏 gate_line，prompt_rendering 返回 101，16 通过/4 失败（新角色金样、新 Bug 金样、旧角色金样、预算占位符位置）；还原后 20 全绿。日志 m06-negative-prompts.txt、m06-prompts-green.txt。临时复制协议后移除 G-1 eval 整目录，静态 eval 返回 1，明确报目录不存在；移除 grader 阳性对照返回 1，明确报无阳性对照，日志 m06-negative-missing-dir.txt、m06-negative-grader.txt。仅删 case.yaml 的探索返回 0（目录仍在），它不等于设计要求的目录缺失对照，未用它充当通过证据。拒绝授权/读盘失败/超时/外部服务不可达不适用：本里程碑为模板与评测素材，未增加这些运行分支。
3. **全部迁移**：无新增状态机迁移。模板通道五角色×两模式共 10 个选段均检查，标记不泄漏、BeforeMerge 各一个 gate_line、EachRound 无 gate_line；需求设计→实现→审计与 Bug 修复→复核由两条真实宿主 e2e 覆盖。既有协议冻结用例通过。
4. **域边界**：模式域 EachRound/BeforeMerge、无门禁缺省（旧金样）、五个单占位符及不参与分段的其它模板均覆盖。协议现有 67 个测试含不闭合标记、段外占位符拒绝、段内接受、未知字段等边界。日期/数值最大值不适用，本轮不新增该输入域。

五个 scaffold 在五个独立临时目录实跑：check-m01 通过，full-gate-ran 原先不存在，执行 full-gate.sh 后存在且非空，退出 0（m06-scaffolds.txt）。行为 grader 的真实模型表现未运行，按 H-02 留给合并前人工执行，不以静态检查代替。

## 过程与限制

初次静态测试暴露新增 eval 未登记种子（60/67 通过），补齐 evals.rs 后 67/67 通过。一次全金样刷新并发读写导致旧卡片金样瞬时空读；普通非刷新运行 20/20 通过，旧金样最终字节未变。未运行全量 ci-local、clippy 或付费真实模型评测，本轮只执行 M-06 验收与相关子集。
