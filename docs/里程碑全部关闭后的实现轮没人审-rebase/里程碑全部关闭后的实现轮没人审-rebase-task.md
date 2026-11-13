# 任务：里程碑之外的实现轮要有人审（集成复验）

本任务的规则见 `docs/里程碑全部关闭后的实现轮没人审-rebase/protocol/loop-protocol.md` 与
`docs/里程碑全部关闭后的实现轮没人审-rebase/protocol/session-protocol.md`。

- 设计文档：`docs/里程碑全部关闭后的实现轮没人审-rebase/里程碑全部关闭后的实现轮没人审-rebase.md`
- 任务卡（用户写的，第一输入）：`docs/里程碑全部关闭后的实现轮没人审-rebase/里程碑全部关闭后的实现轮没人审-rebase-card.md`
- 任务编号：T-68；分支 `autome/里程碑全部关闭后的实现轮没人审-rebase`；固定协议 `protocol/v6`
- 任务整理日期：2026-11-13
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

下文把任务文档目录 `docs/里程碑全部关闭后的实现轮没人审-rebase/` 简写为 `<doc_dir>`。

## 1. 需求原文

> 里程碑全部关闭后的实现轮没人审：rebase 冲突修复、合并前门禁失败修复之后，审计轮以「无待审里程碑」空转

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理。

## 2. 目标与背景

**背景（任务卡原文整理）。** 需求 Loop 里，实现轮除了推进里程碑，还会在三种情况下被叫回来，而此时里程碑表往往已经
全部 `已完成`：

- rebase 冲突（`Inject::RebaseConflict`）；
- 合并前门禁失败（`Inject::GateFailed`；`[gate] run = "before_merge"` 下这是门禁问题暴露的**唯一**时机，会成为常态）；
- 用户在合并卡点纳入 Backlog 条目之外的手动重跑。

这些实现轮改的是产品代码和测试，但审计轮的简报与 prompt 都按里程碑表决定「审什么」：表里没有 `待审`，审计就写一句
「无待审里程碑」结束。T-61（`docs/门禁只在合并前跑一次-gate-run/`）的实际记录：

- 实现 #9（rebase 集成）改了 `launcher.rs`、`scheduler.rs`、两份 prompt 金样，并改了两个测试的前提（给 e2e 加
  `reuse_for_main_check = false`、旧模板兼容测试改成先选模板），还把 M-08 验收契约的「最新证据」指向自己的实现证据；
  审计 #9：「无待审里程碑，未改表」，一项也没复验。同一轮 rebase 丢了 `## protocol/v4` 标题（T-66），也没人发现。
- 实现 #10（修合并前门禁的 Clippy 失败）之后的审计 #10 同样没有对象。

现在兜底的只有合并前门禁的退出码。门禁拦得住编译与现有测试，拦不住「为了让旧断言继续成立而改测试前提」「冲突两侧
都保留」「版本标题丢失」这类改动。来源：T-61 复盘 L-05、L-06（`docs/门禁只在合并前跑一次-gate-run/lessons.md`）。

**目标。** 让「里程碑之外的实现轮」产生的改动有一个独立的审计对象。任务卡规定的六件事：

1. **内核识别「里程碑之外的实现轮」。** 实现轮开始时里程碑表没有 `开放` / `待审` 的行（即由 rebase 冲突、门禁失败或
   手动重跑触发），且该轮在任务文档目录之外产生了提交。记下这一轮的提交范围（轮前 HEAD..轮后 HEAD；rebase 轮用
   rebase 之后的等价范围，见第 4 条）。
2. **紧随其后的审计轮改为「集成复验」。**
   - 简报新增一段「本轮要审的集成改动」：触发原因（rebase 冲突的文件清单 / 门禁失败的命令与尾部）、提交范围、
     `git diff --stat`、改到的测试文件清单（单独列出，提醒审计关注测试前提的改动）。
   - prompt 增加对应分支：没有 `待审` 里程碑但简报有这一段时，审计必须逐项复验这些提交——改动是否只为解决触发原因、
     有没有削弱既有断言或改测试前提、有没有残留冲突标记（对范围内每个提交查 `^(<<<<<<<|>>>>>>>) `）、受影响里程碑的
     验收命令是否仍然通过——并写出审计证据文件；结论二选一：通过，或把受影响的里程碑退回 `开放`（计 reopen）。
   - 守卫：这种审计轮以「无待审里程碑」结束且没有新增审计证据时，判为未推进（沿用 `audit_made_no_progress` 一类的
     现有守卫，不新造失败类型更好），不让它静默放行。
