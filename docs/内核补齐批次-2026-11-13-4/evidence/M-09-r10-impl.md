# M-09 · 实现 #10

结论：待审。实现 D-R2–D-R4、D-R6、D-R8；保持 M-08 的输入与可见性契约。上一轮审计 M-08 通过，没有退回缺陷或必须逐字搬入的判别检查。本轮没有运行全量门禁和宿主快检。

## 实现与修正

- `handoff::pending` 汇总未送达备注，排除合并开始后的 late 备注。启动固定 `session.started.notes`，需求实现与 Bug 定位/修复收件，原文含换行保留。只有成功且守卫通过才记 `task.note_delivered`；运行期间新增备注不混入当前收尾。
- 复用交接补轮；合并前 Rebase（无门禁）/MergeGate 通过时先检查交接，再决定等待或自动合并。`ReopenForNote` 只接受 AwaitMerge，Bug 耗尽预算拒绝且提示 extend；预授权保留。CLI 的预算失败提示补充未送达备注。
- v9 审计标记控制强制核对，旧模板仍正常送达。审计启动固定核对编号，只有关闭里程碑或集成复验通过才确认；退回后的实现与审计继续列出未确认的备注，不重复记录送达。Bug 复核沿用相同窗口。交接产生产品提交时 integration trigger 为 `operator_note`。
- CHANGELOG 只追加既有 v9 内的 D-1，增加行为评测夹具；README 命令与控制说明、技术设计更新。数据库不迁移。
- 自审修正：Bug 原收尾先于回归保护守卫，备注标记改到守卫通过之后，并加入改测试被拒的反例；正常定位到修复不重复呈现，只有已进入审计而未获确认的备注才重呈现。
- 金样更新器原来直接保存未裁剪的 integration-audit 模板块，导致刷新后普通审计金样失败；按实际渲染裁剪该块，保留原断言。检查最终 diff，既有四个审计金样只新增 operator-notes 三行。

## 命令与结果

