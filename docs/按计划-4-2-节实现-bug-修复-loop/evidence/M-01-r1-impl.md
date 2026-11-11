# M-01 · 实现 #1 · 领域层 Bug Loop

结论：M-01 验收通过，完成下列自审后报待审；不关闭里程碑。implementation-round 为 1/25。
本轮没有上一轮审计文件，也没有需逐字迁移、先红后绿的审计回归检查。

## 改动与范围

- 新增独立的 `bug_loop` 转换表、`BugProgress` 与 Bug 会话结果；需求入口仅增加设计 §1 指定位置的 Bug 节点防御，通用控制与出队仍走原表。
- `Node::ALL` 原来的 15 个节点及顺序不变；新增 `Node::BUG`、7 个节点、会话步骤、触发器、动作、注入与预算失败原因。
- 新增 `bug_doc::parse`：五个必填字段、可选 repos、两种冒号、围栏/正文排除、精确枚举与整数解析；字段错误带键名，重复字段同时带行号。
- 新增 `[bug] fix_rounds` 的默认值 3、项目覆盖、1–20 校验和保存/移除；无新依赖、无账本迁移。
- `TaskRecord::bug_doc()` 沿用 `doc_dir()`；新增枚举导致的穷尽 match 只补拒绝或透传。尚未接入 Bug prompt 和调度器，留给 M-02/M-03；没有启动任何新会话。
- 本轮未改协议、既有 prompt、CLI、apps、plugins、scripts 或 .github。`.autome/` 仅按本轮指令存放忽略的输出与临时验证文件，配置和规则未修改。

## 命令与验收结果

工作树起点 HEAD 为 `1bec0fb`，源代码基线与任务的 `81a4394` 一致。

1. 原始源码 `cargo test --workspace`：退出 0，**990 个用例**，990 通过、0 失败、0 忽略。日志 `.autome/output/m01-baseline-clean.txt`。
   首次尝试在 rustdoc 阶段被本轮并行源码编辑干扰，不能作为完整基线；随后将已编辑的跟踪源码临时备份并恢复 HEAD 原文，完整重跑成功，再恢复改动。失败尝试保留于 `m01-baseline.txt`，未隐去。
2. M-01 原样验收命令：

   ```sh
   cargo test -p autome-domain bug_loop && cargo test -p autome-domain bug_doc && cargo test -p autome-domain bug_fix_rounds && cargo test -p automed --lib bug_fix_rounds && cargo test -p autome-domain && cargo build --workspace --all-targets
   ```

   最终退出 0；前四条分别 **29、8、2、3** 个用例，均通过；领域层全量 **345** 个，全部通过；工作区所有目标编译通过。最终源码日志 `.autome/output/m01-acceptance-delivery.txt`。
