# M-03 · 实现 #3 · 待审

## 范围与实现

- 方案 §4、§5：以持久化 `session.timeout` 事件识别超时，跳过模型不存在及限流分类，沿用计次、退避与失败迁移。
- 新增 `Inject::SessionTimedOut`、`timed_out`、`effective`；连续超时替换上一段说明，保留最初注入。决定领取和修复守卫读取有效内层。
- 提示词先说明超时与最后 40 行可读日志，再渲染原注入；替换门禁修复里的轮数递增指令。Intake 也渲染注入，原无注入金样不变。
- `next_round` 排除超时行已在 M-02 实现，本轮以真实 rebase 冲突验证逻辑轮次及预算。
- 上一轮审计 M-02 通过，没有退回缺陷，因此无须迁入审计判别检查，也不把本轮调试冒充审计红绿回归。

## 验收命令与结果

原契约完整执行，退出码 0：

```sh
cargo test -p autome-domain timeout_retry && cargo test -p automed --lib timeout_retry && cargo test -p automed --test prompt_rendering timeout_retry && cargo test -p automed --test end_to_end timeout_retry && cargo test -p automed --test end_to_end -- paused_running_session paused_then_stopped a_rate_limited_round a_non_zero_exit model_not_found && test "$(git diff "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs | grep -c '^-[^-]')" = 0
```

| 输出文件（均在 `.autome/output/`） | 用例总数 | 结果 |
|---|---:|---|
| m03-final-domain.txt | 2 | 全通过 |
| m03-final-lib.txt | 6 | 全通过，11.42 秒 |
| m03-final-prompts.txt | 3 | 金样逐字节相等 |
| m03-final-e2e.txt | 2 | 全通过，合计 1.63 秒；全程用例内部断言 < 30 秒 |
| m03-final-regression.txt | 5 | 全通过，8.07 秒 |

最后的原行不变检查成立：相对 main 分叉点，`end_to_end.rs` 删除/修改原行数为 0。
直接相关额外命令 `cargo test -p automed --test prompt_rendering`：34 个通过（m03-final-all-prompts.txt），覆盖原 Intake、各角色、Bug、快检和 before_merge 提示词金样。`git diff --check` 通过。仅对修改的 Rust 文件/新增 e2e 尾部运行 rustfmt；未运行宿主快检和全量门禁。

**数量核对：**契约总数 18，相同过滤条件的基线为 5 个旧回归 + 0 个 timeout_retry，用例净增 13（domain 2、lib 6、金样 3、e2e 2）。上一轮 M-02 契约总数 15，本轮多 3：移出 10 个 M-02 新用例及 2 个 stop 回归，纳入 13 个 M-03 新用例及 2 个 pause 回归，其余 3 个旧回归保留。含提示词扩展回归共执行 52 个用例实例，去重 3 个金样后 **49 个不同用例**（13 新增、36 既有）；相比上一轮实跑的 81 个不同用例少 32，原因是本轮不再选取 store/session/invalid-pid 等上一轮子集，改选完整提示词回归，不是删除测试。最终失败 0、ignored 0；过滤项不计作执行。

## 修复与验证过程

- 首次编译发现 `gate` 模块限定名和不存在的 `SessionPaths::prompt`，已修正为现有模块及提示词文件路径；渲染提取后的 match 语法修正后编译通过。
- 首次 e2e 两条均红（m03-e2e.txt）：等待 tick 日志超时。保留临时夹具复查（m03-e2e-diagnostic.txt），日志出现测试程序的 `running 0 tests`、wrapper 退出 141。原因是库集成测试中 `current_exe()` 返回测试 harness，launcher 把它传作 renderer；它不读取 CLI 流，持续输出随后 SIGPIPE，根本没有到达超时条件。
- 只在新增 e2e 临时夹具中将 wrapper 的 renderer 实参绑定为 Cargo 构建的真实 `automed` 可执行文件。其余 wrapper 正文不变，真实进程组、git worktree、SQLite、调度及终止路径都保留，仅模型 CLI 是替身。生产没有新增开关，既有 e2e 未改。修正后两条绿（m03-e2e-v2.txt，1.99 秒）；最终原契约复跑仍绿（1.63 秒）。
- 金样仅用 `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering timeout_retry` 创建三个新文件，检查了超时段、日志、轮数约束与 Intake 卡片/附件；随后关闭更新模式执行字节比较。旧金样未改。

## 标“待审”之前的四项自审

### 1. 情形表逐行

