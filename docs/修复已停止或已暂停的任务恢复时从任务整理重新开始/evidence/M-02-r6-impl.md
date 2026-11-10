# M-02 · 实现轮 6/25

## 范围与修复

- 本轮只推进最小开放里程碑 M-02。上一轮 M-01 审计通过，无退回缺陷，因此没有需要逐字搬入的审计失败检查。
- `advance` 在 Pause/Stop 的成功转换清除 retry 前，将同节点待启动 inject 写入 `task.resume_input`。
- `Store::resume_input` 按任务、节点和事件序号选择留存输入，必须晚于 latest 会话的启动事件；`Reenter` 优先使用它，再取对应会话的原输入。显式空 inject 不回退到旧输入。未改 schema。
- 新增 1 个调度用例（内部遍历 Pause、Stop）和 5 个真实 e2e，用真实 Git/worktree/包装脚本，只替换模型；文件信号放行，等待上限 90 秒。成功后停止测试会话；脚本也在测试目录移除时结束。
- 用户纳入 B-01、B-03，已加入设计的 M-04、M-05。B-03 明确记录为任务 §4 的授权例外；本轮不跨里程碑实现。

## 验收与计数

- `cargo test -p automed --lib requeued`：7 个通过，相比上一轮 6 个增加 1 个待启动输入用例；日志 `.autome/output/m02-lib.txt`。
- `cargo test -p automed --test end_to_end resumes_at`：5 个通过，相比上一轮 0 个增加 5 个停止恢复 e2e；日志 `.autome/output/m02-e2e.txt`。
- `cargo test --workspace`：901 个通过，相比上一轮 895 个增加 6 个（lib +1、e2e +5）；分层为 CLI 8+4、domain 288、automed lib 541、e2e 43、prompt_rendering 10、real_cli 7，0 失败、0 忽略。日志 `.autome/output/m02-workspace.txt`。
- 原有 `stopping_a_task_leaves_it_resumable` 未改，全量 e2e 43 个中通过。M-02 原定的 lib 7、过滤 e2e 5、全量 e2e 43 与实际一致。
- `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check`：通过；日志 `.autome/output/m02-clippy.txt`、`.autome/output/m02-fmt-check.txt`。
- 开发时 e2e 首次编译因测试调用 `git::head` 而失败，改为实际 API `head_sha`；门禁首次检查因测试用了完整 SHA 而非作业的 12 位 key 失败，修正夹具后通过。未修改生产门禁行为来迁就测试。

## 标待审之前的自审清单

1. **情形表逐行**
   - R1 原会话仍运行：既有 `requeued_waits_for_slot_then_reenters_and_rejects_stop_without_killing` 通过，同一会话不重复启动。
   - R2 暂缓结论：不适用，本轮是停止重入；设计明确由 M-03 实现暂缓结论。
   - R3 停止重起：实现、审计 e2e 断言恢复节点和新会话 kind，并断言任务整理会话始终仅 1 个。
   - inject 来源 1：新 lib 用例验证 Pause/Stop 后 retry 已清、新会话带 GateFailed；来源 2：门禁打回 e2e 验证新 prompt 包含旧门禁输出；来源 3：无输入的实现重入 e2e 通过。
   - 停止门禁：伪造完整合法 143 标记，恢复后文件被清除，gate.started 从 1 变 2，成功进入 Audit；既有 lib 验证不会清除 merge_gate，Paused 不清除作业。
   - 槽满：并行 1，B 正在运行时恢复 A，JSON 为 queued+resume=implement、队列位置 1、会话数不增；停止 B 后 A 回 Implement。
2. **失败分支**
   - 门禁失败输入：真实失败门禁输出进入实现 prompt；停止恢复后仍保留该输出。
   - 被杀门禁伪结果：完整 143 标记不得被采纳，e2e 通过；门禁 pid 尚未写入的等待/清理边界由既有 requeued 门禁用例通过。
   - 角色关闭拒绝恢复、排队后关闭角色转 Failed：domain 全量中的两条 requeued 用例通过；SAME-MODEL 拒绝由原有全量 e2e 通过。
   - 空输入：新 lib 验证显式 None 覆盖旧 GateFailed，不回退；无历史会话路径由既有 lib 覆盖。
   - 读盘失败、外部服务不可达：不适用，M-02 未设计独立的此类失败分支，现有存储/启动错误传播不改；真实模型服务未调用。
3. **全部迁移**
   - Active(Implement/Audit/Gate) → Stop → Stopped → Resume → Requeued → SlotAvailable → 原节点：五条 e2e 分别覆盖直接出队与槽满等待。
   - 待启动重试 → Pause 或 Stop → 清 retry → Resume → Requeued → Reenter：新增 lib 内两种触发均通过。
   - 全 13 节点出队、三个门禁清理动作、Requeued 拒绝 Pause/Stop/Resume/RerunFrom/ExtendBudget 与允许 Cancel：既有 domain 全量通过。本轮未扩展状态机。
   - 暂停结果暂缓、手动复盘来源互斥：不适用，M-03 范围，未宣称完成。
4. **输入域边界**
   - inject 有值/None、没有历史会话、有旧会话、新会话使旧留存失效、节点不匹配：新增与既有 lib 检查通过。
   - 并行上限 1、刚好槽满、释放一个槽位：槽满 e2e 通过。
   - 门禁旧作业存在、其他节点作业保留、无 pid 的等待超时：既有门禁/lib 用例通过。
   - 日期前导零、闰日、跨年、数值最大值：不适用，本轮没有新增日期/数字解析输入域。

## 负向对照

逐个注入设计指定错误，每次恢复源码后再注入下一项；命令输出保存在 `.autome/output/m02-negative-*.txt`。

- Reenter 传 None：`cargo test -p automed --test end_to_end stopped_gate_feedback_resumes_at` 实跑为红（退出码 101），prompt 缺失 unique-resume-gate-feedback。
- 不执行 DiscardGateRuns：`cargo test -p automed --test end_to_end stopped_gate_resumes_at` 实跑为红（退出码 101），143 旧标记仍存在。
- Requeued 绕过并行限制：`cargo test -p automed --test end_to_end stopped_task_resumes_at` 实跑为红（退出码 101），恢复 A 后立即 active 而非 queued。
- Pause/Stop 不留存输入：`cargo test -p automed --lib requeued_preserves_pending_input` 实跑为红（退出码 101），实际输入 None 而非 GateFailed。

四项均因行为断言失败（不是编译失败）。逐项还原后重新跑 M-02 完整验收、工作区全量、clippy 与 fmt，作为最终绿证据。

## 最终结论

四条负向对照全部红，源码还原后的最终验收串退出码 0。全量 e2e 独立命令为 43/43（`.autome/output/m02-e2e-full.txt`）；最终工作区仍为 901/901（较前轮 +6），clippy 与 fmt 通过。M-02 标为待审；实现轮为 6/25；M-03、M-04、M-05 保持开放。H-01 至 H-03 桌面人工项本轮未取证；宿主 `sh scripts/ci-local.sh` 留给 Autome 门禁。
