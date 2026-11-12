# M-05 · 实现 #5

结论：验收通过，提交独立审计；implementation-round 为 5/40。本轮只推进 M-05。

## 实现与范围

- `ResolvedConfig::gate_before_merge()` 仅在有门禁且模式为 before_merge 时提供命令；调度器传给所有任务角色，onboarding 传 None。
- domain 纯函数按整行标记选段，删除标记及其换行，保留选中正文的原始字节；拒绝未闭合、孤立关闭、重复/嵌套、未知模式及非独占行标记，并报行号。
- launcher 先选段，再按数据槽拆分、替换与验证，最后插入命令与用户需求。`${ci_script}`、`{slug}` 和需求中的分段文字不被解释。需求/Bug 的 GateFailed 在新模式下只要求复现失败部分和相关子集，保留门禁尾部与命令。
- 旧版 Bug FixReview 追加一次缺证事实说明；新版自带说明时不重复。intake、onboarding、each_round 及未配置门禁的原有渲染保持不变。
- protocol eval 静态层校验两种模式和 gate_line 位置；case 增加可选 gate_command，行为评估渲染根据它选段并最后插入命令。
- 未修改任何协议模板、prompt 金样、项目配置或其它里程碑实现。没有启动下一个会话。

上一轮审计 M-04 为通过，无退回缺陷，因此没有需要逐字迁入的审计判别检查，也没有对应的红/绿复现义务。本轮新增的是 M-05 契约要求的 17 个项目测试。

## 验收命令与结果

所有命令退出码均为 0，原始输出存于 `.autome/output/`。

| 命令 | 本轮通过/总数 | 上轮同口径基线 | 增量来源 | 日志 |
|---|---:|---:|---|---|
| `cargo test -p autome-domain before_merge_prompt` | 4/4 | 0 | protocol 3 + config 1 | m5-domain.txt |
| `cargo test -p automed --lib launcher` | 63/63 | 57 | 渲染、两类失败注入、无关入口、旧复核兜底 6 | m5-launcher.txt |
| `cargo test -p automed --lib protocol::` | 66/66 | 61 | eval 3 + case 1 + eval_run 1 | m5-protocol.txt |
| `cargo test -p automed --test prompt_rendering` | 17/17 | 17 | 0；现有金样原样通过 | m5-golden.txt |
| `cargo test -p automed --test end_to_end prompt_before_merge` | 2/2 | 0 | 需求/Bug MergeGate 失败修复路径各 1 | m5-e2e.txt |

合计 **152 个用例，相对上一轮同口径 135 个增加 17 个**，失败 0、忽略 0。domain 的独立集成测试在本过滤器下命中 0 个，不计入上述 4 个；未把过滤掉的用例算作通过。库总测试规模从上一轮 domain 390 / automed 663 增至 394 / 674，e2e 从 119 增至 121；这些总规模来自测试运行的通过数加 filtered 数，不代表本轮运行了全量。

其余契约检查：

- `git diff --quiet HEAD -- protocol/prompts crates/automed/tests/golden/prompts`：0。
- `test -z "$(git log --format=%h $(git merge-base HEAD main)..HEAD -- protocol/prompts crates/automed/tests/golden/prompts)"`：0。
- `cargo fmt --all -- --check`：0（m5-fmt-check.txt）。
- `git diff --check`：0（m5-diff-check.txt）。

本轮没有运行全量 ci-local、clippy 或真实模型行为评估；完整宿主门禁仍由 Autome 在提交后调度，不能把本轮子集结果当作全量通过。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行