3. **实现轮不得把已关闭里程碑验收契约的「最新证据」改指向自己的实现证据**（现有 `impl_closed_milestone` 守卫的同一
   精神）：检测到就按现有守卫失败，文案说明原因。
4. **rebase 冲突轮的提交范围要对得上。** 以 rebase 前分支与 rebase 后分支的 `git range-diff` 或「冲突文件清单 + 冲突
   解决提交」为准，设计轮定一个确定的、可测试的口径，不要求审计重审整条分支。
5. **协议文字同步。** `loop-protocol.md` 的实现与审计循环、`prompts/audit.md`、需要的话 `prompts/impl.md`；协议有改动
   就在 `protocol/CHANGELOG.md` 发新版本（独立的 `## protocol/vN` 标题），`behavioral` 条目带 eval。
6. **`docs/development/technical-design.md` 相关小节补一段。**

## 3. 验收标准

以下七条逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. e2e：任务所有里程碑关闭后制造一次 rebase 冲突 → 实现轮解决冲突并提交 → 审计轮的 prompt / 简报含「本轮要审的集成
   改动」段与冲突文件；审计通过后流程继续到合并卡点。
2. e2e：合并前门禁失败 → 实现轮修复 → 审计轮拿到集成复验简报；审计把里程碑退回 `开放` 时，Loop 回到实现轮并计
   reopen。
3. e2e 或单测：集成复验的审计轮不写证据、只写「无待审里程碑」→ 被守卫判为未推进（任务停下并给出可读原因）。
4. 单测：实现轮改已关闭里程碑的「最新证据」指向 → 守卫失败。
5. 里程碑之外的实现轮没有产生文档目录以外的提交时（例如只是 rebase 无冲突后的空轮），审计行为与今天相同。
6. 简报、prompt 金样更新；现有金样与 e2e 在不触发新路径时不改就通过。
7. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿，前两条由实现轮在会话里跑并记录退出码。

补充说明（从任务卡和调研得出，不是新范围）：

- 第 1、2 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、真实
  门禁后台作业，只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
- 第 1 条的「流程继续到合并卡点」：审计通过后经 rebase（以及配了门禁时的 MergeGate）到 `AwaitMerge`；设计须逐节点写出
  这条回路，并说明集成复验通过后再次 rebase / 再次 MergeGate 时不会无限循环。
- 第 3 条的「可读原因」：任务进入失败等人，`guard.failed` 事件的 `detail` 用中文说明这是集成复验轮、要审的提交范围
  是什么、两条出路是什么。
- 第 5 条是零行为变化条款：没有文档目录之外提交的里程碑外实现轮，其后的审计简报、prompt、守卫结论与今天逐字相同。
- 第 6 条的金样目录是 `crates/automed/tests/golden/`（`prompts/` 与 `briefs/`，由 `prompt_rendering.rs` 逐字节比对，
  说明见 `tests/golden/README.md`）。新增的文案要有金样；现有金样除非模板文本确实变了，否则不动。
- 第 7 条：本仓库 `[gate] run = "before_merge"`，完整门禁 `sh scripts/ci-local.sh` 只在合并前由 Autome 跑一次。
  `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings` 必须由实现轮在会话里自己跑，并把命令与
  退出码写进证据文件（T-61 L-01 / L-07 的教训：几秒能跑完的静态检查不能全推给合并前门禁）。

## 4. 范围

在范围内：

