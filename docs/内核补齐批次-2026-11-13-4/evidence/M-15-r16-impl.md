# M-15 · 实现 #16（16/80）

结论：G-R2/G-V2、H-E1/H-E2/H-T1 已实现并取得验收通过证据，交独立审计；不标已完成。上一轮审计 #14 通过 M-14，没有本里程碑退回项或需逐字搬入的审计检查。

## 实现与修复

- schema 仅递增一次：7 → 8。重建 sessions，任务外键可空，新增项目外键；约束 NULL task_id 只用于 onboarding，且必须有项目。保留原列、任务索引与数据，新增项目索引。Session 以可选 project_id 承载归属，NULL task_id 读为既有 onboarding 伪键；既有任务会话的序列化不增加空字段。
- Onboarding 启动写会话行和项目 subject 的 session.started；按项目根下的既有日志/退出标记位置收割，复用 session.ended 载荷，不执行任务 record_usage 或推进任务。会话运行上限在启动前解析并冻结，超时确认进程组退出后结束。
- session.log 直接按会话 id 查账本，再按 kind 分流项目归属。project.get 新增 onboarding_session（id/lifecycle/started_at/ended_at），无历史时为 null，同秒按 rowid 选最新。
- Onboarding 结束先提交后写状态及事件。单仓库保持原路径；工作区按产物实际路径映射到成员，只提交初始化白名单，不在根运行 Git，无成员产物返回空提交。失败为可读拒绝，保留状态，解除索引锁后可重试。
- 技术设计 §10/§13/§14/风险项同步。其它 Session 夹具只补 project_id: None；旧迁移夹具先还原 v7 sessions 结构再模拟 v4/v5/v6，保留原断言（版本号改为当前 SCHEMA_VERSION）。

## 命令、红绿与数量

所有原始输出在 `.autome/output/`，未写入任务目录；测试全部使用临时账本/项目，没有调用真实用户账本。

| 命令 | 结果与证据 |
|---|---|
| `cargo test -p automed workspace_onboarding_commit_`（产品修复前） | 4 个全部失败：工作区根 Git 操作失败、工作区与单仓库索引锁失败后状态已结束。原始输出 `r16-h-red.txt`；回归测试独立提交 `919c226e`，不含产品修复。 |
| `cargo fmt --all -- --check`（回归测试提交前） | 退出 0，`r16-fmt-check.txt`。 |
| `cargo test -p automed workspace_onboarding_commit_`（最终修复后） | 同一命令 4 个全过，`r16-h-green-final.txt`；后续加强原四例内部的无产物、失败事件、advance/skip 与重复结束断言，没有减少原断言。 |
| `cargo test -p automed onboarding` | **18 个全过**：库 16 + end_to_end 1 + prompt_rendering 1，0 失败/跳过。`r16-green-final.txt`。 |
| `cargo test -p automed --lib -- store::tests session_ended_` | 60 个全过，`r16-related-final.txt`。 |
| `cargo test -p automed --test end_to_end session_ended_` | 4 个全过，`r16-ended-e2e.txt`。 |
| `git diff --check` | 退出 0。 |

同范围基线：此前 onboarding 匹配 7 个（既有库 6 + 金样 1），本轮 18，**增加 11**：契约列出的 G 6、H 4，以及补足 G-V2 的真实包装脚本 e2e 1。原契约的 16 漏算既有金样；已将预期更正为 18。相关库子集原 59、本轮 60（新增迁移 1）；session.ended e2e 仍 4。主验收与库子集重叠 2 个 store onboarding 用例，H 子集均已包含，故本轮最终验证 **80 个不同用例，相对同范围原基线 69 增加 11**。上一轮 M-14 的 7 个是不同验收范围，不能把本轮扩大子集误报为新增测试。

缺陷注入（均已还原）：
- 删除 start_onboarding 的 insert_session 后，`cargo test -p automed onboarding_session_log_is_readable` 退出 101，1 个失败，账本行断言捕获缺失；`r16-negative-no-row.txt`。
- 把状态更新移回提交之前，`cargo test -p automed -- workspace_onboarding_commit_failure_keeps_state_and_can_retry workspace_onboarding_commit_single_repo_index_lock_keeps_state` 退出 101，2 个失败；`r16-negative-state-first.txt`。
- 在工作区根提交的反例由修复前 H 四例的红灯覆盖。

## 标待审前自审清单（逐项执行）

### 1. 情形表逐行

