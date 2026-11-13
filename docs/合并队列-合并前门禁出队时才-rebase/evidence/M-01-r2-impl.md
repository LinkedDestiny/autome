# M-01 · 实现轮 2/25（快检修复）

本轮仅修复宿主快检退回项，不推进新里程碑；M-01 保持「待审」。起始 HEAD 为 `57dbc09d`，工作区干净，无审计文件、无审计回归检查待移植。

## 根因与修复

宿主在 `36f646f6a4` 上发现 `crates/automed/src/merge_queue.rs:82` 的 `clippy::nonminimal_bool`。将 `!option.is_some_and(kind == "merge.waiting")` 改成 `option.is_none_or(kind != "merge.waiting")`，不抑制 lint，不改变事件去重行为。检查范围为该等待事件条件及同模块；全目标 Clippy 验证同类问题。

## 命令与结果

- 红：修复前原样运行 `cargo clippy --all-targets -- -D warnings`，退出码 101，同文件同位置同 lint；日志 `.autome/output/m01-r2-clippy-red.txt`。这是宿主现有快检，不是上一轮审计新增的测试，无需复制或改写测试体系。
- 绿：指定快检 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`；全部通过，退出码 0，日志 `.autome/output/m01-r2-quickcheck.txt`。
- M-01 原验收命令：`cargo test -p autome-domain merge_queue_ && cargo test -p automed --lib merge_queue_ && cargo test -p automed --test end_to_end merge_queue_`；19 个用例全部通过（domain 4、lib 11、e2e 4），退出码 0，日志 `.autome/output/m01-r2-acceptance.txt`。
- 未运行全量门禁 `sh scripts/ci-local.sh`。最终快检是否获准以 Autome 提交后重跑为准。

## 自审清单（本轮等价表达式修复范围）

1. **情形表逐行**：等待条件的三行情形逐项核对：无匹配事件 → 写等待事件；末条为 `task.updated` → 写等待事件；末条为 `merge.waiting` → 不重复写。新旧表达式逐行等价。既有 `merge_queue_a_second_merge_gate_waits_outside_the_host_queue_while_the_section_is_held` 连续三次调度只写一条等待事件；`merge_queue_a_merge_waits_while_another_task_holds_the_section` 覆盖 Merging 等锁。两者随原验收命令执行。
2. **失败分支**：本轮不新增失败分支，`last_event_of(...)?` 的错误传播保持原样。原验收中的 `merge_queue_reuse_requires_an_up_to_date_branch` 覆盖检出及配置读取失败，`merge_queue_every_exit_releases_the_section` 覆盖门禁失败、准备失败、超时及解锁；不新增外部服务调用，外部服务不可达不适用。
3. **全部迁移**：本轮不修改转换表；原验收的 `every_exit`、持锁/离开临界区 domain 测试及四条 e2e 复验原有释放、继续、合并与重启路径。等待条件只决定是否追加事件，不转换任务状态。
4. **输入域边界**：本轮条件输入为 `Option<事件>`，None / Some 相同 kind / Some 不同 kind 三种已逐项核对；查询只筛选 `task.updated` 和 `merge.waiting`。没有新增日期、数字或字符串解析域，其它域边界不适用。

## 用例数量与交付

本轮用例总数 **19，通过 19，失败 0，忽略 0**；与上一轮同验收范围 19 个相比差 **0**，与上一轮全部执行的 46 个相比差 **-27**（仅执行范围收窄，并非测试减少）。未新增、删除或重命名测试。上一轮 M-01 验收基线为 19 个，另跑既有队列及 requested_rebase 子集 27 个，总计 46 个；本轮仅复验原验收范围，不重复该额外 27 个。静态快检不计入测试用例数。

`git diff --check` 通过。四项自审及其引用的原验收测试均已完成；未推进 M-02～M-05，未运行全量门禁，未改配置或协议。
