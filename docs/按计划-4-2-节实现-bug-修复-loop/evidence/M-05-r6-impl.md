# M-05 实现 #6：需求任务零变化与全量门禁

## 范围与修复说明

本轮仅推进 M-05；起点 `6ff96c9`，上一轮 M-04 审计通过，无退回缺陷，故没有必须逐字搬入的审计复现检查。本轮不新增功能、不改产品代码或既有测试。

差异检查发现设计 §11 漏列 `end_to_end.rs` 的 `World::has_pending_work`：它在既有六个宿主节点之外追加 `repro_check` / `fix_gate`，防止 Bug 测试在宿主作业执行前提前结束。旧节点集合与判定不变，仅覆盖新增 Bug 节点；这是测试驱动器支持 M-03 的必要适配，不是放宽既有断言。先在此说明，再补为 §11 第 8 项。

## 既有测试逐项核对

`git diff 81a4394 -U0 -- crates` 全文：`.autome/output/m05-r6-crates.diff`；四份集成测试及金样差异：`.autome/output/m05-r6-integration.diff`。

1. §11.1：protocol/mod.rs 仅将既有版本断言 v1 改 v2，哈希断言保留。
2. §11.2：end_to_end 冻结协议测试仅将前缀 v1@ 改 v2@，current().to_wire() 断言保留。
3. §11.3：Bug 草稿测试定位替身改写 Bug 文档、删除旧设计轮 2–4、等待改为 await_info；卡片字节、提交顺序、prompt 顺序与 kind 断言保留。
4. §11.4：CLI Bug 草稿测试追加定位替身并等待 await_info；task_card_prepare 与卡片字节断言保留。
5. §11.5：CLI reply 既有循环只保留 feature；Bug reply 由新增测试覆盖。
6. §11.6：protocol/eval.rs 假想 CHANGELOG 版本与 changed_cases 查询同步 v2 → v3，断言不变。
7. §11.7：protocol/eval_run.rs 的 Case 夹具增加 prompt: None，仍走旧 role 模板。
8. §11.8：World::has_pending_work 追加两个 Bug 门禁节点，理由见上。

除此之外各既有 tests 模块没有删除或替换行，新增内容为独立测试。prompt_rendering.rs 只追加；real_cli.rs 无差异；task.rs 既有测试模块无差异。没有新增忽略标记，也没有删除既有测试。

## 验收命令与结果

- `cargo test --workspace`：1071 通过、0 失败、0 忽略；CLI 单元 19、CLI 集成 10、domain 345、automed 单元 601、e2e 73、prompt 16、real_cli 7，doc-test 0。日志 `.autome/output/m05-r6-test.txt`。
- 相对上一实现轮 M-04-r5 的完整基线 1071：**差 0**，本轮没有新增或删除用例。相对 M-01 记录的原始基线 990：**+81 = 42 + 15 + 11 + 13**，分别来自 M-01 至 M-04。专项重复运行不累计。real_cli 条件性测试报告通过不代表实际调用了真实模型。
- `cargo clippy --all-targets -- -D warnings`：通过，日志 `.autome/output/m05-r6-clippy.txt`。
- `cargo fmt --all -- --check`：退出 0，日志 `.autome/output/m05-r6-fmt.txt` 为空。
- `cargo run -p automed -- protocol eval protocol/`：版本 protocol/v2，8 项通过、0 项不通过、1 项既有警告（42 行条文缺 CHANGELOG 来源）；日志 `.autome/output/m05-r6-eval.txt`。
- 设计 M-05 验收契约中的完整 `git diff --exit-code 81a4394 -- …` 路径集合：退出 0，`.autome/output/m05-r6-protected.diff` 为空；覆盖四份协议基础文件、八份需求 prompt、八份既有金样及 apps/plugins/scripts/.github。
- `git diff --check`：通过。

## 标「待审」之前的自审清单

### 1. 情形表逐行

M-05 无新增业务情形表；按其指定的 §11 改动清单逐行检查，结果见前述 1–8。按范围逐项检查：C2 的需求转换由领域层比较测试与既有测试通过、需求模板与金样逐字节保护通过；C3 的 feature reply/reject 等价、task JSON 既有键集测试通过，桌面端文件无差异；C7 协议静态门与 16 个 prompt 测试通过；C11 全量测试/lint/fmt 通过；C12 原始输出仅放 `.autome/output/`。

### 2. 失败分支

执行全量所含 `bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced`（超时、126/127）、`bug_loop_reply_is_refused_when_empty_or_not_waiting`（空输入/错误状态）、`bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend`（操作拒绝）、`bug_loop_session_errors_fail_like_feature_sessions`（三个会话节点 × 崩溃/解析错误/缺产物/守卫失败），均通过。M-05 不引入授权、外部服务与读盘新路径；真实系统授权和外部服务不可达检查不适用，本轮没有这些依赖。

另外两项 M-05 点名的负向对照均执行：
- 将 Feature 分流临时改为 Queued 不走需求表，使其误走 Locate；原项目测试 `bug_loop_feature_kind_matches_the_feature_table_exactly` 退出 101，失败处为 Queued + SlotAvailable；恢复后同一命令退出 0，1 passed。日志 `.autome/output/m05-r6-negative-transition.txt` 与 `m05-r6-restored-transition.txt`。
- 向 `protocol/prompts/impl.md` 临时追加一个空格：`git diff --exit-code 81a4394 -- protocol/prompts/impl.md` 退出 1；恢复后退出 0。差异 `.autome/output/m05-r6-negative-prompt.diff`。所有变异已还原，不交付变异。

### 3. 全部迁移

本轮无新增迁移。全量中的 `bug_loop_feature_kind_matches_the_feature_table_exactly` 枚举 Draft/Queued/Done/Cancelled，加 Node::ALL 每个节点的 Active/Paused/Stopped/Requeued 和各失败原因，与测试触发器集交叉，再覆盖 merge_requested=false/true，逐项比较需求表输出。既有 task::tests 原文保留且全量通过。

Bug 29 项领域测试同时覆盖：排队到定位；定位三种结论；复现成功、一次未命中、二次未命中；修复到门禁；门禁成功进入复核或跳过复核、失败返回修复；复核通过/打回；rebase 成功/冲突；merge_gate 失败返回修复和共用合并尾段；两个等待节点 reply/cancel 与拒绝操作；预算耗尽/追加；手动重跑；门禁停止恢复与重新排队。73 个 e2e 包含 14 个 Bug 用例和既有需求流程，均通过。

### 4. 输入域边界

M-05 无新增输入域。全量包含既有 Bug 解析器 8 项与配置测试：fix_rounds 缺省、1/3/20 接受，0/-1/21/u32 最大值及溢出、浮点/布尔/字符串/数组拒绝；reply 空白拒绝及原文多字节保留；状态块缺字段、重复字段、非法枚举、空命令等拒绝，全部通过。日期、闰日与跨年不适用，本里程碑不新增日期输入。

## 交付边界

M-05 仅标待审；M-06 仍开放，回归测试文件保护尚待后续实现。未执行人工清单中的真实模型/桌面/安装验收；没有启动下一会话，没有推送或切换分支。