- `crates/automed`：
  - `scheduler.rs`——识别里程碑之外的实现轮、记录其提交范围与触发原因（账本事件）、把它们交给审计简报与 prompt、
    把新守卫所需的事实装进 `guards::Round`；
  - `brief.rs`——审计简报的「本轮要审的集成改动」段；
  - `guards.rs`——集成复验审计轮「未推进」守卫、实现轮改已关闭里程碑「最新证据」指向的守卫；
  - `launcher.rs`——若 prompt 分支需要内核按情形取舍（现有先例：`<!-- gate-run: … -->` 条件行）；
  - `git.rs`——取提交范围、`diff --stat`、逐提交冲突标记扫描等需要的只读 Git 辅助；
  - 单元测试、金样（`tests/golden/`、`tests/prompt_rendering.rs`）与 e2e（`tests/end_to_end.rs`）。
- `crates/autome-domain`：仅当状态机或域类型确实需要时（例如审计轮在全部 `已完成` 的表上退回里程碑后的转换、给审计
  会话带注入）。现有 `Audit` 会话结束后的转换已按状态块决定去向，设计须先核对它是否已经满足验收 2，再决定要不要改。
- 协议源 `protocol/`：`loop-protocol.md`（「实现循环」「审计轮」一带）、`prompts/audit.md`、需要时 `prompts/impl.md`、
  `CHANGELOG.md`（新的 `## protocol/v7` 一节）、`evals/`（`behavioral` 条目的用例）；`brief-map.toml` 仅当新增了需要
  摘进简报的小节标题时。
- 文档：`docs/development/technical-design.md` 相关小节（§5 任务状态机 / §6 Git 工作区操作 / §8 调度器，由设计选定
  落点）补一段。README 若有描述审计轮或守卫的现有段落与新行为矛盾，就地改那一句；没有就不动。

不在范围内（任务卡「不做什么」，不要做）：

- 不改有 `待审` 里程碑时的审计流程；
- 不让审计轮修改产品代码或测试（T-48 / T-62 的范围守卫 `guard_session_scope` 不变）；发现问题只能退回；
- 不处理 Bug Loop（修复轮之后本来就有独立复核）；
- 不做 T-66（`## protocol/vN` 版本标题）与 T-67（每轮快检 `[gate] quick`）的内容。

调研补充的边界（同样不要做）：

- **不改 `<doc_dir>/protocol/` 下的任何文件。** 那是本任务固定的协议副本（protocol/v6），本任务自己各轮照它执行；
  要改的协议是仓库根的 `protocol/`（源）。两者不要混淆。
- 不改 `.autome/` 下任何内容（见 §7 权威规则）；不改 `scripts/ci-local.sh`；不改 `apps/desktop`、`apps/web`、`plugins/`
  ——除非设计能指出某个现有读者在新行为下会显示错误内容，并逐条列出理由。
- 不新增 JSON-RPC 方法、不改已有方法的参数；不新增 `FailureReason` 变体（任务卡：不新造失败类型更好）。
- 不改被 `<!-- kernel-contract: … -->` 包住的协议区域（角色表、状态块、里程碑表、Backlog 与争议项、硬性边界）。
  这些区域的哈希编译在二进制里；本任务不需要改它们，设计若认为必须改，要连解析器与哈希一起列出并说明理由。
- 不顺手重构 `scheduler.rs`、`guards.rs`、`brief.rs`、`task.rs` 或与本需求无关的代码。

## 5. 硬性约束

1. **不触发新路径时零行为变化。** 有 `待审` 里程碑的审计轮、里程碑之外但没有文档目录外提交的实现轮之后的审计轮，
   其简报、prompt、守卫结论与今天逐字相同；现有金样与 e2e **不改就通过**（验收 5、6）。若确有必要改已有测试（例如
   结构体多了字段导致构造处编译不过），设计要逐条列出，且理由不能是「新行为与旧断言不符」。
2. **「里程碑之外的实现轮」的判定要确定、可测试。** 任务卡的口径是两条同时成立：(a) 实现轮**开始时**里程碑表没有
   `开放` / `待审` 的行；(b) 该轮在任务文档目录之外产生了提交。设计须写明：
   - (a) 用哪份数据判定（现状：上一轮结束时写的 `task.snapshot` 事件是「本轮开始时的表」的现有来源）；
   - (b) 的「该轮产生的提交」怎样界定——轮前 HEAD 取自哪里（现状：只有审计 / 复盘会话在 `session.started` 里记
     `scope_heads`，实现会话没有；每个角色会话过守卫后会写一条 `task.head`），Autome 自己的提交（简报提交
     `chore(autome): … 简报`、会话结束后的兜底提交）算不算在范围内；
   - 「任务文档目录之外」按路径前缀 `<doc_dir>/` 判定，口径与 `guard_session_scope` 一致（不做 rename 检测，源与目标
     两个路径都要在目录内才算目录内）。
