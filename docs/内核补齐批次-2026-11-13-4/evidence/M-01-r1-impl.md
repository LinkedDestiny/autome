# M-01 实现 #1：搬入 T-69 越界自愈

## 范围与来源

基线 `360c757e`，当前分支内按以下顺序对每个来源提交执行 `git diff <commit>^ <commit> -- crates`、`git apply --check`、`git apply`；全部无冲突。没有搬旧任务文档，README 与技术设计按设计留给 M-02。

| 来源提交 | 搬入内容 |
|---|---|
| bd887a251d88863a1bfd3583ebb86d947a0c6c74 | 路径还原、持久化计划、正向补偿及测试 |
| b783930fc52712eec984e5d4f45edbae5653db06 | 域补丁队列与实现 / 修复轮迁移 |
| 1c500579b7786dbe418953053f1fb680ee730f29 | 需求审计 / 复盘还原、简报交接、账本与 e2e |
| 969a4d04bc4df0096ef5681fc9c276c18f62f95f | prompt fixture 新字段 |
| 81d0534e6453f53574bb3b7206d546f0d840af70 | 同角色连续越界熔断、失败回退 |
| 82bd940398dbc2f69643bc145d44c76dbe56cb21 | Bug 复核向修复轮交接 |
| adb0537c283998df1276ee6f0cbc2e8dce8ecf8c | CLI 与 task.get 展示（仅 crates） |
| f52f3df655e54208dadbe220fe48e853c0b4ecd4 | 集成复验与补丁交接 brief 共存 |
| 41d63e663160fb855039a52b3405784d07e62e50 | integration brief fixture 新字段 |

逐文件执行 `git show refs/keep/t69-scope-self-heal:<path> | cmp -s - <path>`，15 个产品文件（含两份新增 scope.rs）全部一致。未改来源测试断言，无必要冲突修复。原始补丁、提交清单、测试输出在 `.autome/output/`，不作为任务文档正文。

## 验收命令与基线

```sh
cargo test -p autome-domain scope_patch
cargo test -p automed --lib -- scope::tests::restore_ session_scope_ scope_patch_ task_get_scope_restore
cargo test -p automed --test end_to_end -- scope_restore_ scope_breaker_ review_scope_
cargo test -p autome-cli scope_
```

本任务没有上一轮实现 / 审计测试基线。设计给定来源基线为 52 个；本轮目标为 52，差 0。相对搬入前 HEAD 同过滤范围的静态测试清单为 8 个（库 2、e2e 6），净增 44：域 +13、库 +17、e2e +10、CLI +4；这个 8 是源码清点，不冒称上一轮实跑结果。过滤掉的其它测试不计入本次用例总数。

## 标待审前自审清单

### 1. 情形表逐行

本里程碑没有单独情形表，逐行对照 K-R1–R5、K-V1–V6 与 M-01 验收契约：

| 情形 | 执行检查（属于上述验收命令） |
|---|---|
| K-R1 / V5 修改、新增、删除，文档保留 | restore_reverts_modified_added_and_deleted_paths_and_keeps_docs；restore_patch_reapplies_to_the_session_result；restore_commit_message_names_session_base_paths_and_patch |
| K-R1(c) 事件 / CLI / RPC | task_get_scope_restore 与 CLI scope_ 四例 |
| K-R2 / V1 普通审计还原后继续、交给下个实现轮 | scope_restore_feature_audit_edit_is_reverted_and_handed_to_next_impl；review_scope_ 六例保留其它文档守卫 |
| K-R3 / V2 最后里程碑补实现轮再审计合并 | scope_restore_feature_last_milestone_adds_an_impl_round_then_reaches_merge |
| K-R3 复盘交接 | scope_restore_feature_retro_edit_routes_through_impl_and_audit；session_scope_manual_retro_first_crossing_is_restored |
| K-R3 / V3 Bug 复核交接 | scope_restore_bug_review_crossing_returns_to_fix_with_patch |
| K-R4 / V4 连续越界熔断与干净轮重置 | scope_breaker_second_consecutive_audit_crossing_fails_with_note；scope_breaker_clean_audit_between_crossings_resets_the_streak；scope_restore_bug_second_consecutive_review_crossing_fails；session_scope_manual_retro_second_crossing_fails |
| K-R5 / V6 失败回退 | scope_breaker_restore_failure_falls_back_to_todays_failure；restore_workspace_failure_in_one_repo_compensates_the_other |

