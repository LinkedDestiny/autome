# 宿主门禁按机器排队：裁决记录

## 第 1 轮裁决（design-round 0 → 1）

- **评审结论**：第 1 轮独立评审提出 5 项阻塞问题（P01–P05）和 3 项非阻塞建议（N01–N03），基线 `599873c`。
- **裁决结论**：5 项阻塞问题**全部采纳**——每项我都对照当前源码复核了评审引用的调用顺序，反例成立。设计文档已按下表修改，
  `design-round` 0 → 1。因为新增了清扫、空闲任务超时回收、`gate.queue_dropped`、`bug.fix_round_requested` 与接续资格这几个
  新机制，它们还没有被独立评审过，所以 `status` **保持 `设计中`**，请下一轮评审重点核对这些机制；六个里程碑的表与契约已同步
  更新数字（M-02 +5、M-04 +2）。
- 没有任何主张达到复提计数 2，本轮无争议项。
- 提交：本轮改动在当前分支提交；若提交被拒绝则由 Autome 兜底。

### 逐条裁决

| ID | 主张摘要 | 设计位置 | 裁决 | 复提计数 |
|---|---|---|---|---|
| D1-P01 | head 在两次 tick 之间变化，旧 head 的已退出作业仍满足 (b) 且无人 poll/clear，永久占位 | §2 | 采纳 | 0 |
| D1-P02 | 暂停期间释放名额的准备作业，恢复后仍被 §5 当作「接续」免准入，突破 N | §2、§5 | 采纳 | 0 |
| D1-P03 | `perform` 在 `start_session` 之前就写 `bug.fix_round_started`，准备排队时已花掉修复轮次 | §5、§6 | 采纳 | 0 |
| D1-P04 | R1（不可推进的队首）与 R3（暂停的挂死作业）让有限等待不成立，与约束 7 冲突 | §2、§3、§4、R1、R3 | 采纳 | 0 |
| D1-P05 | ReproCheck 内核超时后的 `Inject::ReproNotFailing` 与 `bug.repro_missed` 仍不写「门禁超时」 | §9、M-04 | 采纳 | 0 |
| D1-N01 | T3 的负向对照应针对 B 的入队时间 | T3 | 部分采纳（进 Backlog） | 0 |
| D1-N02 | 项目值 1 不是持续独占，括号解释不准确 | §7 | 采纳（文字修正） | 0 |
| D1-N03 | §9 保留的熔断摘要对非内核 124 的说法是兼容例外，应明示 | §9 | 采纳（文字修正） | 0 |

### D1-P01 head 变化使旧作业永久占位

- **证据**：`crates/automed/src/scheduler.rs` 的 `run_gate` 每次按当前 `gate_head` 构造 `Job::at(&repo, &task.id, node, key)`，
  只 poll 这一个；`job.clear()` 只在「同一调用内 poll 到 Done 后发现 head 已变」时执行。head 在 tick 之间变化时，旧 key 的
  作业没有任何调用方再碰，反例成立：旧作业退出后若仍是任务在该节点最后一条 `gate.started`，§2 (b) 会让它永久占位。
  （`job.clear()` 也不杀进程，所以旧作业活着时它还继续跑。）
- **修改**：§2 新增「旧 head 的作业必须被收束」：`run_gate` 对 Gate / MergeGate / ReproCheck / FixGate 在一切准入和
  `job.exists()` 之前，先调新增的 `gate::discard_stale(repo, task, node, keep_key)`，对 key 不等于当前 head 的作业：活的杀
  进程组，随后一律连文件删除（已退出未收、`Vanished` 同样）。MainCheck 的 head 是固定合并提交，不需要。
- **落点**：设计 §2；M-02 新增 1 个用例（旧 head 作业不占名额、新 head 照常准入）。

### D1-P02 暂停期间释放的准备作业恢复后绕过准入

- **证据**：`task.rs` 暂停不杀作业、恢复 Paused 不清作业；`setup.rs` 的 `ensure` 可收遗留的 `Done` 作业，其后 `run_gate`
  同一调用里 `gate::start`。§2 说非 `Active` 的已退出作业不占名额，§5 又以「准备作业存在且未 `Vanished`」免准入，两处合起来
  给出评审的 N=1 反例（A、B 并行）。
- **修改**：§5 把准入点从「某个函数入口」改成「每一个要起进程的点」（`ensure` 起准备进程前、`run_gate` 的 `gate::start` 前），
  并把免准入收窄为**接续资格**：本次调用刚收完的同任务同检出准备作业，且任务是 `Active`、最后一条 `setup.started` 之后
  没有 `task.updated`。暂停 / 恢复必然写 `task.updated`，所以越过暂停的作业不再免准入，恢复的任务重新排队尾；暂停期间作业
  仍存活的情形没有问题（它按 (a) 一直占名额，恢复后 `ensure` 只返回 `Pending`）。已释放名额的作业结果仍可收，只是其后
  的新进程要准入。判据保守（多等一次队，不超额），覆盖三处：门禁前准备→门禁、会话前准备指纹又变重跑、`setup-main`。
- **落点**：设计 §5；M-02 新增 1 个用例（暂停恢复后重新排队尾、不与已占满名额的任务并行）。

### D1-P03 Fix 会话前准备排队提前消耗修复轮次

- **证据**：`perform(StartBugSession { step: Fix })` 在 `start_session` 之前写 `bug.fix_round_started`（`scheduler.rs`
  约 2026–2035 行），会话前准备的准入在 `start_session` 内部；`redispatch_with`（重试到期）直接调 `start_session`，不再经过
  `perform`。反例成立：FixReview 打回后若准备被名额挡住，轮次已被花掉而没有 Fix 会话。