3. **rebase 轮的范围口径（任务卡第 4 条）。** rebase 之后「轮前 HEAD..轮后 HEAD」不是线性范围——轮前 HEAD 不再是
   轮后 HEAD 的祖先。设计须在 `git range-diff <旧基>..<轮前 HEAD> <新基>..<轮后 HEAD>` 与「冲突文件清单 + 冲突解决
   提交」之间定一个确定的口径，并写明：范围怎样算出来、存在哪里、审计简报怎样呈现、测试怎样钉住它；rebase 之后
   实现会话又追加了普通提交时怎么算。**不要求审计重审整条分支。**
4. **审计轮仍然只写文档。** 集成复验的审计轮受 `guard_session_scope` 约束不变：文档目录外的被跟踪文件不得在其最终
   差异中改变。发现问题的唯一出口是把受影响的里程碑退回 `开放`、`reopen` 加 1（及协议规定的领域与收敛模式）。
   集成复验没有「验证缺口」式的当场改测试。
5. **结论二选一，且必须留下证据。** 通过：里程碑表不变，但必须新增一份审计证据文件；退回：受影响里程碑改为 `开放`
   并计 reopen，同样写证据。设计须定下通过时证据文件的命名（现状：`evidence_file` 守卫只认
   `M-*-r<k>-audit.md`，而集成复验轮没有「本里程碑」；简报的「本轮证据必须写入 …」一行也依赖焦点里程碑），并让
   简报把确切路径写出来（历史任务 `docs/审计简报没写出证据文件的确切路径/` 的同一教训）。
6. **守卫不静默放行，也不新造失败类型。** 集成复验的审计轮结束时表没动且没有本轮新增的审计证据 → 按现有
   `guard.failed` 机制让任务停下等人，优先复用 `audit_made_no_progress`（或同族现有 code），`detail` 写清原因。
   现状有两处显式放行需要与新守卫对齐而不是被绕过：`guards::check` 的 `unchanged_completed_table`（表全 `已完成` 且
   未变时免查审计证据）与 `audit_moved_the_table` 在没有 `待审` 时直接返回 `None`。只在「本轮是集成复验轮」时收紧，
   其余情形保持（约束 1）。
7. **「最新证据」指针守卫（任务卡第 3 条）。** 实现轮把**已完成**里程碑 `### M-xx 验收契约` 一带的「最新证据：…」
   改指向 `-impl.md` → 按现有守卫失败（任务卡：`impl_closed_milestone` 的同一精神）。设计须写明：检测的精确对象
   （哪一行、怎样对应到里程碑、怎样判定「改指向实现证据」）、与轮前文档比较的基线从哪来、文案；并确认正常实现轮
   （推进 `开放` 里程碑、给它写「最新证据：…-impl.md」）不被误伤。
8. **退回之后的回路要走得通。** 集成复验把一个 `已完成` 里程碑退回 `开放` 后：下一轮是实现轮、reopen 计数与收敛模式
   按协议更新、`milestone.contradicted` 等现有事件照常、实现预算照常计数；修好后回到普通审计，再到 rebase / MergeGate。
   设计须逐节点写出这条回路并用测试钉住（验收 2）。
9. **简报与 prompt 给的是事实，不是结论。** 「本轮要审的集成改动」段由内核从账本与 Git 算出：触发原因（冲突文件
   清单 / 门禁命令与尾部 / 手动重跑）、提交范围、`git diff --stat`、单独列出的测试文件清单。不转述实现轮证据里的
   说法（审计必须独立复验）。设计须定义「测试文件」的判定规则（本仓库：`crates/*/tests/`、`#[cfg(test)]` 所在源文件
   无法按路径区分；`apps/*/test/`、金样目录等），口径要确定、可测试，宁可多列。
