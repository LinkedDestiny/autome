# M-01 · 实现轮 1/25

结论：M-01 验收通过，申请独立审计，状态为「待审」。本轮只推进 M-01。

## 范围与实现

- 起始 HEAD：`356db02`。开场工作区干净，没有上一轮审计文件，因此没有需逐字移植的审计回归检查。
- 已读取简报、任务、设计、会话协议与项目规则。核对本地 main：T-51 已合并（`bf399db`），未检出 T-53 合并记录；FIFO 继续以 `gate.queued` 的 seq 为准。没有 fetch、切分支、操作其它工作树或推送。
- 新增 domain `merge_queue`：有效授权、持锁/释放、持锁者优先与 FIFO 资格判断；新增服务层模块从账本恢复授权、锁和等待关系。
- 读取项目 `[merge] auto`，缺省 false，拒绝非布尔；M-02 的授权寿命、预先同意/撤销接口、警告、保存验收不在本轮推进。
- MergeGate 无作业时先取资格并拿锁，再沿用、准备、宿主准入和出队对齐；落后时 rebase，重算 HEAD/作业并重做准备检查。单次调用最多重入一次；若外部再次推进主线，保留锁并在下一 tick 重试。
- 落后判断复用现有 `git::is_ancestor(base, HEAD)`，等价于本轮所需的「base 独有提交数是否为零」；数值展示函数留在 M-04。
- 等锁任务保留 FIFO seq，但不进入宿主准入队列；Merging 同样等项目锁；advance 离开临界区时记解锁。事件覆盖 `merge.locked/unlocked/waiting/rebased/direct`。
- 保留 M-01 约定的旧授权清理，但只检查账本 active，不因项目 auto 写撤销事件；`merge.direct` 使用转换前的授权来源，避免旧清理逻辑丢掉 `behind` 来源。
- 出队冲突仍按本阶段的通用分支失败；回实现/修复轮与授权保留由 M-03/M-02 完成。

## 命令与结果

以下均在最终产品代码上执行，退出码 0：

| 命令 | 用例数 | 原始输出 |
|---|---:|---|
| `cargo test -p autome-domain merge_queue_` | 4 | `.autome/output/m01-final-domain.txt` |
| `cargo test -p automed --lib merge_queue_` | 11 | `.autome/output/m01-final-lib.txt` |
| `cargo test -p automed --test end_to_end merge_queue_` | 4 | `.autome/output/m01-final-e2e.txt` |
| `cargo test -p automed --lib gate_queue_` | 24 | `.autome/output/m01-final-existing-queue.txt` |
| `cargo test -p automed --lib requested_rebase` | 3 | `.autome/output/m01-final-existing-rebase.txt` |

里程碑验收命令按设计的 `&&` 顺序执行，19 个用例全部通过。另有 27 个直接相关既有用例通过；去重后的用例总数 **46，失败 0，忽略/跳过 0**。多次运行同一个测试与测试内部的矩阵分支不重复计数。domain 的另一测试目标没有此前缀，报告 0 个匹配用例，不计入 4。

**与上一轮基线之差**：本轮为首次实现，无上一轮执行基线。起始 HEAD 上 `git grep 'fn merge_queue_'` 命中 0；本轮同一选定范围的既有基线是 27 个，最终 46 个，增加 **19**（domain 4、lib 11、e2e 4），没有删除或改名既有用例。前缀基线输出：`.autome/output/m01-baseline-prefix.txt`。

已对改动的 Rust 文件执行 rustfmt 排版，`git diff --check` 通过。按本轮用户指令，**没有运行** `sh scripts/ci-local.sh` 或宿主快检命令（cargo fmt check、clippy、Web lint/typecheck）；这些检查仍由宿主执行，不以本轮子集通过替代。

## 标「待审」之前的自审清单

### 1. 情形表逐行

| 设计情形 | 检查与结果 |
|---|---|
| D1 有其它持锁者（已授权或未授权任务） | domain turn 的两种授权输入均等持锁者；lib second_merge_gate 实跑等锁，不准备/开门禁，等待事件与排队事件各一条；通过 |
| D1 已授权且前面有更早排队者 | reusable_gate 用创建顺序 C→B、入队顺序 B→C；A 失败后先访问 C，C 不拿锁、不沿用；B 先拿锁后 C 才沿用；通过 |
| D1 自己已持锁/没有前驱 | turn 自己不等自己；pending_setup 在多次调用间始终由 B 持锁；通过 |
| D1 沿用结果但分支落后/不落后 | reuse_requires 同一 HEAD 的通过记录正反对照：落后重跑，不落后沿用；通过 |
| D1 宿主名额不足 | dequeued_gate 用真实账本和阻塞作业，排队期间推进 main；排队时 HEAD 不动，释放名额后才 rebase；通过 |
| D1 对齐改变 setup watch | 两个 setup 用例：首次准备后主线新增 watch 文件，对齐后准备重跑；重跑异步期间持锁，C 不抢占；通过 |
| D1 主线未变化 | up_to_date 无 `merge.rebased`、门禁一次；通过 |
| D3/D4 门禁通过至合并完成的跨 tick 窗口 | 三任务 e2e：每次 gate 启动时记录 main/HEAD 并检查祖先，恰好三行 merge_gate；每任务 MergeGate started 一次、MainCheck reused 一次、MainCheck started 零次、merge.auto 零次、无落后回退；通过 |
| D4 临界区内用户按合并 | press_during e2e 通过真实 `task.merge` 按 B，A 门禁未结束时 B 保持 Merging 且 main 不动；A 失败解锁后 B 合并；通过 |
| D4 concurrency=2 | e2e 显式配置宿主/项目两个名额，同项目仍逐一合并；通过 |
| D3 重启 | e2e 重开磁盘 SQLite Store、新建 Ctx 并 recover，作业不重复启动，持锁者和 B→C 顺序保持；通过 |
| D2 项目开关/账本优先级 | domain authorization 覆盖 user、behind、旧无 by、撤销、旧 inactive、无记录，分别配 auto true/false；配置解析正反输入通过 |

