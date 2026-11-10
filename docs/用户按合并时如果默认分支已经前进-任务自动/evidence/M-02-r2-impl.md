# M-02 · 实现 #2（2/30）

## 范围与实现

- 仅推进最小开放里程碑 M-02。上一轮 M-01 审计通过，无退回缺陷，因此没有需要逐字迁入的审计缺陷检查；上轮临时变异检查没有重跑。
- `scheduler::advance` 每次从最后一条 `merge.auto.active` 读取授权，使用 `apply_with_merge_request`；清除在状态写入之前、置真在之后，不改 schema。无事件按 false 处理。
- `merge_all` 先检查全部未并入仓：纯 Behind 直接交还槽位并排队；混合拒绝沿用部分合并循环。实际合并用 `try_merge` 当场的分类汇总，任何非 Behind 拒绝都返回 Blocked，不重新查询原因。
- 成功的已授权 rebase 在转换前写 `merge.rebased`，记录每仓 repo、onto、实际 head。无授权不写，门禁运行/沿用逻辑未改。
- 新增 S1/S2/S3/S4/S5/S7 六个调度器测试与 E1/E2 两个 e2e；既有测试未修改。
- B-01、B-02 用户决定忽略，不实现；B-03 纳入新增 M-06，前置 M-04，独立验收实际幂等变异。一轮只推进一个里程碑，因此本轮未执行 M-06，不把它冒报为已实现。

## 验收命令与结果

原始输出均在 `.autome/output/`，本文仅保留判据。

- `cargo test -p automed --lib > .autome/output/m02-lib.txt 2>&1`：554/554 通过，失败 0，忽略 0。上一轮 548，增量 +6，全部来自 S1/S2/S3/S4/S5/S7；S1 内还检查空任务引用、缺失仓库两个错误输入，不另计测试数。
- `cargo test -p automed --test end_to_end > .autome/output/m02-e2e.txt 2>&1`：52/52 通过，失败 0，忽略 0。上一轮 50，增量 +2，来自 E1/E2。两个验收目标合计 606（上一轮 598，+8），没有减少或修改旧用例。
- `cargo test --workspace > .autome/output/m02-all-final.txt 2>&1`：退出码 0，全工作区 938 例通过，上一轮 930，+8。分二进制为 autome 12、cli 4、domain 299、automed lib 554、end_to_end 52、prompt_rendering 10、real_cli 7，doc-tests 0；失败/忽略均 0。
- `cargo clippy --all-targets -- -D warnings > .autome/output/m02-clippy.txt 2>&1`：退出码 0。初次发现新增嵌套 if 可折叠，按建议修复，未关闭 lint。
- `cargo fmt --all -- --check > .autome/output/m02-fmt-check.txt 2>&1`：退出码 0。

### 首轮失败与修正

1. E2 初次失败：RPC `task.merge` 返回时已到 cleanup，不是设计假定的 merging。源码 `dispatch::task_trigger` 先 apply_trigger 再立即 tick。本轮就地修订 E2 契约，使用同一个生产 `scheduler::apply_trigger(Merge)`，在它与 tick 之间提交 main，再 settle；其余 intake/实现/审计/包装脚本/rebase/merge 仍是真实链路。修正后单例 1/1 通过（`m02-e2-rerun.txt`）。E1 仍用真实 RPC，完整证明用户只按一次。
2. 补充错误输入自审时，首次全工作区运行在 S1 失败（lib 553/554）：空 base 被既有 NotOnDefault 前置条件挡回，并不是读盘失败。用空 task branch 与缺失仓库来触发 Git 读取错误；最终 lib 554/554 通过。未改变产品错误分类来迁就测试。

### 四项负向对照

临时变异均运行对应现有项目测试，每次退出码 101、0 通过/1 失败，之后还原。没有以编译错误代替失败断言。

| 变异 | 命令（输出文件前缀均为 `.autome/output/`） | 判据 |
|---|---|---|
| 阶段一纯 Behind 改回 Blocked | `cargo test -p automed --lib a_merge_behind_main_is_requeued` → `m02-negative-behind-blocked.txt` | S1 得到 AwaitMerge 而非 Requeued，红 |
| 脏文件分类改成 Behind | `cargo test -p automed --lib dirt_and_a_stale_branch_together` → `m02-negative-dirty-is-behind.txt` | S4 得到 Requeued 而非 AwaitMerge，红 |
| 阶段二只要有 Behind 就忽略其它拒绝 | `cargo test -p automed --lib a_dirty_repo_and_a_behind_repo` → `m02-negative-any-behind.txt` | S7 的 Blocked 断言红 |
| Behind 直接进 Active Rebase | `cargo test -p automed --lib with_every_slot_taken_the_requested_rebase` → `m02-negative-bypass-queue.txt` | S5 槽位变为 2，预期 1，红 |

对应变异脚本仅留 `.autome/output/m02-mutations.rb`，第四项命令为替换 domain 的 Behind 分支 Requeued→Active 后跑 S5；全部最终还原。`git diff --quiet -- crates/autome-domain/src/task.rs crates/automed/src/git.rs` 为 0，调度器四个判别用例在最终 lib 全量中全部绿。

## 标待审之前的自审清单

### 1. 情形表逐行

