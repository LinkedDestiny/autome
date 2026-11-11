# M-02 实现轮 #2

## 范围与修复说明

推进 M-02：Bug 协议 v2、定位/修复/复核模板、种子与静态门、渲染与金样。上一轮审计 M-01 通过，无退回缺陷，故无需搬入审计判别检查。

既有测试清单补充理由（修改前记录）：`protocol::eval` 的 `with_behavioral_entry` 将人为追加的未来版本写死为 v2，与本次真实 v2 冲突，`changed_cases` 取到第一份 v2，丢失夹具条目；改为 v3，同时改该夹具的查询版本，不改变断言。`eval_run::tests::case` 因 Case 新增可选 prompt 字段须显式填写 None，旧用例仍选 role 模板。初次测试的失败记录在 `.autome/output/m02-first.txt`；CHANGELOG 新 eval 路径缺 `evals/` 前缀也已定位，将按现有目录契约修正。

新增的 Bug GateFailed/RebaseConflict 注入只针对 Bug 会话，避免继承需求路径中的 implementation-round、里程碑与证据文件指令；需求注入分支原样保留。Bug 渲染必须拿到内核预算，定位卡片前置段逐字沿用 K1，先替换模板、检查占位符，再插入用户需求，正文不进入模板替换。

## 验收命令与当前证据

完整 M-02 命令链（`.autome/output/m02-acceptance.txt`）：

