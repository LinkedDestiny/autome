# M-01 · 实现 #1

## 范围与实现

仅推进 M-01。新增旁挂 GateRun、ProjectConfig.gate_run、ResolvedConfig.gate_run 与 gates_each_round()；解析项目 [gate] run，唯一生产转换改动是 Implement 完成处改用 gates_each_round()。MergeGate、MainCheck、全局解析和保存器不改。现有测试与断言未修改；新增 domain 6 个、config_io 5 个。

没有上一轮审计、没有待搬入的审计缺陷检查。本轮 implementation-round 为 1/40。

## 命令、总数与结果

开始时独立测得基线（不是上一轮，本任务尚无上一实现轮）：
- cargo test -p autome-domain：单测 384 通过；集成测试总数 1，默认忽略 1；doc-test 0。日志 .autome/output/m01-baseline-domain.txt。
- cargo test -p automed --lib config_io：40 通过。日志 .autome/output/m01-baseline-config.txt。

验收命令：
`cargo test -p autome-domain before_merge && cargo test -p autome-domain && cargo test -p automed --lib config_io`

最终恢复正确实现后再次执行，退出码 0：
- before_merge：6 通过，基线该过滤器 0，+6（config 1、task 5）；这 6 个包含在下项，不重复计数。
- domain：单测总数 390，390 通过，+6；集成总数 1，忽略 1，增量 0；doc-test 0。
- config_io：总数 45，45 通过，+5（默认/两模式、非法输入、角色保存、空命令、全局忽略）。
- 去重合计总数 436（435 通过、1 原有忽略），基线 425，+11；没有删除、改写或新增忽略任何旧用例。
- 原始输出：.autome/output/m01-final-filter.txt、m01-final-domain.txt、m01-final-config.txt。
- cargo fmt --all -- --check：退出码 0，日志 .autome/output/m01-final-fmt.txt。
- git diff --check：通过。

设计验收原写“集成 1 个通过”与源码不符：tests/t14_gate_logs.rs 明确 #[ignore]，需 AUTOME_T14_GATE_DIR 指向原始 T-14 日志。基线和本轮同为忽略 1，本轮未提供该日志、不伪造通过；就地订正 M-01 的预期。完整宿主门禁/clippy/e2e 不在本轮会话重复执行，交由宿主；本证据不声称这些已通过。

## 标待审之前的自审清单

### 1. 情形表逐行

- §1 缺省 run：ProjectConfig None，resolve EachRound；显式 each_round 与缺省 resolve 全字段相等。parse_default_and_explicit_modes 通过。
- §1 before_merge：解析为 BeforeMerge，resolve 保留；默认 JSON 不增加 gate_run，旧 JSON 反序列化恢复默认；before_merge JSON 值正确。defaults_override_and_json_compatibility 通过。
- §1 其它值：sometimes、整数、布尔、空串、大小写变体、前后空格、数组、表逐项拒绝并断言完整中文错误。invalid_run_is_rejected 通过。
- §1 command 为空/纯空格：两合法模式仍被保存、gate None、不启用逐轮门禁；非法模式仍拒绝。empty_command_still_validates_run 通过。
- §1 全局 run：load_global 接受并忽略 sometimes，与 GlobalConfig::default 相等。global_gate_run_remains_ignored 通过。
- §1 保存：改 Audit.enabled 后 save_project，整段 gate（含注释）原文保留，重载模式和角色均生效。save_role_preserves_gate_text_and_reload 通过。
- §2 审计开：Implement 完成与无 gate 的 Transition 全字段相等，目的节点 Audit。
- §2 审计关五行：有开放里程碑、无开放且复盘开、无开放且复盘关、预算耗尽、有纳入条目，分别与无 gate 的 Transition 全字段相等；纳入条目还断言消费标记与预算 35。
- §2 Rebase/Merging 成功：分别进 MergeGate/MainCheck，断言 RunCoreStep。
- §2 MergeGate：通过/失败 × 已请求/未请求合并四种组合，与 each_round 全字段相等。
- §2 原 Gate 收到 Passed：与 each_round 全字段相等，进 Audit。

### 2. 失败分支

- 配置非法值拒绝：上述 8 类输入均实际执行；空命令非法值也执行。
- 实现预算耗尽：审计关闭、开放里程碑、impl_round=25，实际执行并与无门禁失败转换相等。
- MergeGate 失败：请求/不请求合并两条都执行，比较包含 GateFailed 注入的完整 Transition。
- 授权拒绝、读盘失败、超时、外部服务不可达：不适用；M-01 不新增这些 I/O/授权分支，不改变对应现有实现。

### 3. 全部迁移

M-01 变更域的全部迁移已执行：Implement→Audit；关闭 Audit 时 Implement→Implement / Retro / Rebase / Failed(ImplBudget)，含纳入决议消费；Rebase→MergeGate；Merging→MainCheck；MergeGate Passed→AwaitMerge/Merging、Failed→Implement（两种 merge_requested）；原 Gate Passed→Audit。完整 Transition 比较同时覆盖 action、budget_n、consumes_decisions、merge_requested，不只比较目的节点。域单测全量保留现有 each_round 转换覆盖。

### 4. 输入域边界

- 两合法枚举值、缺省、空串、大小写/空白变体、非字符串集合：逐项执行并通过。
- 空/纯空格 command 与合法/非法 run 交叉执行。
- 单里程碑、预算恰等于 25、有纳入条目 2（预算变为 35）执行。
- 日期、闰日、前导零、跨年、数值最大值：不适用；新输入为封闭字符串枚举，未引入日期/数值字段。

## 负向对照（临时变异已恢复）

1. 将 Implement 判断恢复为 ctx.config.gate.is_some()，运行 cargo test -p autome-domain before_merge_implement：退出 101，两例均红（审计开/关），日志 .autome/output/m01-negative-transition.txt。
2. 将 sometimes 临时当作 EachRound，运行 cargo test -p automed --lib config_io::tests::before_merge_invalid_run_is_rejected：退出 101，一例红，日志 .autome/output/m01-negative-parse.txt。
3. 恢复两文件后重跑完整 M-01 验收链，全部绿（见最终日志）。临时备份仅存 .autome/output/，未纳入提交。

结论：M-01 待审；无产品实现阻塞。原有 T-14 日志测试未实跑，已明确基线与原因。