K-R6 文档和 K-R3 与 T-68 交接标签 / 补丁轮说明句明确归 M-02，不在本轮宣称完成。

### 2. 失败分支

- 未提交改动阻挡：restore_refuses_a_dirty_worktree_and_leaves_it_untouched，验证拒绝且原状态保留。
- 补丁目录不可写 / 读盘失败（补丁被删除）/ HEAD 已推进：restore_patch_dir_blocked_changes_nothing 含三条检查。
- 多仓库后一个补丁无效：restore_workspace_failure_in_one_repo_compensates_the_other 验证正向补偿和原树恢复。
- 还原失败的任务协议失败回退：scope_breaker_restore_failure_falls_back_to_todays_failure。
- 修复预算耗尽：scope_patch_passed_review_obeys_fix_budget 与 scope_restore_bug_review_crossing_with_spent_fix_budget_fails_fix_budget。
- 授权拒绝、网络服务不可达、网络超时不适用：此里程碑是本地 Git / 账本恢复，没有这些接口。空输入见域边界。

### 3. 全部迁移

- Audit + 有补丁 + 全部完成 → Impl（含预算耗尽、关闭 retro）；scope_patch_last_audit_ 三例。
- Audit + 无补丁 → Retro / Rebase；scope_patch_clean_audit_keeps_retro_or_rebase。
- Audit + 开放里程碑仍受实现预算限制；scope_patch_open_milestone_still_obeys_budget。
- Retro + 有补丁 → Impl；无补丁 → Rebase；scope_patch_retro_starts_implementation、scope_patch_clean_retro_rebases。
- FixReview + 通过 + 有补丁 → Fix；预算耗尽 → 失败；无补丁 → Rebase；bug_loop 的 scope_patch_ 三例。
- 首次越界 → begun → restored；失败 → failed 并补偿；连续同角色越界 → 协议失败；干净同角色结束 → 熔断清零；对应上表测试。
- 持久化 begun 尚未结束 → 恢复；首仓库已提交 / 提交前中断 → 幂等完成：scope_patch_restore_resumes_unfinished_begun 与 restore_apply_resumes_ 两例。
- Impl Ok → 仅标记已呈现补丁 handled；崩溃 → 下一轮再次呈现；scope_patch_ok_impl_marks_only_presented_patches_handled、scope_patch_crashed_impl_is_presented_again_on_next_launch。域 pending 两例验证失败 / 已处理排除及账本顺序。

### 4. 输入域边界

- 空仓库集 / 无越界路径：restore_without_outside_paths_does_nothing；空 pending：scope_patch_pending_empty_after_all_handled_or_failed。
- 单仓库 / 双仓库：restore_ 单仓库夹具与 restore_workspace_writes_one_patch_per_repo。
- 空格、中文、glob、Git magic、前导短横线、换行、相邻目录前缀：restore_handles_spaces_non_ascii_and_glob_characters；重命名两端及目录边界：session_scope_checks_both_rename_paths_and_directory_boundary。
- 二进制与 executable mode：restore_round_trips_binary_and_mode_changes。
- 预算边界见全部迁移。前导零、闰日、跨年不适用：没有日期或数字字符串解析输入域。

## 审计回归与限制

无上一轮审计退回，无需搬入审计检查或补红绿回归。原来源测试原样搬入。未执行项目全量门禁或宿主快检；遵从本轮指令，由宿主运行。没有修改协议副本、真实账本、其它 worktree，也没有启动下一会话。

## 最终结果

四段验收退出码均为 0，分别 13 / 19 / 16 / 4，共 **52 个通过、0 失败、0 忽略**；上述四项自审所列检查均在这 52 个中执行通过。原始日志：`.autome/output/m01-domain.txt`、`m01-lib.txt`、`m01-e2e.txt`、`m01-cli.txt`。

额外直接相关检查 `cargo test -p automed --test prompt_rendering`：40 个通过、0 失败、0 忽略（`.autome/output/m01-prompt.txt`），没有更新金样。其用例数量未变，只补齐来源提交要求的 fixture 字段。合计本轮实际执行 **92 个不同用例**；相对来源同范围 92 个差 0，相对搬入前同范围静态清单 48 个净增 44。`git diff --check` 通过。

结论：M-01 待审。M-02 及其它里程碑保持开放，未提前推进。
