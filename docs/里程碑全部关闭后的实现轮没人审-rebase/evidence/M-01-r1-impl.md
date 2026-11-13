# M-01 实现 #1 证据

仅推进 M-01。无上一轮审计文件，没有需要逐字迁入的审计回归检查。

## 实现与修复说明

- scheduler 在简报提交之后、启动进程之前记录 `start_heads`；兜底提交之后、替换 `task.head` 之前计算范围，在 `task.snapshot` 之后写 `integration.range`。
- 新模块 integration.rs 集中处理账本尝试分组和只读 Git 范围，复用现有 git::run_ok 的隔离配置、禁钩子与超时。H0 取最近 task.head 后第一次实现尝试，覆盖崩溃重试；旧会话缺字段不猜测。
- 逐仓库记录列出/消失提交，保守并入冲突文件；无 rename 的路径过滤；路径、diff、完整内联测试内容三层识别；载荷包含原因、证据文件名、统计与截断数量。
- 源码核对发现原有 `?` 传播只记录 TickReport 错误，此时会话已结束，任务仍 Active/Implement，不能直接 rerun。现在范围读取失败先以既有 FailureReason::Config 写 Failed 与 task.updated，再返回错误；不写范围事件、不启动审计、不新增失败类型。设计 §3 就地修正这个假设。

## 命令与用例总数

- 基线 `cargo test -p automed --lib integration_range`：退出 0，0 个匹配用例，794 个既有库测试被筛选。首次实现，无上一轮实测基线。
- 最终验收同一命令：退出 0，17 通过、0 失败、0 跳过，794 个既有库测试被筛选；总数 17，较基线 +17。包含设计指定的 16 个用例和新增调度器失败状态回归 1 个；验收契约同步增强为 17。边界增强放在这些用例内部。既有测试断言、前提不变。
- 仅对修改文件执行 `rustfmt --edition 2024` 格式化。本轮最新用户指令优先：未跑宿主快检（fmt check、clippy、Web lint/typecheck），未跑 ci-local.sh。
- 原始输出在 `.autome/output/m01-baseline.txt`、`m01-tests.txt`、`m01-final.txt`、`m01-negative-*.txt`、`m01-negative-summary.txt`。

## 标待审之前的自审清单

### 1. 情形表逐行

下列用例名称均加前缀 `integration_range_`，各行检查已执行。

| 设计情形 | 用例 | 检查结果 |
|---|---|---|
| §1(a)(b) 非空全关闭 | is_recorded_only_for_an_all_done_table_that_stays_done | 全关闭正例；轮前待审、轮后开放、空表拒绝 |
| §1(c)(d) 目录外差异 | linear_repair_lists_the_fix_commit | 单提交纳入，消失为空 |
| §1 纯文档 | doc_only_commits_are_not_integration | 纯文档排除，相似前缀目录不误排 |
| §1 简报、兜底 | counts_the_sweep_commit_and_skips_the_brief_commit | 简报先提交再取 H0；兜底提交纳入，证据名确切 |
| §1 崩溃重试 | a_crashed_attempt_keeps_its_commits | 两次提交均纳入；session_id 取第一次；新 task.head 后重置 |
| §2 rebase+追加 | rebase_lists_only_rewritten_and_new_commits | 只列重写与追加；旧版本列为消失 |
| §2 干净重放 | clean_rebase_is_empty | 两个集合均空 |
| §2 丢弃产品提交 | conflict_that_drops_the_product_commit_is_still_integration | 列出为空仍有集成载荷；消失含旧 SHA、paths、stat |
| §2 patch-id 等价 | conflict_file_commit_with_an_equivalent_patch_id_is_still_listed | 无清单时 Git 确实过滤；有清单时保守纳入 |
| §2 测试规则 1–3 | test_files_by_path_and_by_content | 六目录、四命名、assert 增删；普通文本不误列 |
| §2 测试规则 4 | inline_test_module_with_only_setup_changed_is_listed | 只改准备条件也列入 |
| §2 工作区 | workspace_member_paths_carry_the_repo_name | 成员同名文档路径仍为目录外；测试路径带仓库名 |
| §3 触发原因 | records_the_trigger_from_the_carried_inject | rebase/gate/none/decisions；gate 不含失败 SHA |
| Bug 排除 | bug_loop_records_nothing | Bug Fix 不进入范围逻辑 |
| §3 必需读取失败 | unreadable_range_fails_closed | Err、中文诊断、无事件；旧会话缺字段跳过 |
| §3 展示读取失败 | a_stat_failure_keeps_the_range | 保留范围和中文缺失说明 |

### 2. 失败分支

无效 H0 返回错误，不写事件；stat 注入失败保留范围；不存在路径的内容扫描保守列为测试文件；空表/空范围/旧会话无 start_heads 均检查。新增 scheduler::tests::integration_range_read_failure_stops_task_before_audit 通过真实启动+apply_guards，实跑确认 Failed(Config)、task.updated 与无 integration.range；完整后续审计回路仍留给 M-04。拒绝授权、外部服务不可达不适用：仅本地 Git/SQLite，无对应接口。Git 超时沿用现有运行器，无新独立超时分支。

### 3. 全部迁移

已执行：全关闭→全关闭+产品差异生成载荷；全关闭→开放、待审→全关闭、空表不生成；首次实现→崩溃重试保留 H0，task.head→下一实现重置 H0，缺旧字段跳过；线性→范围、干净 rebase→空、重写→列出+消失、只丢弃→消失触发、冲突等价→保守列出。必需事实失败→Failed(Config) 由 Git 失败测试与新增调度器用例实际执行；展示失败→保留范围由注入测试验证。审计回路属于 M-02/M-04，未提前验收。

### 4. 输入域边界

空范围、空表、单提交、缺字段、相似目录前缀、空仓库名/成员名均执行。51 提交保留 50 并记录 commits_omitted=1；31 行 stat 保留 30 并说明其余 1 行。字符串 hello world/helloworld 用真实 Git rebase 验证。日期、闰日、跨年、前导零不适用：无对应输入解析接口。

## 负向对照

临时破坏生产实现后逐项跑同一验收命令，脚本 `.autome/output/m01-negative.rb` 用 ensure 还原。七项全部以断言失败被拒绝（不是编译失败）：去掉 cherry-pick、去掉文档过滤、重试取最新 H0、忽略消失提交、去掉冲突保守并入、去掉内联扫描、stat 失败使范围失败。每项运行当时的 16 用例集；它们是重复实验，不计新增独立用例。还原后最终 17 用例全部通过。

另对新增调度状态用例运行精确子集 `cargo test -p automed --lib integration_range_read_failure_stops_task_before_audit`：临时删除 Failed 状态写入时 1 失败（红），还原后 1 通过（绿），均有运行断言；输出 `m01-state-red.txt` / `m01-state-green.txt`。这是同一个用例的重复实验，总独立用例仍为 17。全部临时缺陷已还原，`git diff --check` 退出 0。

## 边界核对

`rg -n 'guard.failed|task.snapshot' apps` 无匹配；继续查事件代码：通知匹配已有规则，新事件不发通知；本轮未改 apps。集成事件的新产品读者由 M-02 审计启动接线实现。

未改固定协议、协议源、金样、既有 e2e 或其他里程碑。未 rebase 本任务分支，无本轮 rebase 后冲突扫描要求。

## 交付

自审完成，M-01 标为待审；implementation-round 为 1/25。提交仅包含本里程碑实现、测试与任务记录；使用 NO_CI=1 避免提交钩子自行运行全量门禁。