### 2. 失败分支

- 门禁失败、准备失败、熔断：`every_exit` 分别用 GateDone Failed、SetupFailed、GateSameFailure 走真实 advance，均记解锁；press_during 用真实 shell 失败证实 B 继续。
- 超时：`every_exit` 收取 124 结果后解锁；既有 `gate_queue_paused_timeout_reclaims_and_preserves_124_for_resume`、`gate_queue_active_unreachable_jobs_timeout_without_business_poll` 实跑宿主回收，均通过。
- 出队冲突：本阶段仅检查 MergeGate 的 Conflict 进入通用失败分支并解锁；没有声称已验证 M-03 的真实冲突修复流程。
- 读盘/检出不可达：`reuse_requires` 内把 worktree 临时移走、把配置文件临时换成目录，两者均返回错误且 gate.started 为零；恢复后能启动门禁。
- 授权拒绝：本轮无新授权 RPC；domain 的用户撤销记录覆盖项目开关，并检查无授权时不会被 FIFO 前驱拦住。RPC 状态拒绝矩阵属于 M-02，不适用本轮。
- 外部服务不可达：不适用；本里程碑没有新增外部服务调用。本地 git/配置读取失败已检查。
- 空输入：空项目配置正常解析，auto 默认为 false；空字符串作为 auto 值被拒绝。

### 3. 全部状态迁移

`every_exit` 中分别执行下列转换，断言释放事件与推导持有者；domain holds/releases 另遍历全部 15 个需求节点和 7 个 Bug 节点。

| 事件/迁移 | 结果 |
|---|---|
| MergeGate → Implement（门禁失败） | 解锁，通过 |
| MergeGate → Failed（熔断、准备失败、本阶段出队 Conflict） | 解锁，通过 |
| 暂停、停止、取消 | 分别执行 Trigger，解锁，通过 |
| 门禁超时结果 124 | 离开 MergeGate 并解锁，通过 |
| Poll::Vanished | 清旧作业，保持 MergeGate 和原锁，通过 |
| HEAD 变化作废 | 按新 HEAD 重跑，原锁不释放，通过 |
| 门禁通过且授权有效 → Merging | 保留锁，最终合并后释放；project/behind 的 direct 来源均检查，通过 |
| 通过前账本授权撤销 → AwaitMerge | 通过时无有效授权，解锁；撤销 RPC 当场解锁留 M-02 |
| 配置移除 gate → Merging | 保留锁，通过 |
| Merging → Ok / Blocked / Behind / Failed | 四条结果各执行，全部解锁，通过；Behind 的 T-10 兜底保留 |
| 驱动重启 → 原状态继续 | 磁盘账本 e2e 通过 |

### 4. 输入域边界

- bool：缺省、true、false；非 bool 的整数 0/1、浮点、空串、字符串 true、数组、表均拒绝。
- 授权：无记录、旧记录无 by、active false 的用户覆盖、project 开关两值；均通过。
- 队列：空队列、单持锁者、自身持锁、自身队列项、无 seq、seq 相等、0、u64::MAX、前/后顺序与未授权任务；均通过。
- 状态：所有节点的 Active，加 Draft/Queued/Requeued/Paused/Stopped/Done/Cancelled；Failed 由服务层真实转换覆盖。
- 日期、闰日、前导零：不适用；本里程碑不解析日期或文本序号。

## 负向对照与修复记录

全部临时突变已还原，最终 19 个里程碑用例重新全绿。以下红色运行均退出 101，测试本身未削弱：

| 临时突变 | 检查 | 观测 | 输出 |
|---|---|---|---|
| 仅绕过入场时其它持锁者检查 | `cargo test -p automed --test end_to_end merge_queue_three_auto_merging` | `automatic merge fell behind: T-2` | `.autome/output/m01-negative-entry-final.txt` |
| 保留锁但跳过出队 rebase | 同上 | T-1 门禁中 `git merge-base --is-ancestor main HEAD` 返回 1，gate.result 为失败 | `.autome/output/m01-negative-align-final.txt` |
| Merging 绕过等锁 | `cargo test -p automed --test end_to_end merge_queue_a_press_during` | B 提前进入 main_check，原应仍在 merging | `.autome/output/m01-negative-merging.txt` |
| 拿锁推迟到 setup/对齐之后 | `cargo test -p automed --lib merge_queue_a_pending_setup` | 准备重跑期间 holder 为 None，原应为 B | `.autome/output/m01-negative-late-lock.txt` |
| 跳过已授权前驱 seq 判断 | `cargo test -p automed --lib merge_queue_a_reusable_gate` | C 提前写 merge.locked，原应为零 | `.autome/output/m01-negative-fifo.txt` |

开发时修正了测试夹具对尚不存在配置文件的读取，以及 GateDone 的字段构造。自审还发现 Requeued 的 JSON 实际为 `queued + resume: rebase`，已改成真实格式的断言：早期过宽突变依靠 90 秒超时失败，现在精确突变在落后回退出现时立即失败。旧输出留在 `.autome/output/m01-negative-no-lock.txt`、`m01-negative-no-holder-check.txt`、`m01-negative-entry-check.txt`，不把它们当成最终的精确判据。

## 交付边界

没有推进 M-02～M-05，没有更改协议、项目运行配置、全量门禁脚本或 UI。没有人工验收阻塞。宿主快检和独立审计仍待执行；本轮不把里程碑标为已完成。