- 配置六种组合：缺省/each_round/before_merge × 有/无门禁；只有 before_merge 且有门禁返回原命令。`before_merge_prompt_command_requires_both_gate_and_mode` 通过。
- 无标记旧模板：空串、无末尾换行、中文与 CRLF，两模式逐字节保持原样。`before_merge_prompt_unmarked_bytes_are_unchanged` 通过。
- 相邻双段、单独 before_merge、单独 each_round、反向相邻双段、空段：两模式输出均与手写字节一致，标记行不残留空行。`before_merge_prompt_sections_preserve_exact_bytes` 通过。
- 角色与 Bug 两条渲染通道：None 选旧段，Some 选新段；make check、shell 变量与字面 `{slug}` 保留；Bug 需求中的标记和占位符保留。launcher 对应两个测试通过。
- 需求/Bug × None/Some 的失败注入：旧模式逐字匹配原注入；新模式含尾部、命令与宿主句，无旧的全量重跑要求。对应两个 launcher 用例及两条真实 Git/worktree/门禁作业 e2e 通过。
- intake/onboarding：Some 与 None 相等。旧 Bug Locate/Fix 不加说明、FixReview 末尾只加一行；新模板 FixReview 只有自带的一次说明。对应两个 launcher 用例通过。
- eval：段内 gate_line 接受，段外拒绝；case 有字段/无字段及未知字段；eval_run 两模式与原始命令。protocol 五个新增用例通过。

### 2. 失败分支

- 未闭合、孤立关闭、重复/嵌套、未知模式、非独占行：domain 用例在两模式下均检查 Err 和行号，通过。
- 静态 eval 的未闭合模板检查文件名与行号；段外 gate_line 拒绝；段内接受构成正向对照，通过。
- launcher 的未知占位符仍拒绝，each_round 下段外 gate_line 仍拒绝；角色与 Bug 均覆盖，通过。
- 真实 MergeGate 失败分别返回需求实现/Bug 修复，检查真实启动 prompt 包含门禁尾部与新范围说明，然后修复至 await_merge，通过。
- 拒绝授权、读盘失败、超时、外部服务不可达：不适用；M-05 新增的是纯文本渲染和字段传递，没有新增这些 I/O 或作业生命周期分支。空输入已在域边界检查。

### 3. 全部迁移

- 分段状态：段外→EachRound、段外→BeforeMerge；EachRound→BeforeMerge、BeforeMerge→EachRound；两种活动段→关闭→段外；段外正文/活动段正文保留或丢弃。上述单段、正反双段和多段用例均执行通过。
- 非法迁移：段外关闭、同模式重复、模式往返嵌套、活动段 EOF、未知模式；均错误返回并带行号。
- 渲染通道：Role/Bug/评估 × each_round/before_merge 均跑过；intake/onboarding × None/Some 相等；FixReview × 旧/新模板与 None/Some 覆盖。
- 业务状态机本轮未改；本轮相关的 MergeGate 失败→实现/修复→再次 MergeGate→await_merge 两条端到端回路实跑通过。

### 4. 输入域边界

- 空文本、空段、单独段、多个段、没有末尾换行、CRLF、标记行尾空白、正文前后空行：domain 字节断言通过。
- 无 gate_command、空 gate_command、普通命令、含 shell 变量及 `{slug}` 的命令：解析/渲染测试通过；空命令在 eval case 中作为 Some 按存在性选模式，项目配置仍由既有非空命令规则控制。
- 用户需求中含分段标记、`{gate_line}` 与 `{slug}`：原文保留，测试通过。
- 前导零、闰日、跨年、数值最大值：不适用；本里程碑没有新增日期或数值输入域。

## 现有测试适配与设计事实修正

- 按 §10，两个 PromptSpec 测试构造器只补 `gate_before_merge: None`。
- 实际还存在 `protocol::eval_run::tests::case()` 的 Case 字面量；首次编译因缺 gate_command 失败（m5-new.txt），补 `gate_command: None` 后通过。设计中“Case 只由 parse 构造”的事实已就地修正。
- `no_placeholder_reaches_the_model_even_though_a_case_has_no_brief` 的 render_prompt 调用补 None 和 unwrap 以适配可失败的选段函数；所有原有断言保持不变。没有删除、跳过或弱化现有用例。