10. **多仓库（工作区）项目不能被新代码弄坏。** 本任务在单仓库里实现，但内核同时服务工作区项目（`TaskLayout` 的
    `repos` / `scope_heads` 按仓库记）。提交范围、diff、冲突标记扫描要么按仓库逐个处理，要么设计明确写出工作区项目
    下的行为与测试，不能只在单仓库下成立。
11. **协议改动走协议的规矩。** 改 `protocol/` 就在 `protocol/CHANGELOG.md` 末尾加独立的 `## protocol/v7` 标题（T-61
    rebase 丢过 `## protocol/v4` 标题——提交前核对 `grep -n '^## protocol/v' protocol/CHANGELOG.md` 版本连续、无缺失）；
    每条改动一个 yaml 块，写 `kind`、`clause`、`evidence`（至少两个任务）、`predicted_impact`（`metric` 只能取
    `TaskMetrics::METRIC_NAMES` 里的）、`eval`；`behavioral` 条目必须带 `protocol/evals/<用例>/`（`case.yaml` +
    `graders/` + `scaffold.sh`，「改前红、改后绿」）。`cargo run -p automed -- protocol eval protocol/` 静态门必须通过；
    `--behaviour` 要花钱，不在会话里跑。prompt 模板改了就看过 diff 再用
    `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering` 接受金样，和协议改动放同一个提交。
12. **`autome-domain` 保持纯净。** 不做 I/O、不读文件、不碰 SQLite、不调 Git；需要的事实由 `automed` 读好再传入。
    账本 schema（表结构）不改；新增事件种类可以，须在设计里列出名字、载荷与读者。
13. **Bug Loop 不变。** 定位 / 修复 / 复核的 prompt、简报、守卫与转换一字不动；新逻辑只在需求 Loop（`kind = 需求`）
    的实现 / 审计轮生效，设计须写明区分点并有用例钉住。
14. **并行任务与 rebase。** 本仓库 `[loop] parallel = 3`，其它任务也在改 `scheduler.rs`、`guards.rs`、`protocol/`。
    改动保持在必要范围内，不做无关的移动或重排。本任务自己被 rebase 时，证据里要记录对 `main..HEAD` 每个提交执行
    `git grep -nE '^(<<<<<<<|>>>>>>>) '` 的结果且为空（L-06）；`protocol/CHANGELOG.md` 冲突时两侧的版本标题都要在。
15. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过。不要为了让门禁变绿去改
    `ci-local.sh`、跳过步骤或削弱既有断言。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

行号是 2026-11-13 本分支（基于 `b98bbbd`，HEAD `02c0c40`）上的位置，会随 rebase 漂移，以符号名为准。

**状态机（`crates/autome-domain/src/task.rs`）。**

- `Inject`（约第 599 行）：`RebaseConflict { files }`、`GateFailed { head, command, tail }`、`Decisions`、
  `DesignFeedback` 等。注入只随 `Action::StartRole { role, inject }` 给下一个角色会话。
- rebase 冲突（约第 1276 行）：`(Node::Rebase, CoreStepResult::Conflict { files, .. })` → `Implement`，
  `StartRole { Impl, inject: RebaseConflict }`。
- 门禁失败：`after_gate`（约第 1380 行）里的闭包 `back_to_impl`，`(Gate | MergeGate, Failed)` → `Implement`，
  `inject: GateFailed`。
- 手动重跑：`(Failed | Stopped, RerunFrom { node: Implement | Audit | … })`（约第 1027 行）→ 该节点，`inject: None`。
  合并卡点 `AwaitMerge` 上「纳入 Backlog」走的是 `Trigger::Merge` → `press_merge` → `start_impl`，带
  `Inject::Decisions`（新增里程碑；任务卡把它排除在「里程碑之外的实现轮」之外）。