| 设计情形 | 检查与结果 |
|---|---|
| 启动记账与项目归属 | writes_a_row_and_start_event：项目 id、伪键、round=0、运行态、开始事件通过；e2e 验证真实启动。 |
| 结束记账且不走任务收尾 | is_reaped_with_session_ended 覆盖退出码 0/17、消失、超时；e2e 断言调度报告不含伪任务且结束事件恰好一次。 |
| 项目根日志 | log_is_readable 在工作区项目校验中文正文及缺失日志为空；e2e 按 session id 读取真实 wrapper 日志头。 |
| 最新项目会话 | project_get_reports_latest 覆盖无会话 null、历史已结束、同秒第二次启动与固定四键；e2e 校验结束时间。 |
| 任务费用/轮次隔离 | costs_stay_out_of_tasks 给 Onboarding 写非零 999 美元，真实任务 2 美元仍不变；e2e 前后 task.get 完整回包相等且任务会话为空。 |
| v7 → v8 | migration_v8_keeps_existing_rows 保留非空费用/token、协议/规则哈希、上限、所有会话字段及索引；外键检查为空。旧 v4/v5/v6 迁移均通过相关 store 子集。 |
| 工作区 skip / finish | 两条 H 正例成功，成员提交、根无 .git、事件带提交；已有单仓库结束测试保持通过。 |
| 无产物 | skip 正例内部移除成员产物，返回 null，不新增提交；工作区根产物不提交。 |
| 提交失败与重试 | 工作区和单仓库分别对 skip/advance 加 index.lock：原状态、事件数不变，错误含重试说明；移锁后成功。 |

### 2. 失败分支

- 设计点名的 Git 提交失败：两个仓库形态 × skip/advance 均实跑，断言状态和事件不变及可重试。
- 日志读盘失败：删除日志后 session.log 返回空串，沿用既有语义；不再因伪任务而失败。
- 会话非零退出、无进程无标记、超时：分别运行 17 退出码、旧开始时间且缺失日志、独立 sleep 进程组；检查最终 lifecycle 和结束事件。超时子进程由测试创建、终止并 join/wait，无遗留。
- 拒绝授权、外部模型不可达：不适用，本里程碑无新增授权或模型网络契约；e2e 使用既有假 CLI，不声称真实模型调用验收。

### 3. 全部迁移

- Onboarding：step 3 → 4 → 5 → Completed；InProgress → Skipped；上述最终事件提交失败 → 保持原态 → 重试成功（单仓库/工作区均覆盖）；Completed/Skipped 的 advance 拒绝；Completed → Skipped、Skipped → Skipped 维持原行为且不重复提交。
- 会话：Running → 继续等待；Running → Exited(0)、Exited(17)、Vanished、TimedOut；项目 ended 事件无伪任务推进。既有任务 Exited/Crashed/Stopped/TimedOut 四条 e2e 全绿。Onboarding 无独立 stop RPC，Killed 不属于本项可触发迁移，不新增接口。
- 账本：v7 → v8 保留原行；v4/v5/v6 → v8 由旧迁移用例复验；重复 migrate 由 store 既有用例覆盖。

### 4. 域边界

- 空会话列表 → null；单会话及同秒两会话 → 正确最新项。
- SQL NULL task_id、缺 project_id、未知项目外键、把 Onboarding 填真实 task_id、把任务 task_id 置 NULL：正/反约束均检查。
- 空成员产物集、仅一个成员文件、中文日志、缺失日志、成功/非零退出边界均检查。
- 前导零、闰日、跨年、数字最大值不适用：本项未新增日期/数字用户输入域，时间解析沿用现有实现并由 session_ended_vanished_metrics_and_timestamp_boundaries 相关用例复验。

## 会话调用点核对（D1-P07）

检索结果：`.autome/output/r16-session-callers.txt`。

- scheduler::tick 是 all_running_sessions 的生产消费方：reap_session 在 get_task 前按 Onboarding 分流，tasks_advanced 排除伪任务。错误显示分支 get_task 失败已有回退为 session id，不参与任务处理。
- scheduler 的任务重试、record_usage/任务指标、恢复任务、并发槽位准入，以及 dispatch 的 stop/pause/rerun/task.get/dashboard 均从真实 TaskRecord 或 task_id 调 list_sessions/running_session；SQL `WHERE task_id = ?` 排除 NULL，天然只对任务会话。
- dispatch::project_costs 使用 sessions_in_project，查询按真实 tasks 关联 task_id，保持费用与缺失计数隔离。
- dispatch::session_log 已改成 get_session 并分流 project_id。record_session_ended 按 id 读取行，不再依赖伪 task_id 查历史。
- 测试中的 get_task(&s.task_id) 只用于既有任务夹具；未改为给伪任务创建记录。

## 限制与交接

未跑 `sh scripts/ci-local.sh`、宿主快检、真实模型调用或页面人工验收。包装脚本 e2e 使用项目既有假 CLI；其渲染器在该测试夹具中是测试二进制，所以真实日志验证取 wrapper 头，正文读取由独立单测验证。全部最终用例通过，无跳过；临时缺陷注入已还原。本轮仅推进 M-15，不启动下一会话。