3. `cargo test --workspace`：**1032 个用例**，1032 通过、0 失败、0 忽略；相对基线 **+42**，来自转换表 29、解析器 8、领域配置 2、配置 IO 3。领域层 306 → 345（+39），automed 单元 582 → 585（+3）；CLI 单元 17、CLI 集成 8、e2e 59、prompt 11、real_cli 7 均不变，doc-test 0。日志 `.autome/output/m01-final-checks.txt`。
4. 全量测试后的 Clippy 首次指出解析器 `collapsible_if`；仅合并条件表达式，随后 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test -p autome-domain bug_doc` 退出 0。日志 `.autome/output/m01-lint-final.txt`。此格式修正后又完整执行了第 2 项；不将此前那条含 Clippy 失败的组合命令说成整体通过。
5. `git diff --check` 通过。既有 `task.rs` 整个测试模块与 HEAD 逐字节相同（自审发现批量替换曾误碰一条既有断言，已原样恢复并以恢复后的版本跑全量测试）；其他既有测试没有改动。

用例总数按测试框架的 passed + failed + ignored 计，重复执行的专项测试不累计到总数。`real_cli` 的既有环境条件未改，不把该套件报告等同于本轮真实模型验收。

## 标待审之前的自审清单

### 1. 情形表逐行

下表编号指设计 M-01 的 29 个 `bug_loop::tests` 用例；均实际执行并通过。

| 设计 §1 情形 | 检查与结果 |
|---|---|
| Queued + SlotAvailable | #6 到 Locate 且 StartBugSession；#7 实现角色关闭则 Config 失败 |
| Locate + 已定位 | #8 到 ReproCheck 并 RunCoreStep |
| Locate + 无法复现 | #9 到 AwaitInfo，无槽、等待用户 |
| Locate + 不是 Bug | #9 到 NotABug，无槽、等待用户 |
| ReproCheck + 已复现 | #13 到 Fix，预算取配置；#22 耗尽则 FixBudget |
| ReproCheck + 首次未复现 | #14/15 回 Locate，携带命令、sha、退出码、原因、尾部 |
| ReproCheck + 第二次未复现 | #16 到 AwaitInfo；超时、126、127 同样计为未复现 |
| Fix + 会话正常结束 | #17 无论有无项目 gate 都到 FixGate |
| FixGate + 通过 | #19 审计开启到 FixReview；关闭到 Rebase |
| FixGate + 失败 | #18 回 Fix 并带原门禁尾部；#22 预算耗尽停下 |
| FixReview + 通过 | #20 到 Rebase，不经过 Retro |
| FixReview + 打回 | #21 回 Fix，注入 FixRejected；#22 查预算 |
| Rebase + 成功 | #24 必到 MergeGate，保留合法 merge_requested |
| Rebase + 冲突 | #25 回 Fix，保留冲突文件；#22 查预算 |
| MergeGate + 失败 | #25 回 Fix；#22 查预算 |
| MergeGate 通过及合并尾段 | #26 与需求表在全部触发器、两种 merge_requested 下逐项相同 |
| AwaitInfo / NotABug + Reply | #10 非空回复回 Locate；#11 空白或非等待状态拒绝 |
| Failed FixBudget + ExtendBudget | #23 追加至新 limit 后到 Fix；0 和溢出拒绝 |
| Failed / Stopped + RerunFrom | #28 只接受 Locate/Fix/FixReview/Rebase，显式重跑 Fix 不查已用预算 |
| Stopped Bug gate + Resume | #29 到 Requeued 并 DiscardGateRuns，两种 gate 均验证 |
| 通用控制与重新出队 | #4/#12/#29 覆盖 Cancel/Pause/Stop/Resume、等待节点拒绝暂停/停止/追加、Bug 节点出队；会话 Reenter、内核步骤 RunCoreStep |

设计 §2 字段落法表也逐行核对：

- status：解析器三值测试；#27 验证 Fix/Review 必须已定位。
- fix-round：格式与数值边界测试；领域轮次只读 BugProgress/Context（#13/#22/#23），不以文档里的 0/3 限制实际轮次。
- 复现命令：已定位时空值/无被拒，单行取值；运行时固定使用复现事件命令属 M-03，本轮不声称执行取证。
- 根因：已定位时空值/无被拒；其余状态允许。
- 复核：三值解析；#27 对三种状态 × 三种复核值 × 三个会话节点检查，定位/修复必须 Pending，复核必须 Passed/Rejected。
- repos：缺省空、两种逗号、去空白/空项、单元素、重复键拒绝均验证。工作区成员资格守卫属 M-04，不在纯解析层添加外部依赖。

### 2. 设计点名的失败分支

- 文档缺键、重复键、非法枚举、非法轮次、已定位缺命令/根因：8 个解析器测试覆盖，错误可定位字段。
- 会话 Crashed / Unparseable / MissingArtifact / GuardFailed：#27 在三个会话节点逐一验证；崩溃映射 SessionCrashed，其他映射 Protocol。真实读盘失败和自动重试由 M-03 接入，这里只验证领域输入结果。
- 超时、退出 0、不可执行 126/不存在 127：#14–16 验证不算复现；已执行超时负向对照。
- 门禁失败、复核打回、rebase 冲突、预算边界、角色关闭、空回复、非法重跑：#7/#11/#18/#21–23/#25/#28 验证。
- 拒绝授权、外部服务不可达：不适用，本里程碑为无 IO 的领域转换与本地配置 IO，不新增授权或外部服务调用。用户操作的状态拒绝已经由 #11/#12/#28 验证。

### 3. 全部状态迁移

- 上述 §1 全部 21 行都有对应运行检查，不只主路径。
- #3 遍历 Node::ALL × Active/Paused/Stopped/Requeued/全部 FailureReason，另加 Draft/Queued/Done/Cancelled，覆盖所有 Trigger 变体、SessionEnded 的正常文档与四类失败、全部节点的 core/gate/rerun 输入及两种 merge_requested；Feature 入口与原表逐项相等。
- #4 确认 Bug 专属事件不能进入需求节点表；控制触发器仍有效，Bug 会话状态不会落到需求表的 unreachable 分支。
- #5 检查新 Bug 的 Draft/Queued 选表、旧 Bug 的所有需求节点不自动换表、started 事件标记生效、Feature 永不选 Bug 表。
- #1/#2 验证 7 个新节点的线上名称、角色、占槽/等待/内核/gate 标志与 SessionKind 映射，原 Node::ALL 的 15 个名称和顺序不变。

### 4. 输入域边界

- fix-round：0/0、前导零 01/03、u32 最大值、k>N 接受；空串、缺斜杠、负号、加号、内部空格、全角数字、重复斜杠与 u32 溢出拒绝。文档轮次只校验格式，符合设计。
- fix_rounds：缺省/空表为 3，1 与 20 接受；0、-1、21、u32 边界及以上、浮点/布尔/字符串/数组拒绝；保存后重读、移除配置及非法保存不破坏原配置均验证。
- repro_misses：0、1、u32::MAX；退出码 0、1、125、126、127、128、255、-1；预算 limit-1/limit/limit+1；追加 0 与溢出，均有断言。
- 空文档、五个必填键缺失、缩进键、围栏内伪字段、首个正文标题后的伪字段、空白回复、repos 单元素及空项，均覆盖。
- 闰日、跨年：不适用，本里程碑不解析日期。

## 负向对照（故障注入后全部还原）

运行器 `.autome/output/m01-mutations.rb` 每次仅改一处源码，运行指定项目测试，finally 恢复原文；要求是真正断言失败，而非编译错误。

| 注入错误 | 判别检查 | 红结果 |
|---|---|---|
| Feature 入口错误地启动 Locate | #3 | 1 失败，退出 101 |
| 超时也算复现 | #15 | 1 失败，退出 101 |
| 预算判断改为 > | #22 | 1 失败，退出 101 |
| 只认半角冒号 | reads_all_five_fields_with_either_colon | 1 失败，退出 101 |
| 把 Locate 加入 Node::ALL | #2 | 1 失败，退出 101 |
| 把 Bug 防御放在通用控制之前 | 全部 bug_loop 测试 | #4/#12/#29 共 3 失败，退出 101 |

汇总 `.autome/output/m01-mutations-summary.txt`，原始输出在 `m01-negative-*.txt`。还原后的 29/8/2/3 专项及领域全量均绿（最终验收日志）；没有遗留注入。

## 交接

只报 M-01 待审；M-02–M-05 保持开放。后续独立审计复验本里程碑，审计通过后才能关闭。本轮不启动下一会话、不推送远端。人工真实模型 Bug Loop 与调度器 e2e 尚不在本里程碑范围。

交付状态格式检查：以本轮构建的 `autome_domain::status_block::parse` 读取设计文档，断言状态为实现中、轮次 1/25、五个里程碑中只有 M-01 为 Pending，其余为 Open，退出 0。检查源码与输出：`.autome/output/m01-validate.rs`、`.autome/output/m01-status-validation.txt`。未修改冻结协议副本。