- 实现会话结束后：配了门禁且 `each_round` → `Gate`；否则审计开启 → `Audit`（`inject: None`）；审计关闭 →
  `after_implement_without_audit`。审计会话结束后按状态块：全部 `已完成` → `after_the_loop`（复盘开启 → `Retro`，
  否则 `Rebase`）；有未完成且 k < N → `start_impl`（即审计把 `已完成` 里程碑退回 `开放` 后，现有转换已经回实现轮）；
  k ≥ N → `Failed(ImplBudget)`。
- rebase 成功后配了门禁 → `MergeGate`，否则 `AwaitMerge`。

**内核步骤与账本（`crates/automed/src/scheduler.rs`、`git.rs`）。**

- `git::rebase`（`git.rs` 约第 646 行）：冲突时取 `conflicted_files` 后**立刻 `rebase --abort`**。所以冲突修复轮开始时
  工作树里没有冲突标记、分支仍在 rebase 之前的位置；实现会话按 prompt（`launcher.rs` 的 `REBASE_YOURSELF`）自己
  `git rebase <默认分支>` 并解决冲突。即：分支的改写发生在实现会话**内部**，内核事先不知道 rebase 后的提交。
- 账本事件：`rebase.conflicts { files, count }`（约第 1826 行，同一文件集连续 3 次即失败）；`session.started`（带
  `inject`，`carried_inject` 约第 617 行从它读回；审计 / 复盘会话另带 `scope_heads: [{repo, head}]`，约第 2941 行；
  实现会话**不带**）；`task.snapshot`（`guards::Snapshot { milestones, retro_lines, design_bytes }`，每个角色会话过
  守卫后写）；`task.head { sha }`（同一位置写）；`guard.failed` / `guard.warning { code, detail, role }`；
  `milestone.contradicted`。
- `apply_guards`（约第 668 行）组装 `guards::Round`；`repairing` 仅在注入是 `GateFailed` 时为真。
- `guard_session_scope`（约第 433 行，T-48 / T-62）：审计 / 复盘会话相对 `scope_heads` 的
  `git diff --name-only --no-renames -z <base> HEAD` 里出现 `<doc_dir>/` 之外的路径即协议失败。
- `write_brief`（约第 3098 行）：读设计文档状态块、审计文档、证据文件名，调 `brief::build`，把简报写到
  `<doc_dir>/brief/<角色>-<轮次>.md` 并提交（`chore(autome): T-n <角色> 简报`）。这个提交落在任务分支上、路径在
  文档目录内。

**守卫（`crates/automed/src/guards.rs`，纯函数、无 I/O）。**

- `check`（约第 106 行）：实现轮跑 `evidence_file("impl")`、`no_closing_a_milestone`（code `impl_closed_milestone`）、
  `impl_had_a_target`（code `impl_had_no_target`；`repairing` 或轮前表全 `已完成` 时放行）、`retro_one_line`。
  审计轮：轮前表非空、全 `已完成` 且本轮没改表（`unchanged_completed_table`）时**免查** `evidence_file("audit")`；
  然后 `audit_moved_the_table`（code `audit_made_no_progress`）、`retro_one_line`、`audit_stayed_in_its_column`
  （code `audit_edited_the_design`，警告）。
- `audit_moved_the_table`（约第 222 行）：表里没有 `待审` 时直接返回 `None`——注释写明这是「最后一个里程碑关闭后的
  那个审计轮」的正常出口。这正是任务卡说的空转能通过守卫的原因。
- `evidence_file`（约第 149 行）：要求存在 `M-*-r<k>-<角色>.md`（k 取状态块的 `implementation-round`），code
  `evidence_missing`。
- `Round` 现有字段里没有「本轮提交范围」「设计文档轮前文本」之类的东西；`design_changed_outside_milestones` 由
  `scheduler.rs` 算好传入，是可参照的先例。

**简报（`crates/automed/src/brief.rs`）。**

- `build`（约第 287 行）的段落顺序：`## 本里程碑` → `## 上一轮对手轮的结论` → `## 本轮相关的协议小节` →
  `## 预算与用户表态` →（有门禁证据时）`## 宿主门禁`。
- `focus`（约第 268 行）在全部 `已完成` 时返回 `None`，此时「本里程碑」段只有一句「还没有里程碑表，或者所有里程碑都
  已完成。」，也不会写「本轮证据必须写入 …」那一行。
