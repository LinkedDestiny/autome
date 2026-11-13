# 裁决记录：单个会话的时长上限

## 裁决 #1（设计轮次 d=1）

评审结论：设计评审 #1 提出 5 项阻塞问题（基线 `9759021`）。本轮逐条用源码核对，**5 项全部成立**：4 项采纳、1 项部分采纳（范围裁定）。设计已按裁决修订，`design-round` 由 0 加到 1。修订后的内容（终止确认、`next_round` 排除超时、`Inject::effective`、Intake 注入路径）尚未经独立评审，所以 `status` 保持 `设计中`，交下一轮评审复核。

核对用到的源码位置（均已亲自读过）：`scheduler.rs` 的 `read_pid`（40×50ms 后返回 `None`）、`reap_session`、`classify`（`pid` 为 `None` 时 `alive=false`，日志在动则 `Running`）、`launch_session`（`next_round`）、`start_session`（决定领取）、`repairing` 判定、`start_onboarding`；`store.rs` 的 `next_round`；`launcher.rs` 的 `build_prompt`（Intake/Onboarding 提前返回）、`stop_session`。

### D1-P01 未完成进程组终止就结束会话并重派

- 主张：pid 缺失、终止出错、或恢复时凭 `session.timeout` 事件收割，都会在进程组未确认停止时 `sweep_commit` 并重派，旧进程继续写工作树。
- 设计位置：方案 §3 第 1、3 步，§4「进程组已经没了」。
- 裁决：**采纳**。
- 证据：`read_pid` 只等 2 秒后返回 `None` 并登记；`classify` 在 `pid=None` 且日志在动时判 `Running`，原设计「`pid=None` 跳过终止」后仍继续收割，属实。恢复路径中 wrapper 的 TERM 处理只写标记并退出，标记存在即 `Exited`，原设计据事件直接收割而不再终止，属实。
- 修改落点：设计文档 §3——超时拆为「取 pid（`None` 时再读一次并 `set_session_pid` 补写，仍无则 `session.timeout_unconfirmed`、保持 `Running`）→ 记事件、`launcher::terminate_group` → 确认」；`stop_session` 抽出返回 `bool` 的 `terminate_group`（TERM、轮询 3 秒、KILL、再轮询 2 秒）；恢复路径先再次终止并确认；未确认不 `finish_session`、不 `sweep_commit`、不重派，下个 tick 重试。风险节与 M-02（新增 ⑨ 恢复、⑩ 取不到 pid；用例 13 → 15）同步。
- 复提计数：0。

### D1-P02 沿用重派路径会消耗实现轮数预算

- 主张：`next_round` 对同角色所有会话取 `MAX(round)+1`，超时重派占下一个轮号，导致预算行与 rebase 冲突后的 `ImplBudget` 判断提前耗尽；注入文案还有「照常加一」冲突。
- 设计位置：方案 §4「不动轮数」、§5 文案、风险节。
- 裁决：**采纳**。
- 证据：`store.rs` `next_round` 的 SQL 不区分生命周期；`launch_session` 用它做预算行的 `round`；rebase 路径 `next > limit` 得 `ImplBudget`；`GateFailed` 文案含「implementation-round 照常加一」。原设计只论证「内核没有显式加轮数」，漏了这条隐式路径，属于 C-5 被违反的事实错误。
- 修改落点：设计文档 §4 新增「物理会话次数 ≠ 逻辑轮次」——超时终止的会话不占轮号，重派沿用其轮号，落点只在 `next_round` 的查询排除 `timed_out` 会话，所有调用点一并受益；§5 渲染内层注入时把「照常加一」替换为「以本 prompt 开头给出的数为准」；设计轮的 `design-round` 无账本侧数字，留给 H-01。M-03 新增轮号用例（⑥）与「照常加一」被替换的断言。
- 复提计数：0。

### D1-P03 包装 carried 注入后丢失调度语义

- 主张：决定领取与 `repairing` 守卫豁免只认顶层变体，包装后决定为空、合法修复轮被判 `impl_had_no_target`。
- 设计位置：目标 4、方案 §5。
- 裁决：**采纳**。
- 证据：`start_session` 的 `matches!(inject, Some(Decisions | DesignFeedback{..}))` 与 `repairing: carried_inject(..).is_some_and(GateFailed)` 均为顶层匹配；`carried_inject` 读的是重派会话自己的 `session.started` 事件，即包装后的值。全库 `Inject::`/`spec.inject` 的生产读者只有这两处加 `build_prompt` 的渲染，已逐一核对（`hands_decisions_to_a_session` 匹配状态机产出的 `Action`，重派不经过）。
- 修改落点：设计文档 §5 增读者表与统一入口 `Inject::effective()`；决定领取、`repairing` 改读它；渲染对内层递归。M-03 新增 `effective` 单测（②）、决定重领（⑦）、修复守卫（⑧）。
- 复提计数：0。

### D1-P04 Onboarding 不在会话登记与检查链中