| M-02 情形 | 实际检查与结果 |
|---|---|
| 只有默认分支落后 | S1：Requeued(Rebase)、active=true、main/worktree head 均未动、merge.repo 为 0 |
| 自动 rebase、无门禁 | S2：事件 head 等于真实 worktree head，转 Merging 时 active=false，再一步 Cleanup，改动进入 main |
| 正常复盘 rebase，无用户授权 | S3：停 AwaitMerge；merge.auto 与 merge.rebased 均为 0 |
| 同一仓又脏又落后 | S4：停 AwaitMerge，无授权事件；main、任务 head、用户文件内容不变 |
| 并行上限 1 已占满 | S5：核心步骤后与 fill_slots 后均只占 1；没有会话、没有 rebase；释放后自动入 Rebase，授权保留 |
| 一个仓脏、另一个仓落后 | S7：真实双仓 TaskLayout，返回 Blocked，两个 worktree head 和用户文件均未动 |
| 两任务同时等合并、配置门禁 | E1：A/B 都完成；B 的 merge.rebased 之后存在 merge_gate 成功事件且 sha 相同；A 合并提交为 B 祖先；B 按下后无 AwaitMerge 事件 |
| 按下到真正执行间 main 前进 | E2：生产 Merge 触发后 main 提交，最终 done、恰好一次 merge.rebased、main 新提交仍在历史中 |

### 2. 设计点名的失败分支

- 拒绝授权：S3 无事件的旧任务不自动合并；domain D9/D11 检查过期授权与错误节点/触发组合拒绝；本轮无新增 RPC 或权限输入。
- 读盘失败：S1 中空任务引用、缺失仓库都返回 Failed，main 不动，不产生额外授权。已有 G1 对不存在与空分支的检查仍通过。
- DirtyConflict：S4/S7；NotOnDefault/MergeFailed：现有 G3 的真实 Git 四类拒绝检查随 lib 全量通过，未将其改为 Behind。
- Rebase 冲突、门禁失败：domain D6/D7 在全工作区运行通过；调度器/包装脚本层专项是下一里程碑 M-03，本轮没有宣称其验收完成。
- 超时：本轮没有新增等待/重试逻辑；原有 gate timeout 用例保留在 lib，全量通过。E1/E2 的 settle 使用现有 90 秒有条件截止循环。
- 空输入：S1 的空任务引用返回 Failed；单仓 repo 名为空字符串是合法值，S1/S2 的事件断言保留它。
- 外部服务不可达：不适用；本路径只调用本地 Git/SQLite/宿主门禁，没有新增外部服务。

### 3. 全部状态迁移

- AwaitMerge + Merge → Merging：S1/S2/S4/S5，E1 真实 RPC；新纳入 Backlog 回实现仍由原有调度器测试覆盖。
- Merging + Behind → Requeued(Rebase)、授权 true：S1/S5；Merging + Blocked → AwaitMerge：S4/S7；Merging + Ok → Cleanup：S2，配置门禁则 MainCheck→Cleanup→Done：E1。
- Requeued + 无槽位保持原状态；+ SlotAvailable → Active Rebase：S5。
- Rebase + Ok + 授权 + 无门禁 → Merging：S2；有门禁 → MergeGate → Passed → Merging：E1；无授权 → AwaitMerge：S3。
- Rebase/MergeGate 的排队、Pause→Resume→SlotAvailable 保留授权；Stop/Cancel 清除、重跑/复盘不带入旧授权：现有 domain D8/D9 随本轮 domain 全量执行。D6/D7 覆盖冲突/门禁失败回实现及 Rebase Failed 清除授权；D10 覆盖自动路径中新纳入 Backlog 回实现；D11 遍历 Node::ALL 拒绝非 Merging 的 Behind。
- 事件落盘顺序逐行检查：active=false 在 set_task_state 之前；active=true 在之后；merge.rebased 在 rebase 成功后的 advance 之前。没有修改任务 schema 或状态 JSON。

### 4. 输入域边界

- 单仓/两个仓、空合法 repo 名：S1/S2/S7；零个落后仓 S3/现有直合并测试，一个落后仓 S1，多类拒绝 S7。
- 并行上限最小有效值 1、满槽、释放：S5。无门禁 S2、有门禁 E1。
- 无 merge.auto 历史：S3；true 延续及退出变 false：S2/S5。
- 空 task ref、缺失仓库：S1。非空 Git SHA 由真实 Git 产生，E1 验证门禁 sha 与事件 head 完全相同。
- 日期、前导零、闰日、跨年、数值最大值：不适用，本轮没有解析这类输入，也没有引入计数上限（B-02 已忽略）。

## 交付

四项自审执行完成后，M-02 标为待审，implementation-round 为 2/30；M-01 保留已完成，M-03～M-06 保持开放。设计只保留本轮最新证据指针，retro 追加一行并记录 B-01/B-02 忽略、B-03 纳入 M-06。

`git diff --check`、格式检查通过；现有 e2e 删除行数为 0。修改范围为两个 crates 文件及本任务文档；没有改 apps、协议源、其它 worktree，没有推送或启动下一会话。按用户要求在当前分支执行 `git add` 与 `git commit`。宿主 `sh scripts/ci-local.sh` 留给 Autome 执行；本轮不冒报桌面人工验收或 M-03/M-04/M-06 已通过。