- **修改**：§6 把轮次的记入点后移到真正起会话：`perform` 改写标记事件 `bug.fix_round_requested`（纯增量，不进
  `bug_progress`），`start_session` 对 Fix 步骤在准备 `Ready` 之后、起会话之前，仅当最后一条属于
  {`bug.fix_round_requested`, `bug.fix_round_started`} 的事件是前者时才写 `bug.fix_round_started`。这样排队、冷却推迟都不动
  轮次，等待后真正启动只记一次，崩溃重试 / 暂停恢复不重复。不能用「`task.updated` 之后」作判据——暂停恢复会误加一轮，故用独立标记。
  旧用例（`bug_loop_*` e2e）都在会话启动后才计数，预计不受影响；若实现期发现需要改旧用例，按任务文件 §3 记理由。
- **落点**：设计 §5、§6；M-02 新增 1 个用例（Fix 准备被挡住期间 `bug.fix_round_started` 数不变，启动后 +1，崩溃重试不再加）。

### D1-P04 两种无限阻塞与约束 7 冲突

- **证据**：`tick` 对内核步骤错误只 `report.errors.push` 后继续（`scheduler.rs` 约 196–210 行）；`poll_as` 只在 `run_gate` /
  `ensure` 里对 `Active` 任务调用，超时终止发生在那里。设计 R1、R3 自认两个前提可以永久不成立，仅靠人工解除——这与约束 7
  「任何排队任务在有限个作业之后一定轮到」直接冲突，写进风险不能使约束成立。
- **修改**：
  - R1：tick 在 `run_core_step` / 会话启动路径出错的同一处，若该任务持有有效排队项，追加 `gate.queue_dropped {node, error}`
    （加入有效排队项的失效列表）；错误照旧上报，不吞；有限等待的保证范围限定为「能走到准入判定的任务」。
  - R3：§2 新增**空闲任务超时回收**——每个 tick 在内核步骤之前，对非 `Active` 的未结束任务的存活作业按其超时调
    `gate::poll_as`（它的既有行为：到点杀进程组、追加终止行、写 124），不收结果、不删文件、不写事件；名额因此释放，
    恢复后收到同一个 124。这与「恢复时超时立即生效」的结果一致，只是提前到超时那一刻，所以不改任何既有用例的语义。
  - §4 不饿死论证改写为两个由设计保证的前提（作业都会结束：`Active` 靠 poll、非 `Active` 靠回收、旧 head 靠清扫、升级遗留同理；
    队首能推进：靠 `gate.queue_dropped`）；R1、R3 改为「已解决 + 残余」；B-02 因已采纳而移出 Backlog。
- **落点**：设计 §2、§3、§4、R1、R3、B-02；M-02 新增 2 个用例（队首出错退队、暂停挂死作业被回收）。

### D1-P05 ReproCheck 超时的两条可见文案

- **证据**：`Trigger::ReproDone{timed_out:true}` 经 `bug_loop.rs` 构造 `Inject::ReproNotFailing{why:"timed_out"}`，`launcher.rs` 仍写
  「复现检查没有失败……原因：timed_out……请重新定位并提交稳定的回归测试」；`scheduler.rs` 写 `bug.repro_missed`，`render.rs`
  仍显示「未复现……timed_out」。设计 §9 原先只列了 7 处，并把 ReproCheck 后续文案标为「不动」，那只是原因码，不是满足约束 9 的说明。
- **修改**：§9 新增第 8、9 条（共九处）：`why == "timed_out"` 且 `kernel_timeout(tail)` 为 `Some(N)` 时，注入改为「复现检查门禁超时……
  没有得出结论，不能据此断言未复现」，事件行改为「门禁超时：……本次不能据此断言未复现」；其余 `why`（含非内核 124）保持原文。
  不改任何 serde 形状、不改 `scheduler.rs` 的写入点。判据复用设计已选的 `tail`。
- **落点**：设计 §9、M-04 说明与契约（automed +1、autome-cli +1）。

### 非阻塞建议

- **D1-N01**：T3 的鉴别力改进，按协议不单独触发复审；有跨轮价值，作为实现注意事项记入 Backlog **B-07**，不改 T3 正文。
- **D1-N02**：采纳，§7 去掉「要独占机器」的说法，改为准确描述（项目值是该项目作业的启动时机约束，不是持续独占）。
- **D1-N03**：采纳，§9 明示「非内核 124 在熔断摘要里仍说被内核终止」是保留的兼容例外，并与 B-03 互指。

### 对里程碑的影响

- 里程碑表不变（6 个，均 `开放`，reopen 0），`current-milestone` 仍 M-01，`next-action` 不变（M-01 的内容未受影响）。
- 契约数字：M-02 新增 12 → 17（automed --lib 667 → 672）；M-03 670 → 675；M-04 automed 4 → 5（674 → 680）、autome-cli 3 → 4（45 → 46）；
  M-06 合计 1251 → 1258（新增 50 → 57：domain 12、automed lib 32、autome-cli 7、end_to_end 6）。M-01 的数字不变，只在有效排队项的
  失效种类里补 `gate.queue_dropped`。

### 设计是否定稿

否。五项阻塞问题已全部处理，但本轮引入的几个新机制（旧 head 清扫、空闲任务超时回收、`gate.queue_dropped`、
`bug.fix_round_requested`、接续资格）会改动被评审过的生命周期与预算路径，值得下一轮独立评审核对；`status` 保持 `设计中`。