| 命令 | 最终结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed -- operator_note_` | 19：库 5、end_to_end 11、prompt_rendering 3，全通过 | `.autome/output/m09-final.txt` |
| `cargo run -p automed -- protocol eval protocol/` | 8 项通过、0 失败；1 项既有警告：41 行旧条款缺 CHANGELOG 来源。23 个评测定义可读且有阳性对照 | `.autome/output/m09-protocol-final.txt` |
| `grep -q 'autome note' README.md && grep -q '操作者备注' docs/development/technical-design.md` | 退出 0 | 主验收链 `acceptance_exit=0` |
| `cargo test -p autome-domain --lib operator_note_` | 2 个通过；覆盖无门禁 Rebase、MergeGate、有无备注与预授权组合、reopen 节点限制、Bug 预算 0/1 边界 | `.autome/output/m09-domain-final.txt` |
| `cargo test -p autome-domain --lib scope_patch` | 13 个通过，共用交接转换及既有补丁行为不退化 | `.autome/output/m09-scope-domain.txt` |
| `cargo test -p automed --lib handoff` | 5 个通过，含超时重试和结果文件不可读不送达 | `.autome/output/m09-handoff-final.txt` |
| `cargo test -p automed --test prompt_rendering` | 43 个通过，包含主验收的 3 个金样 | `.autome/output/m09-prompts-final.txt` |
| `cargo test -p autome-cli operator_note_` | 单元 3 + 真二进制集成 1，全通过，含预算提示 | `.autome/output/m09-cli.txt` |
| `git diff --check` | 退出 0 | 无输出 |

主验收仍为设计预期 **19 个**。同口径上一轮 `automed operator_note_` 为 5，本轮 **+14**（11 个端到端 + 3 个金样）。最终验证集合 **83 个不同用例**，上述测试命令共执行 86 次用例（3 个金样重复），全部通过，0 失败、0 跳过。上一轮实现/审计基线实际为 **60**（并非契约预期 59，另有 CLI 集成 1），本轮 **+23**：保留内核 5、CLI 4；本轮不跑网页 51，增加端到端 11、金样 43、域 15、交接库 5，共新增本轮验证范围 74。不是全库净增数字。过程中重复执行与红色开发尝试不累加。

开发阶段的失败及纠正：`m09-golden-before.txt` 记录新增金样缺失及模板变化；`m09-golden-update.txt` 的更新器裁剪问题已修正，最终 43 个金样通过。`m09-e2e.txt` 曾因零退避重试不可观察、从无复现结果的定位状态直接 rerun fix、测试自己提前触发合并而失败；夹具分别改为读取退出标记、从已成功复现的 Bug 重跑、仅保留授权事件。没有弱化产品断言。最终主验收覆盖这些修正；产品提交触发标签的补强亦以同名端到端用例验证（`m09-integration.txt`，1 个通过，不另计总数）。

## 标待审之前的自审清单

### 1. 情形表逐行

- 需求实现中收备注：当前简报没有，下一实现轮有且仅送达一次（`note_during_impl_reaches_the_next_impl_once`）。
- 最后审计中收备注：实现补轮→独立集成复验→合并，产品提交 trigger 为 operator_note（`note_during_last_audit_adds_an_impl_round_then_merges`）。
- AwaitMerge：无 reopen 拒绝且不记备注；有 reopen 实现→审计→合并，已有授权保留（`at_await_merge_needs_reopen_and_keeps_preauthorization`）。
- Bug 停止后 rerun fix/locate：备注只收不改状态，指定收件轮看到原文，成功后送达；改保护测试时失败且不标送达（`bug_fix_rerun_fix_and_rerun_locate_carry_notes`）。失败态录入不迁移也由库状态遍历覆盖。
- 排队阶段：整理/设计/评审/裁决不收件，首实现轮收到（`queued_task_note_reaches_the_first_impl_not_design`）。
- 多条：三条顺序和送达编号精确为 [1,2,3]（`several_notes_arrive_in_order`）。
- 旧模板无标记：实现有原文、审计不附核对；v9 有核对段（两个版本用例）。
- 崩溃：退出前不送达，重试两个 started 都带 [1]，最终仅一次 delivered（`crashed_impl_presents_the_note_again`）。
- 合并前门禁：auto=false/true 都先补轮；auto=true 最终完成（`note_during_merge_gate_adds_an_impl_round_before_merge`）。无门禁 Rebase 的四种 pending×authorization 组合由域用例验证。
- 审计退回：两次审计与第二实现简报都保留备注，仅一次 delivered；通过后窗口清空（`unhandled_note_stays_in_the_audit_brief_after_a_reject`）。
- 合并已开始三节点：warning/late 与永久未送达投影由库用例覆盖；pending 明确过滤 late，不补轮。Done/Cancelled 仍拒绝。

### 2. 设计点名的失败分支

- 未给 reopen 授权：端到端拒绝且 added 数为 0；Bug 预算用尽：域用例拒绝 reopen，合并门禁补轮返回 FixBudget，CLI 给出未送达备注及 extend 提示。
- 读盘失败：`handoff_note_unreadable_outcome_is_not_delivered` 删除结果文件后收割，送达数为 0，pending 保留 1。
- 超时：`handoff_note_timeout_keeps_notes_for_retry` 使用项目既有超时收尾夹具，出现 retry，未送达，重启收件仍为 [1]。
- 崩溃与回归保护守卫失败：以上端到端反例均验证不送达。
- 空输入、超长输入、终态：原 5 个库用例继续通过。
- 外部服务不可达：不适用，备注通道只操作本地账本和简报，不调用外部服务；模型真实行为评测另列 H-01。

### 3. 全部状态迁移

- 所有非终态节点录入保持原状态；AwaitMerge 独立切换到明确授权路径（调整 M-08 两处状态遍历，移出 AwaitMerge，转由 reopen 端到端断言）。终态拒绝。
- 未送达→启动仍未送达→成功且守卫通过后已送达→审计确认；崩溃/超时/读盘/守卫失败维持未送达；审计退回维持待核对，重呈现不重复送达。
- 审计末里程碑/复盘→实现→审计、Bug 复核→修复→复核复用 handoff；13 个域 scope_patch 用例逐条通过，含开放里程碑预算与关闭后补轮。
- Rebase无门禁/MergeGate通过→无交接时等待或自动合并，有交接时先实现；Bug MergeGate→修复或 FixBudget。需求 reopen→实现，Bug reopen→修复或拒绝；非 AwaitMerge reopen 拒绝。域表格用例覆盖。
- 收件方向：需求仅实现，Bug 定位/修复；旧协议送达不加强审计，v9 送达并核对；late 不进入收件集合。两类 Loop 的成功收尾均跑过。

### 4. 输入域边界

- 沿用库用例：空白拒绝、8000 UTF-16 单元合法、8001 拒绝、中文与 emoji 边界、原样保留首尾空格/换行、n 递增。单条与三条顺序分别由端到端验证。
- 无备注为空段；单条送达后不重复；预算剩余 1 与耗尽 0 的域测试通过；旧协议缺 marker 与 v9 有 marker 均覆盖。
- 前导零、闰日、跨年不适用：接口没有可输入编号/日期域，编号和时间由账本生成。

## 限制

H-01 的真实模型行为评测未运行（费用与人工验收项），静态评测通过不等价于真实模型改前红/改后绿。全量门禁及宿主快检留给宿主；未动冻结协议副本、.autome 配置、其它 worktree，未推送或启动下一会话。
