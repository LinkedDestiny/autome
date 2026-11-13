# M-01 实现 #2 修复轮证据

本轮仅修复宿主快检失败，M-01 保持待审，不推进 M-02；implementation-round 更新为 2/25。没有上一轮审计文件，本次缺陷由宿主 Clippy 检查发现。

## 修复说明与红绿证据

- 根因：`repo_range` 参数 `&dyn Fn(&Path, &str, Option<&str>) -> Result<String>` 触发 `clippy::type_complexity`。改为 `&impl Fn`，保留入参、返回值、借用方式、调用逻辑与失败处理；无 lint 豁免，无测试前提或断言改动。
- 同类检查：搜索 `crates/automed/src` 相同动态回调形状，修后无剩余匹配；全目标 Clippy 通过。
- 红：修改前原样运行 `cargo clippy --all-targets -- -D warnings`，退出 101，复现 integration.rs:203 的 type_complexity。原始输出 `.autome/output/m01-r2-red.txt`。
- 绿：修改后运行用户指定的完整快检 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`，退出 0。原始输出 `.autome/output/m01-r2-quick.txt`。
- 此检查本来就在项目/宿主快检体系内，保持原命令作为回归检查，无需新增镜像测试。未运行 `sh scripts/ci-local.sh`。最终快检结果以 Autome 提交后重跑为准。

## 验收命令与用例总数

`cargo test -p automed --lib integration_range`：退出 0，17 通过、0 失败、0 跳过，794 个被筛选；独立用例总数 17，较上一轮 17 的基线差为 0。本轮仅改回调参数的分派方式，没有增删测试。快检为静态检查，不计为测试用例。原始输出 `.autome/output/m01-r2-tests.txt`。

## 四项自审清单

本轮重新执行上述完整 M-01 测试子集，逐项核对以下既有检查，全部通过。沿用上一轮用例对应表，未把上一轮结果代替本轮运行；本轮不重复缺陷注入实验。


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

无效 H0 返回错误，不写事件；stat 注入失败保留范围；不存在路径的内容扫描保守列为测试文件；空表/空范围/旧会话无 start_heads 均检查。既有 scheduler::tests::integration_range_read_failure_stops_task_before_audit 通过真实启动+apply_guards，实跑确认 Failed(Config)、task.updated 与无 integration.range；完整后续审计回路仍留给 M-04。拒绝授权、外部服务不可达不适用：仅本地 Git/SQLite，无对应接口。Git 超时沿用现有运行器，无新独立超时分支。

### 3. 全部迁移

已执行：全关闭→全关闭+产品差异生成载荷；全关闭→开放、待审→全关闭、空表不生成；首次实现→崩溃重试保留 H0，task.head→下一实现重置 H0，缺旧字段跳过；线性→范围、干净 rebase→空、重写→列出+消失、只丢弃→消失触发、冲突等价→保守列出。必需事实失败→Failed(Config) 由 Git 失败测试与既有调度器用例实际执行；展示失败→保留范围由注入测试验证。审计回路属于 M-02/M-04，未提前验收。

### 4. 输入域边界

空范围、空表、单提交、缺字段、相似目录前缀、空仓库名/成员名均执行。51 提交保留 50 并记录 commits_omitted=1；31 行 stat 保留 30 并说明其余 1 行。字符串 hello world/helloworld 用真实 Git rebase 验证。日期、闰日、跨年、前导零不适用：无对应输入解析接口。

## 交付范围

`git diff --check` 通过。仅修改 integration.rs 的一个参数类型及本轮任务记录；M-01 保持待审，其他里程碑不变。没有修改协议、金样、既有测试、其他 worktree，也未 rebase 或推送。提交使用 `NO_CI=1` 避免提交钩子启动用户禁止本轮运行的全量门禁。
