# M-01 r2 审计复验

## 验收命令
`cargo test -p automed --lib integration_range` → 17 通过 / 0 失败，与契约预期 17 一致（基线 0）。`cargo test -p automed --lib integration` 同为 17。
`cargo fmt --all -- --check` 退出 0；`cargo clippy --all-targets -- -D warnings` 退出 0。

## 负向对照（逐项注入缺陷、跑验收命令、还原；最终 `git status` 干净）
| 注入 | 结果 |
|---|---|
| 去掉两处 `--cherry-pick` | 3 个失败：`rebase_lists_only_rewritten_and_new_commits`、`clean_rebase_is_empty`、`conflict_file_commit_with_an_equivalent_patch_id_is_still_listed` |
| 不按 `<doc_dir>/` 过滤路径 | `doc_only_commits_are_not_integration` 失败 |
| 去掉冲突文件保守纳入 | `equivalent_patch_id_is_still_listed` 失败 |
| 判定只看「列出」不看「消失」 | `conflict_that_drops_the_product_commit_is_still_integration` 失败 |
| 内联测试模块内容扫描置空 | `inline_test_module_with_only_setup_changed_is_listed` 失败 |
| stat 读取失败改为传播错误 | `a_stat_failure_keeps_the_range` 失败 |
| H0 取最后一次（重试）尝试而非第一次 | `a_crashed_attempt_keeps_its_commits` 失败 |
全部 7 项注入都被验收命令拦下，验收有判别力。

## 其它核对
- 调度器回归 `scheduler::tests::integration_range_read_failure_stops_task_before_audit` 在 17 个里通过。
- 代码核对：`range` 要求 Impl 会话、轮前与轮后表均非空全 `已完成`；`start_heads` 缺失返回 None（不猜）；读取错误向上传播（由调度器转 Failed(Config)）。与设计 §1–§3 一致。
- 本轮新造判别检查：无需新增，现有用例已覆盖上述每个分支。

## 结论
通过。残余：B-07（新增/删除文件被当作测试文件多列，符合「宁可多列」，进 Backlog）。