- `Inputs.gate` 是「已渲染好的字符串」由调用方传入的先例；`gate_evidence` / `gate_line_for_mode`（约第 28–110 行）
  处理 `before_merge` 下「本轮没有宿主门禁结果」的文案。

**prompt（`protocol/prompts/`，由 `crates/automed/src/launcher.rs` 渲染）。**

- `audit.md`：开头要求「找出状态为 `待审` 的里程碑」，结论三选一，「没有第四种结论，里程碑表必须动」。没有针对
  「无 `待审`」情形的任何文字。`<!-- gate-run: each_round | before_merge -->` 包住的行按项目配置取舍，是「同一模板按
  情形渲染不同文字」的现有机制。
- `impl.md`：写「推进编号最小的『开放』里程碑」；rebase 冲突 / 门禁失败时的补充文字不在模板里，而由
  `launcher.rs` 按 `Inject` 追加（约第 518–600 行）：`RebaseConflict` → 冲突文件清单 + `REBASE_YOURSELF` +
  「保持里程碑状态与设计文档一致」；`GateFailed` → 提交、命令、尾部 + 「本轮是修复轮，不推进新的里程碑…照常写本轮的
  证据文件」。两段都没有提「不要改已关闭里程碑的最新证据指针」。
- 协议正文 `loop-protocol.md`：「审计轮」小节写三种结论与「没有第四种结论」；「审计造的检查归谁」写复验范围；
  「Autome 会检查什么」表列出五条内核检查。「角色」表在 `kernel-contract: roles` 区内（审计：`已完成` 或退回 `开放`）。
- `brief-map.toml`：审计角色摘 `共同原则`、`文件`、`实现循环`、`Backlog 与争议项` 与会话协议的 `状态块`。

**测试现状。**

- `crates/automed/tests/end_to_end.rs`（约 8.8k 行）：相关现有用例
  `feature_before_merge_two_milestones_run_the_gate_once_before_the_merge`、
  `feature_before_merge_a_failing_merge_gate_returns_to_implement_and_merges_after_the_fix`（约第 2870、2972 行），
  可作新 e2e 的脚手架参照；替身模型脚本在 `tests/fixtures/`。
- `crates/automed/tests/prompt_rendering.rs`：`before_merge_audit_brief_matches_its_golden`（简报金样
  `golden/briefs/audit-before-merge.md`、`audit-before-merge-quick.md`）、`before_merge_role_prompts_match_their_goldens`
  （`golden/prompts/audit.md`、`audit-before-merge.md`、`impl.md`、`impl-before-merge*.md` 等）。
- `guards.rs` 内的单测：`a_round_repairing_a_red_gate_needs_no_open_milestone` 等，`Round` 在测试里按字段字面量构造
  （`guards.rs` 两处、`brief.rs` 一处）——给 `Round` 加字段会让这些构造处编译不过，属于约束 1 允许的那类改动，但要列出。
- `protocol/evals/` 现有 20 个用例目录（如 `audit-keeps-project-files-unchanged`、`auditor-rerun`、
  `impl-handles-audit-return`），结构是 `case.yaml` + `graders/` + `scaffold.sh`。
- `protocol/CHANGELOG.md` 当前最大版本 `## protocol/v6`（v1..v6 标题齐全）。

**T-61 的原始证据（只读参照，不要改）。**

- `docs/门禁只在合并前跑一次-gate-run/lessons.md`：L-05（集成轮无人审，`level: protocol`）、L-06（rebase 后 7 个提交
  带冲突标记，标记直到末尾另开的提交才清掉；`level: rule`）。
- `docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r9-impl.md`：实现 #9 的集成证据。
- 相邻历史任务：`docs/没有待审里程碑的审计轮被判协议失败/`（今天「无待审即放行」行为的来历——收紧时不能把它修掉的
  那个误判带回来）、`docs/审计-prompt/`（T-62，审计只写文档）、`docs/rebase-冲突退回实现不受预算约束/`、
  `docs/审计简报没写出证据文件的确切路径/`。

## 7. 项目背景摘要（任务整理时）