- 主张：`start_onboarding` 不登记会话行，tick 看不到它，设计声称的覆盖不成立。
- 设计位置：背景、方案 §2、§3 覆盖表。
- 裁决：**部分采纳**——事实成立，修正错误陈述；处理上选「排除」而非「补跟踪」。
- 证据：`start_onboarding` 只调 `launcher::launch` 并追加事件；`all_running_sessions` 读会话表；`reap_session` 首行要真实任务。任务文件开放问题 7 只要求给 Onboarding「确定行为」，未要求必须有上限；它是用户在可见终端盯着的交互向导，无任务节点可重派，R-3 对它无意义，补跟踪需要新增向导会话登记与项目级归属，超出本任务目标与范围。
- 修改落点：设计文档背景改正陈述；§3 覆盖表 Onboarding 行改为「不覆盖」并写明范围裁定；非目标加一条；Backlog `D1-B01`。
- 复提计数：0。

### D1-P05 Intake 提前返回绕过超时注入

- 主张：`build_prompt` 对 `Intake` 直接 `return intake_prompt(spec)`，不经注入段，任务整理重派收不到超时说明。
- 设计位置：方案 §3 覆盖表、§5。
- 裁决：**采纳**。
- 证据：`launcher.rs` `build_prompt` 中 `SessionKind::Intake => return intake_prompt(spec)` 早于 `match spec.inject`；`intake_prompt` 不读 `spec.inject`。
- 修改落点：设计文档 §5：`Intake` 取 `intake_prompt` 结果后只执行 `render_inject`（不插技能行，`inject` 为 `None` 时与今天逐字节相同），`Onboarding` 保持提前返回；新增金样 `intake-timeout.md`（⑪）。
- 复提计数：0。

## 定稿判断

不定稿，`status` 保持 `设计中`。五项都已按证据修订，但 P01 的终止确认语义、P02 的 `next_round` 查询、P03 的 `effective` 入口、P05 的 Intake 路径是本轮新增设计，需要评审 #2 独立核对是否引入新的六类问题。无争议项（所有复提计数为 0）。

## 提交

提交由本轮尝试；如被拒绝则由 Autome 兜底。

## 裁决 #2（设计轮次 d=2）

评审结论：设计评审 #2（基线 `43cfac1`）提出 1 项阻塞问题 D2-P01（内部逻辑矛盾），并确认 D1-P01～P05 的修订落点无新反例。本轮逐条核对源码，**D2-P01 成立，采纳**。`design-round` 由 1 加到 2。

核对用到的源码：`crates/autome-domain/src/session.rs` 的 `classify`（无标记、pid 不活、日志静默 ≥ 600 秒 → `Vanished`）；`scheduler.rs` 的 `reap_session`（非 `Running` 直接 `finish_session`）；wrapper 的退出标记由 shell 信号处理函数写。

### D2-P01 恢复查询限定为有退出标记，与超时恢复及轮号复用冲突

- 主张：方案 §3 第 1 步一边要求「已有 `session.timeout` 事件则再终止并确认」，一边写「只发生在已有退出标记的那一次 tick」；无标记、已满足 `Vanished` 的恢复输入下例外不可达，会话被收成 `Vanished`，`next_round` 不排除它（C-5 被违反），`timed_out` 派生为假（R-5 被违反），重试用尽时失败理由缺「超时」。
- 设计位置：方案 §3 第 1 步，连带 §4「账本里的轮号」、§6 的 `timed_out` 派生；M-02 ⑨。
- 裁决：**采纳**。
- 证据：退出标记不是终止的必然产物（写文件可失败，升级 SIGKILL 可打断信号处理函数）；`classify` 在上述输入下确实返回 `Vanished`，原文「只在有退出标记时查询」使 `session.timeout` 事件无从被看到。矛盾属实，纯逻辑可证。
- 修改落点：设计文档 §3 第 1 步改为「`classify` 结果不是 `Running`（`Exited` 或 `Vanished`）时查一次 `session.timeout` 事件，有则先 `terminate_group` 再确认，确认后一律按 `TimedOut` 收割；确认不了保持 `Running`」；没有事件的 `Vanished` 仍是普通心跳兜底（N-3 不动）。M-02 ⑨ 改为一个用例两种输入（有 130 标记 / 无标记且日志拨到 11 分钟前，并断言同输入无事件时得 `Vanished`、`next_round` 不计该行、detail 含「超时」），负向对照补「只在有标记时查事件则 ⑨(b) 失败」；用例总数不变（15）。不新增里程碑。
- 复提计数：0。

### Backlog

- D2-B01（评审建议）：设计轮超时续做只有 H-01 兜底，且 H-01 只检查实现轮。裁决：纳入 Backlog（验证更完整，不属六类问题），不触发轮次。

## 定稿判断

**定稿。** D2-P01 的修订只是把第 1 步的触发条件从「有退出标记」放宽为「非 `Running`」，评审已给出最小修订要求，修订与之一致，不引入新的状态、变体或接口。逐类复核六类问题：关键事实错误（源码已核）、内部逻辑矛盾（§3 第 1 步与 §4/§6 现在一致）、违反项目规则（分层表未变）、违反任务目标（R-1～R-5、C-1～C-7 仍各有落点）、里程碑不可执行（M-01～M-04 均有验收命令、预期与负向对照，用例数与总数已核）、必须提前验证的重大风险（无）——均无剩余。`status` 由 `设计中` 改为 `实现中`，里程碑表完整（M-01～M-04，均 `开放`）。无争议项（所有复提计数为 0）。

## 提交

提交由本轮尝试；如被拒绝则由 Autome 兜底。