1. `cargo run -p automed -- protocol eval protocol/`：退出 0，报告 `protocol/v2`，14 个 eval（旧 11 + 新 3），契约区 6 个，正文 24446/24576 字节。保留既有未覆盖条文警告，未把警告当失败。
2. `cargo test -p automed --test prompt_rendering`：16 passed，相对上一轮 11 **+5**，旧用例未删。
3. `cargo test -p automed --lib launcher::tests::bug_loop`：3 passed。
4. `cargo test -p automed --lib bug_loop_protocol`：7 passed。
5. `cargo test -p automed --lib protocol::`：61 passed，相对原 54 **+7**，包括第 4 条的 7 个，不重复计数。
6. `cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：1 passed，原用例数不变。

新增 **15 个**（金样/边界 5、注入 3、协议 7）。设计原列 14，额外 1 个 `bug_loop_protocol_eval_runs_the_selected_bug_template` 用替身 runner 断言真正送入模型接口的是选定 Bug 模板、路径已填好，而非仅验证 YAML 能解析。设计预期同步为 15。首次金样检查 12 passed / 4 failed（新金样不存在）；阅读模板及卡片前置段后，仅用 `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering bug_loop` 生成四份新金样，既有金样未刷新。

## 标待审之前的自审清单

### 1. 情形表逐行

逐行对照设计 §2、§6、§8、§10：

- 定位模板：定位根因、提交失败测试、写前三个章节；`0/N`、待复核、无法复现/不是 Bug 和工作区 repos 要求均在协议与模板中。locate 金样通过。
- 修复模板：最小改动、不改测试、同一复现命令、更新轮次、重置待复核、可修正根因与只追加 repos。fix 金样通过。
- 复核模板：根因/测试/范围三查，只改复核状态与意见，禁止修代码或自行合并。fix-review 金样通过。
- 有卡片/无卡片：四份新金样逐字对比；有卡片时路径在需求之前，后缀与无卡片版本完全一致。卡片正文只引用文件。
- 三种预算：定位 `0/3`、修复 `2/3`、复核 `2/3`，更大/更小预算由边界测试证明；不读 brief。
- 三份模板随种子冻结：seed 与契约区测试通过，原需求冻结 e2e 通过；不改 ROLE_PROMPTS/EVERY_ROLE。
- YAML 有 prompt/无 prompt：新模板选择与旧角色回退测试通过；三个选定 Bug 模板实际交给替身 runner。
- 三个行为夹具：`.autome/output/m02-fixtures.rb` 在隔离目录用真实 sh/Git 搭建。定位夹具证明空输入错误退出 0；修复夹具 regression 先退出 1，只修产品后退出 0，测试字节不变；复核夹具弱化测试后退出 0，但 Git 提交差异准确暴露改测试。结果见 `.autome/output/m02-fixtures.txt`。真实模型行为评估未跑。

### 2. 失败分支

- 复现未失败 0、命令不存在 127、不可执行 126、超时 -1：注入测试逐一断言命令、提交、退出码、why、原始多行 tail；此处验证传达，运行时分类已属 M-01，实际调度留 M-03。
- 门禁失败：传递同一命令和原始 tail，且没有 implementation-round/证据文件指令；复核打回指向 Bug 文档复核意见；rebase 冲突点名文件并重置复核。
- 缺 Bug 模板、缺预算、空/不存在的 case prompt：逐项拒绝并指出原因。三份模板缺预算占位符、缺禁止接力短语、整份缺失均拒绝。
- 拒绝授权、读盘故障、外部服务不可达：不适用 M-02 的纯文本渲染，不新增 IO/授权/服务通道；没有声称本轮实跑这些运行时故障。

### 3. 全部状态迁移

本里程碑不改状态机，状态 × 事件运行验收不适用（M-01 已关闭、调度 M-03）。本轮文本通道全部覆盖：Locate/Fix/FixReview → 对应模板；Locate × 有/无卡片；ReproNotFailing → Locate；GateFailed/FixRejected/RebaseConflict → Fix；case 显式 prompt/缺省 role → 对应模板。每项由金样、launcher 或 eval-run 测试执行，不用正路径代替失败通道。

### 4. 输入域边界

- 预算 `(0,1)`、`(1,1)`、`(20,20)`、`(u32::MAX,u32::MAX)` 对三种 Bug 步骤均渲染并核对；缺预算拒绝。预算配置 1..20 的校验属于 M-01，不在渲染层重复限制追加轮次。
- 空需求、中文工作区卡片路径、需求中的 `{literal}`、单个 case prompt；空 prompt 与不存在/越界样式模板名拒绝。多行中文日志原文保留。
- 前导零、闰日、跨年：不适用，渲染输入是已解析 u32、不解析日期；Bug 状态块解析器的前导零边界已属 M-01。

## 负向对照（均已还原）

`.autome/output/m02-negative.rb` 逐项变异后运行项目测试，并在 ensure 还原源码。汇总 `.autome/output/m02-negative-summary.txt`，各原始输出 `m02-negative-*.txt`。

- 从 fix 模板删除 `{budget_line}`：eval 退出 1；对应预算测试退出 101（断言失败）。
- 契约示例改为 `复现命令 = …`：内核解析示例测试退出 101。
- 定位卡片前置分支改给 Fix：locate-card 金样测试退出 101。
- 既有 impl 模板加文字：既有 `every_role_prompt_renders_exactly_what_the_constants_did` 退出 101。

以上都不是编译失败；还原后的全量测试作为绿证据。另对比任务基线 `81a4394` 的全部受保护需求协议、既有模板/金样及 apps/plugins/scripts/.github：`git diff --exit-code` 退出 0，输出为空（`.autome/output/m02-protected-diff.txt`）。

## 用户裁定与后续

B-01、B-02、B-04～B-08 按用户决定忽略、不实施；B-03 纳入第六个里程碑 M-06，包含独立验收命令与正负对照，遵循本轮只推进最小开放里程碑 M-02 的规则，本轮不提前实现 M-06。修复轮测试机械守卫尚未交付，不把 prompt 禁止当作已经机械阻止。

## 还原后的全量绿证据

`cargo test --workspace` 退出 0，**1047 个用例，1047 通过、0 失败、0 忽略**，相对上一轮基线 1032 **+15**。分层：CLI 单元 17、CLI 集成 8、领域 345、automed 单元 595（585 +10）、e2e 59、prompt 16（11 +5）、real_cli 7；doc-test 0。日志 `.autome/output/m02-workspace.txt`。重复执行的专项不累加；real_cli 既有环境条件未改，此统计不代表真实模型验收。

`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --all -- --check` 均退出 0，日志 `.autome/output/m02-clippy.txt`、`.autome/output/m02-fmt-check.txt`。`git diff --check` 退出 0。

四项自审与负向还原验证均完成，M-02 可标待审；M-01 保持审计已完成，其余保持开放。本轮不启动下一会话，不推送远端。真实模型行为 eval、真实 Bug Loop 人工验收未执行。

状态格式检查：使用本轮构建的 `autome_domain::status_block::parse` 读取最终设计文档，断言实现中、轮次 2/30、6 个里程碑、M-01 Done、M-02 Pending、M-03～M-06 Open，退出 0；源码和结果在 `.autome/output/m02-validate.rs`、`.autome/output/m02-status-validation.txt`。没有修改任务冻结协议、其它 worktree 或 `.autome/` 配置；该目录仅写入本轮要求的 output 验证产物。