**项目。** autome-v2 是 Autome 2.0：一个本机守护进程（内核）按固定协议调度 Claude Code / Codex 等 CLI 会话，跑
「设计 → 评审 / 裁决 → 实现 / 审计 → 复盘 → rebase → 合并」的需求 Loop 与「定位 → 修复 → 复核」的 Bug Loop。
本仓库用自己的 Loop 开发自己。

**技术栈与布局。**

| 路径 | 是什么 |
|---|---|
| `crates/autome-domain` | 纯域：状态机（`task.rs`、`bug_loop.rs`）、状态块解析、配置类型、协议与 changelog 解析。无 I/O |
| `crates/automed` | 守护进程：调度器（`scheduler.rs`）、会话启动器（`launcher.rs`）、守卫（`guards.rs`）、简报（`brief.rs`）、Git 操作（`git.rs`）、门禁、SQLite 账本（`store.rs`）、JSON-RPC（`dispatch.rs`、`ipc/`）。协议经 `include_str!` 编译进二进制 |
| `crates/autome-cli` | `autome` 命令行客户端 |
| `protocol/` | 协议源：`loop-protocol.md`、`session-protocol.md`、`bug-protocol.md`、`prompts/*.md`、`brief-map.toml`、`evals/`、`CHANGELOG.md`、`README.md`（怎么改协议） |
| `apps/desktop` | Electron 外壳，`node --test test/*.test.js` |
| `apps/web` | React Router 8 + Vite 面板，`vitest` / `eslint` / `tsc` |
| `plugins/` | Claude Code 插件（`autome`、`loop`） |
| `docs/development/` | `requirements.md`、`technical-design.md`（中文）；`docs/adr/` 三份 ADR；`docs/<任务>/` 各任务的文档目录 |
| `scripts/ci-local.sh` | 本地 CI，也是本项目的宿主门禁命令 |

Rust workspace：edition 2024，工具链固定 `1.93.1`（`rust-toolchain.toml`，含 rustfmt、clippy）。本仓库没有 Git 远端，
`.github/workflows/ci.yml` 从未真正运行，`scripts/ci-local.sh` 是唯一执行这些检查的地方。

**测试命令。**

- 相关子集（实现轮 / 审计轮在会话里跑的）：
  - `cargo test -p automed --lib guards`、`cargo test -p automed --lib brief`、`cargo test -p autome-domain`
  - `cargo test -p automed --test prompt_rendering`（金样；接受新金样用 `AUTOME_UPDATE_GOLDEN=1`，先看 diff）
  - `cargo test -p automed --test end_to_end <用例名>`（真实 Git 的 e2e，单跑相关用例；整套很慢）
  - `cargo run -p automed -- protocol eval protocol/`（协议静态门）
  - `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`（验收 7 要求会话里跑并记退出码）
- 完整门禁：`sh scripts/ci-local.sh`（Format、Clippy、`cargo test --workspace --all-targets`、Desktop、Contract、
  Web lint / typecheck / test / build）。本仓库 `.autome/config.toml`：`[gate] command = "sh scripts/ci-local.sh"`、
  `run = "before_merge"`、`timeout_minutes = 30`——由 Autome 在合并前跑一次，会话不要自己跑完整门禁。
- 超过 5 行的命令输出重定向到 `.autome/output/`（已被 `.gitignore` 忽略），只读头尾。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不修改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不操作 `.worktree/` 下其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（规则应是可检验的约束），没有项目专属规则条目。
- `docs/agent-project-profile.md`：不存在。
- `protocol/README.md`：改协议的四步流程与「改这里也改不了」的三样东西（内核契约区、状态机 / 守卫代码、项目规则）。
- `crates/automed/tests/golden/README.md`：金样怎么更新、为什么不能不看 diff 就刷新。
- `docs/adr/0003-protocol-is-built-in.md`：协议只有 `protocol/` 一个来源，编译进 `automed`。

**提交约定。** 提交信息沿用仓库现有风格（`feat(scope): …`、`fix(scope): …`、`docs(<slug>): …`、`test: …`）；
`.githooks/post-commit` 会在提交后后台跑 `ci-local.sh`（`NO_CI=1 git commit …` 可跳过单次）。