| 方案/契约情形 | 检查与结果 |
|---|---|
| ① GateFailed 原样携带、连续超时不嵌套、JSON 往返 | domain `timeout_retry_preserves_carried_and_replaces_previous_timeout` 通过 |
| ② Decisions、空 carried、非包装 effective | domain `timeout_retry_effective_preserves_decisions_and_empty` 通过 |
| ③ 超时遇限流仍计次、不冷却 | lib `timeout_retry_counts_despite_stream_failures_and_bounds_log_tail`，rate_limited=false、attempt=1、无 cooldown；另以模型不存在记录对照，通过 |
| ④ 用尽失败、detail 含上限与超时、非协议失败 | lib `timeout_retry_exhaustion_fails_without_protocol_failure`，零重试边界通过 |
| ⑤ 超时段先于门禁段，各一次，不再加一 | launcher `timeout_retry_renders_timeout_then_gate_without_increment` 通过 |
| ⑥ 重派仍为第 1 轮、next_round=2、rebase 修复第 2 轮 | lib `timeout_retry_preserves_round_and_rebase_budget`，N=3 及真实 Git 冲突通过 |
| ⑦ 决定释放后重新领取、prompt 有正文 | lib `timeout_retry_reclaims_decisions_for_restarted_session`，两次 carried_by 指向各自会话，通过 |
| ⑧ session.started 中包装 GateFailed 获得修复豁免 | lib `timeout_retry_gate_repair_guard_reads_effective_injection`，顶层真、包装真、空内层假，通过 |
| ⑨⑩⑪ 三个新金样与既有无注入 Intake | 新金样 3 个及全部 prompt_rendering 34 个通过 |
| ⑫ 两次超时、一次重试、用尽失败、日志与 30 秒约束 | e2e `timeout_retry_full_lifecycle_exhausts_in_under_thirty_seconds` 通过；两行均 timed_out、无 protocol_failure、两次原 wrapper PID 均已退出 |
| ⑬ 暂停挂起、恢复后重派保留说明 | e2e `timeout_retry_pause_holds_result_and_resume_keeps_timeout_prompt` 通过 |

### 2. 失败分支

- 超时终止后计次及用尽：上述 lib 零次数和 e2e 一次重试用尽均通过。
- 流日志存在限流/模型不存在错误：同一 lib 用例两种输入均通过，不生成冷却/配置失败。
- 可读日志读盘失败：移除日志后重派仍成功，tail 为空；lib ③内检查通过。空 carried：domain ②、守卫⑧及普通超时金样通过。
- 无超时的普通崩溃、模型不存在、限流：原 e2e 3 个分别保持原有重试/配置失败/冷却行为，通过。
- 拒绝授权、外部服务不可达：不适用，本里程碑未引入授权或外部网络调用；模型 CLI 用本地替身。

### 3. 全部迁移

- Active + 超时、剩余次数 > 0 → 原节点 pending retry → 新会话：lib ③⑥⑦与 e2e ⑫通过；重派前后 round 都为 1。
- Active + 超时、次数耗尽 → Failed(SessionCrashed)，无协议失败：lib ④、e2e ⑫通过。
- Paused + 超时 → outcome_held、无运行会话且无 pending retry；Resume → 原节点计次重派：e2e ⑬通过。
- 既有 Paused + 正常继续、Paused → Stop、非超时限流/崩溃/模型错误：契约旧回归 5 个通过。
- 原注入 → 超时包装 → 再次超时替换：domain ①通过；包装内决定的领取与释放及门禁修复守卫分别由⑦⑧覆盖。
- Bug 的 fix_round、Requeued 的挂起、终态 detached_retro：本轮不新增或修改这些迁移，沿用共享 schedule_retry 和既有调度路径；不声称本轮另有这些通道的端到端证据。

### 4. 输入域边界

- minutes=5 与 u32::MAX、空日志字符串、单行日志：domain ①②和 launcher ⑤通过；配置最小值校验属已关闭 M-01，本轮未改。
- 41 行可读日志截为恰好最后 40 行（line-2 至 line-41）、缺失文件变空尾：lib ③通过；继续沿用 gate::tail 的既有 UTF-8/6000 字节截断实现，未新增截断算法。
- carried=None / Decisions / GateFailed / 已包装的 GateFailed，均覆盖；JSON 往返通过。
- retry attempts=0、attempts=1、backoff_seconds=0，均覆盖；实现预算 N=3，第 1 轮超时续做、第 2 轮冲突修复均通过。
- 日期、前导零等文本输入域：不适用，本里程碑没有新增这些输入解析器。

## 边界与交付

本轮只推进 M-03，未实施 M-04，未启动后续会话。真实模型人工验收 H-01/H-02 及宿主快检/合并前全量门禁仍由相应后续环节执行。
