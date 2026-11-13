# 里程碑之外的实现轮要有人审：rebase 冲突 / 合并前门禁修复之后的审计轮改为集成复验

status: 实现中
design-round: 1/15
implementation-round: 1/25
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01 独立审计通过后，实现 M-02 的集成复验守卫、证据指针守卫与审计启动接线

任务文件：`docs/里程碑全部关闭后的实现轮没人审-rebase/里程碑全部关闭后的实现轮没人审-rebase-task.md`

任务卡：`docs/里程碑全部关闭后的实现轮没人审-rebase/里程碑全部关闭后的实现轮没人审-rebase-card.md`

下文 `<doc_dir>` 指任务文档目录；「任务 §n」指任务文件第 n 节，「约束 n」指任务 §5 第 n 条，「验收 n」指任务 §3 第 n 条。
行号以 2026-11-13 的 HEAD `cca5be8` 为准，会随 rebase 漂移，以符号名为准。

## 背景与现状

需求 Loop 的实现轮有三种情形是在**里程碑表已全部 `已完成`** 时被叫回来的：rebase 冲突（`Inject::RebaseConflict`）、
门禁失败（`Inject::GateFailed`，本仓库 `run = "before_merge"` 下只在合并前出现，会成为常态）、手动重跑（`RerunFrom`，
无注入）。这些轮次改产品代码和测试，但随后的审计轮按里程碑表决定审什么，表里没有 `待审` 就写一句「无待审里程碑」
结束，守卫也放行。T-61 的实现 #9（rebase 集成：改了 `launcher.rs`、`scheduler.rs`、两份金样、两个测试的前提，并把 M-08
的「最新证据」指向自己的实现证据）与实现 #10（修 Clippy）之后的两次审计都没有对象（L-05、L-06）。

已核对的代码现状（只列本设计依赖的事实）：

- **转换（`autome-domain/src/task.rs`）。** `(Rebase, Conflict)` → `Implement` 带 `RebaseConflict`；`after_gate` 的
  `back_to_impl` → `Implement` 带 `GateFailed`；`Implement` 会话结束：`each_round` 门禁 → `Gate`，否则审计开启 → `Audit`
  （`inject: None`）。`Audit` 会话结束：`all_milestones_done()` → `after_the_loop`（复盘开 → `Retro`，否则 `Rebase`）；
  否则 `impl_round < budget` → `start_impl`；否则 `Failed(ImplBudget)`。**结论：审计在全 `已完成` 的表上退回一个里程碑
  后，现有转换已经回到实现轮（验收 2），`autome-domain` 不需要改。**
- **守卫（`automed/src/guards.rs`）。** `check` 对审计轮：轮前表非空、全 `已完成` 且本轮没改表时免查
  `evidence_file("audit")`；`audit_moved_the_table` 在表里没有 `待审` 时直接返回 `None`。两处一起构成空转放行。
  `evidence_file` 只认 `M-*-r<k>-<角色>.md`（k 取状态块）。`Round` 有四处字面量构造：`scheduler.rs` `apply_guards`、
  `guards.rs` 测试 helper `round()` 与 `the_first_round_has_nothing_to_compare_against_and_is_not_penalised`、
  `brief.rs` 一个测试。
- **调度器（`automed/src/scheduler.rs`）。** `reap_session` 先 `sweep_commit`（会话没提交的改动由 Autome 提交为
  `chore(autome): <角色> #n 未提交的剩余改动`），再 `guard_session_scope`，再 `apply_guards`。需求 Loop 的 Bug 会话在
  `apply_guards` 之前就走 `deliver_bug_outcome` 返回，**`apply_guards` 只见到 `SessionKind::Role` 会话**。`apply_guards`
  对每个通过解析的角色会话写 `task.snapshot` 与 `task.head {sha}`（只记文档仓库的 HEAD）；崩溃的会话不走这里。
  `launch_session` 的顺序是：`write_brief`（提交 `chore(autome): T-n <角色> 简报`）→ `build_prompt` → 取
  `scope_heads`（只给审计 / 复盘）→ 启动 → 写 `session.started`。也就是说**在简报提交之后、会话启动之前取到的 HEAD 天然
  不含 Autome 的简报提交**。
- **Git（`automed/src/git.rs`）。** `git::rebase` 冲突时取 `conflicted_files` 后立即 `rebase --abort`；实现会话按
  `REBASE_YOURSELF` 自己 `git rebase <默认分支>`。所以 rebase 冲突轮的分支改写发生在会话内部，轮前 HEAD 不是轮后 HEAD
  的祖先。`TaskLayout` 的每个 repo slot 有 `name`、`worktree`、`is_docs`、`base`（rebase 的目标分支）。
- **简报（`brief.rs`）。** `build` 段落：`## 本里程碑` → `## 上一轮对手轮的结论` → `## 本轮相关的协议小节` →
  `## 预算与用户表态` →（有时）`## 宿主门禁`。`focus` 在全 `已完成` 时返回 `None`。`Inputs.gate` 是调用方预先渲染
  好的字符串（先例）。`brief-map.toml` 给审计摘 `共同原则`、`文件`、`实现循环`、`Backlog 与争议项`；`section()` 按
  「标题以映射名开头」取第一处，**`### 审计轮` 在 `## 实现循环` 之下，改它会改变所有审计简报的金样**。
- **prompt（`launcher.rs`）。** 模板来自任务固定的协议副本；`select_gate_run` 处理 `<!-- gate-run: … -->` 独占行标记，
  遇到未知的 `<!-- gate-run:` 行报错。注入文字由 `build_prompt` 在代码里追加。评估工具 `protocol/eval_run.rs` 的
  `render_prompt` 也自己渲染模板。
- **现有 e2e 的两个约束（读测试得到）。** `feature_before_merge_a_failing_merge_gate_returns_to_implement_and_merges_after_the_fix`
  与 `brief_before_merge_audit_brief_never_shows_a_stale_result` 里，第 8 步是全 `已完成` 表上的门禁修复轮，提交了文档
  目录外的 `gate-ok`——**它们会走到新路径**。替身会话（`World::EVIDENCE`）每步都写 `M-01-r$k-impl.md` 与
  `M-01-r$k-audit.md`。后者还断言审计简报**不含失败提交 SHA 的前 10 位、不含「：未通过」**。新设计必须让这两个测试
  不改就通过（约束 1：改测试的理由不能是「新行为与旧断言不符」）。

## 目标与非目标

**目标**（对应任务 §2 的六件事）：

1. 内核在实现会话结束时判定它是不是「里程碑之外的实现轮」，并把触发原因与提交范围记进账本（`integration.range`）。
2. 紧随其后的审计轮成为「集成复验」：简报多一段「本轮要审的集成改动」，prompt 多一个分支，守卫不再放行
   「无待审 + 无证据」。
3. 实现轮把已完成里程碑的「最新证据」改指向 `-impl.md` 时守卫失败（code 沿用 `impl_closed_milestone`）。
4. rebase 冲突轮的提交范围有确定、可测试的口径，不要求审计重审整条分支。
5. 协议源 `protocol/` 发布 `protocol/v7`：`prompts/audit.md` 分支、`loop-protocol.md` 新增 `## 集成复验` 与检查表两行、
   `CHANGELOG.md` 条目与 eval 用例。
6. `docs/development/technical-design.md` §6、§8 各补一段。

**非目标**（任务 §4「不在范围内」照抄，外加本设计的取舍）：

- 不改有 `待审` 里程碑时的审计流程；不让审计改产品代码或测试（`guard_session_scope` 不变）；不处理 Bug Loop；不做
  T-66 / T-67。
- 不改 `<doc_dir>/protocol/`（本任务固定的 v6 副本）、`.autome/`、`scripts/ci-local.sh`、`apps/*`、`plugins/`。
- 不新增 JSON-RPC 方法、不新增 `FailureReason` 变体、不改账本表结构、不改任何 `kernel-contract` 区域。
- 不改 `autome-domain`（见上「转换」一条的核对结论）。
- 不改 `prompts/impl.md`：指针守卫由代码检查，按协议「能用代码检查的就不写成『不得』」，只在 `loop-protocol.md` 的
  「Autome 会检查什么」里加一行。
- 内核不自己扫冲突标记（由审计按 prompt 逐提交执行；内核化放 Backlog B-01）。
- 审计角色被关闭的项目（`roles.audit` 关）不在本任务处理范围（Backlog B-02）。

## 方案

### 1. 「里程碑之外的实现轮」的判定（约束 2）

在 `apply_guards` 里，仅当会话是 `SessionKind::Role { role: Role::Impl }`（需求 Loop；Bug 会话到不了这里，约束 13）时
判定。四条同时成立才算：

| 条件 | 数据来源 |
|---|---|
| (a) 本轮开始时表非空且全部 `已完成` | `apply_guards` 已读出的 `before`（最近一条 `task.snapshot`，即上一轮角色会话留下的表） |
| (b) 本轮结束时表仍全部 `已完成` | 会话写的状态块（`task_metrics::snapshot(status)`） |
| (c) 本轮提交范围「列出」或「消失」非空（§2） | Git，按仓库逐个算 |
| (d) 「列出」或「消失」里至少一个提交改了 `<doc_dir>/` 之外的路径 | 同上；路径口径与 `guard_session_scope` 一致 |

(d) 把「消失」也算进来：rebase 冲突时实现会话选择上游版本、使自己原来的产品提交变成空提交而被丢弃，丢掉的正是
已验收的产品改动，必须有人复验（D1-P01）。

(b) 的作用：用户表态（`Inject::Decisions`）的实现轮会新增 `开放` 里程碑并推进成 `待审`，那时审计有正常对象，不进集成
复验；任何把表改成非全 `已完成` 的实现轮同理。所以不需要按注入种类排除。

**「本轮」从哪个 HEAD 算起（H0）。** 给实现会话的 `session.started` 增加 `start_heads: [{repo, head}]`，取法与审计的
`scope_heads` 相同、位置相同（简报提交之后、启动之前），所以**简报提交不在范围内**。崩溃重试会再启动一次实现会话、
再记一次 `start_heads`；为了不丢掉崩溃那次已被兜底提交的改动，**H0 取「最近一条 `task.head` 之后的第一条实现会话
`session.started` 的 `start_heads`」**——崩溃的会话不写 `task.head`，所以这正是本轮第一次尝试开始时的 HEAD。
没有 `start_heads`（升级前启动的会话）→ 不判定、不记事件（不猜）。

**兜底提交算不算。** 算。`sweep_commit` 在 `apply_guards` 之前提交，内容就是会话自己的改动，落在 H1 里，按 (d) 的路径
规则参与判定。

**文档目录之内 / 之外。** 与 `guard_session_scope` 相同：`git diff --name-only --no-renames -z <父> <提交>`（不做
rename 检测，源与目标两个路径都出现在列表里），对文档仓库 slot，路径以 `<doc_dir>/` 开头算目录内；对工作区里的成员
仓库 slot（`!is_docs`），所有路径都算目录外。合并提交按第一父提交比较（多列不少列）。

### 2. 提交范围的口径（约束 3、任务卡第 4 条）

对每个仓库 slot，H1 = 会话结束时（兜底提交之后）的 HEAD，B1 = `git merge-base H1 <slot.base>`：

```text
列出：git rev-list --reverse --cherry-pick --right-only <H0>...<H1> ^<B1>
消失：git rev-list --reverse --cherry-pick --left-only  <H0>...<H1> ^<B1>
```

- **线性情形**（门禁修复、手动重跑，会话没有 rebase）：H0 是 H1 的祖先，左侧为空，「列出」= `H0..H1`。
- **rebase 情形**：`^B1` 排除默认分支上的提交；`--cherry-pick` 去掉补丁与 rebase 前某个提交相同（patch-id 相同）的
  提交。剩下的就是「冲突解决后补丁变了的提交 + 会话 rebase 之后追加的普通提交」，正是任务卡说的「冲突解决提交」
  加上「rebase 后追加的提交」。「消失」列出 rebase 前有、rebase 后找不到同一补丁的提交（被改写或被丢掉），简报单列。
- 设计轮在临时仓库里实测过：分支 T1（干净）、T2（与 main 冲突）、T3（文档），会话 rebase 解决 T2 后追加 T4，「列出」
  恰为 `T2, T4`，「消失」为 `T2`（旧版本）；`H1...H1` 为空。评审轮另用临时仓库实测了两个反例（冲突中选上游而丢掉自己的
  产品提交；patch-id 空白归一化把改写后的提交判等价），已由上面两条规则覆盖，并成为 M-01 的用例。
- **冲突文件的保守纳入（D1-P01）。** patch-id 会归一化空白，所以「冲突解决后补丁被判等价」是漏审来源（字符串字面量里
  `"hello world"` 被解成 `"helloworld"`，patch-id 不变）；只凭 patch-id 一致不能断定无需复验。当 H0 不是 H1 的祖先
  （会话 rebase 过）时加一条规则：**凡是 `B1..H1` 中、不在 H0 历史里（被重写过或新增）、且改到了冲突文件的提交，不论
  patch-id 是否等价，一律并入「列出」**。冲突文件取自该实现会话的 `carried_inject`（`RebaseConflict.files`）。触发原因
  不是 rebase 冲突而会话自己 rebase 了（冲突文件未知）时只按 patch-id，这是已知残余（R1）。没有冲突文件的干净 rebase
  不受这条影响，零变化路径保持。
- **「消失」不只是显示项。** 每个「消失」提交记 `paths` 与 `stat`（口径同「列出」，用被丢弃提交自己的 diff）；改到目录外路径的
  「消失」提交使本轮成为集成轮（§1 (d)），审计能看到哪个产品改动在冲突解决中被放弃。
- 会话违反 prompt 用了 `git merge`：合并提交在「列出」里（不加 `--no-merges`），按 (d) 用第一父比较。
- 选这个而不选 `git range-diff`：`range-diff` 的输出是给人看的模糊配对，难以逐字断言；`rev-list --cherry-pick` 是
  确定的集合运算，测试能钉住，一条公式覆盖全部三种触发原因。

**「测试文件」的判定（约束 9，宁可多列）。** 范围内被「列出」提交改到的文档目录外路径，满足任一条即列入：

1. 路径中有一段是 `tests`、`test`、`__tests__`、`golden`、`fixtures`、`evals`；
2. 文件名匹配 `*.test.*`、`*.spec.*`、`*_test.*`、`test_*`；
3. 该路径在某个「列出」提交的 diff 里，有一行增删（不含 `+++`/`---` 头）包含 `#[test]`、`#[cfg(test)]`、`assert`、
   `expect(` 之一；
4. **文件本身带内联测试（D1-P04）：** 该路径在 H1 的版本或在该提交第一父的版本里含 `#[cfg(test)]` 或 `#[test]`
   （`git show <rev>:<path>` 取文本只扫文本，不要求本次增删了标记）。这一条覆盖「只改了测试准备条件或 helper、没碰断言
   与测试标记」的修改（例如把 `scheduler.rs` 测试用配置里的 `reuse_for_main_check` 改成 `false`）。读不出内容时
   （Git 报错、二进制）宁可列入。实现时可参考 `scheduler.rs` 的 `test_section_start`。

### 3. 账本事件 `integration.range`（新事件种类，表结构不变，约束 12）

实现会话满足 §1 四条时，在 `apply_guards` 写 `task.snapshot` 之后追加：

```json
{
  "session_id": "<本轮第一次尝试的实现会话 id>",
  "impl_round": 9,
  "evidence": "M-08-r9-audit.md",
  "trigger": {"kind": "rebase_conflict", "files": ["src/a.rs"]}
           | {"kind": "gate_failed", "command": "sh scripts/ci-local.sh", "tail": "…"}
           | {"kind": "decisions"} | {"kind": "none"},
  "repos": [{
    "repo": "", "start": "<H0>", "end": "<H1>", "base": "<B1>",
    "commits": [{"sha": "…", "subject": "…", "paths": ["…"], "stat": "…"}],
    "dropped": [{"sha": "…", "subject": "…", "paths": ["…"], "stat": "…"}],
    "test_files": ["…"], "commits_omitted": 0, "dropped_omitted": 0
  }]
}
```

- `impl_round` 取实现会话写的状态块 `implementation-round` 的 k；`evidence` = `M-<表最后一行 ID>-r<k>-audit.md`，
  简报照此写出确切路径（约束 5）。
- `trigger` 取该实现会话 `carried_inject`；`gate_failed` **不存提交 SHA**（理由见 §5 的格式规则）。
- 只列改了目录外路径的提交；`stat` 为 `git show --stat=120 --format= <sha> -- . ':(exclude)<doc_dir>/'`（成员仓库不加
  exclude），每个提交最多 30 行，超出写「其余 n 行见 `git show --stat <sha>`」；`commits` 与 `dropped` 各最多 50 个，超出数量写入 `commits_omitted` / `dropped_omitted`，供简报明确展示。
- 「列出」与「消失」里都没有改目录外路径的提交的仓库不进 `repos`；`repos` 为空即不写事件（验收 5 的「空轮」）。
- **读取失败的两级处理（D1-P03）。** ① 判定「是不是集成轮」所需的事实（H0 / H1 / B1、`rev-list` 的两个集合、各提交的路径列表）
  读取失败：范围未知，不能当作已证实的空轮，也不能退回今天的放行——在 `apply_guards` 传播错误前，用既有 `FailureReason::Config` 记录 Failed 与 `task.updated`，让任务停在可 `rerun` 的状态；错误信息用中文写明读取失败的步骤。不能只用 `?`：收尾时会话已结束，单纯传播会留下没有运行会话的 Active/Implement。② 已确认是集成轮之后才读取的展示信息（`stat`、测试文件内容扫描）失败：**不取消
  集成身份**，该字段写 `（读取失败：<原因>）`，扫描失败的路径一律列入测试文件；事件照常写入，简报照常出现该段并带着缺失项
  的说明。两级都不新增 `FailureReason`，也不标「协议失败」。
- 读者：只有 `scheduler.rs`（审计启动时，§4）。桌面端与面板按事件种类通用展示，不需要改（实现轮用
  `grep -rn "guard.failed\|task.snapshot" apps/` 核对没有按种类白名单过滤的读者，结论写进证据）。

### 4. 哪个审计轮是集成复验（选择与版本门）

审计会话启动时（`launch_session`，`kind == Role { Audit }`）调 `integration_for_audit`：

1. 取最近一条 `integration.range` 事件 E；
2. 取最近一条 `kind` 为实现的 `session.started`，记为 I；S = 严格早于 I 的最近一条 `task.head` 之后的第一条实现
   `session.started` 的会话 id（I 所在尝试组的第一次尝试，与 §1 同一口径）。审计收尾写的 `task.head` 不改变 S，因为 S
   是相对于最近实现会话 I 向前定位的（评审 R1-N01 的澄清）；
3. **E 存在、`E.session_id == S`，且本任务固定协议的 `prompts/audit.md` 含独占行 `<!-- integration-audit -->`** 时，
   本审计是集成复验；否则不是。

第 2 条保证 E 只对紧随其后的审计生效：之后任何新的实现轮都会让它失效（例如集成复验退回里程碑后的正常实现轮）。审计
崩溃重试、或任务停下后 `rerun` 审计时，结论不变，仍拿到同一段。第 3 条是版本门：固定在 v6 及以前的任务（包括本任务
自己、以及升级时正在跑的任务）模板里没有这个标记，**简报、prompt、守卫都与今天逐字相同**——协议原则 6「版本固定」
在这里是代码属性。

结论写进审计的 `session.started`：`"integration": {"impl_session": S, "impl_round": k, "evidence": "…", "summary": "…",
"evidence_stamp": {"exists": false, "mtime_ns": null, "len": null}}`（`summary` 为一行文字：触发原因 + 每个仓库
`H0短..H1短，n 个提交`；`evidence_stamp` 是启动时简报指定的那个证据路径的状态，用途见 §7）。简报、prompt、守卫三处都从
这一个值读，不各算各的。审计崩溃重试或 `rerun` 时，`evidence_stamp` 不重取，沿用这组审计尝试里**第一次**审计
`session.started`（I 之后的第一条审计启动）里的值，所以崩溃那次已写下的证据算本轮的证据。

### 5. 简报段「本轮要审的集成改动」

`brief::Inputs` 增加 `integration: Option<String>`（与 `gate` 同样由调用方渲染好）。`None` 时 `build` 输出与今天逐字
相同。`Some` 时插在 `## 本里程碑` 段之后、`## 上一轮对手轮的结论` 之前。渲染函数 `brief::integration_section(event,
doc_dir) -> String` 是纯函数，输出形如：

```markdown
## 本轮要审的集成改动

本轮是**集成复验**：实现 #9 开始时里程碑表没有 `开放` / `待审`，却在 `docs/x/` 之外留下了提交。规则见
`docs/x/protocol/loop-protocol.md` 的「集成复验」一节。

触发原因：rebase 冲突。冲突文件：
- crates/automed/src/launcher.rs

提交范围：`3f2a1c0..9e81b77`（默认分支基线 `55d0e12`），rebase 前后补丁不变的提交不列出。共 2 个：
- `a1b2c3d` fix: resolve launcher conflict
- `e4f5a6b` test: keep old template compatible
rebase 前有、rebase 后找不到同一补丁的提交 1 个：
- `0c9d8e7` feat: launcher change

改动统计（`docs/x/` 之外）：
（每个提交一块 git show --stat 输出，放在代码围栏里）

改到的测试文件（逐个看 diff，留意测试前提有没有被改弱）：
- crates/automed/tests/end_to_end.rs

本轮证据必须写入 `docs/x/evidence/M-08-r9-audit.md`。结论二选一：通过（里程碑表不动）；或把受影响的里程碑退回
`开放`、`reopen` 加 1。两种都要写这个证据文件。
```

格式规则（钉住现有 e2e，§「背景与现状」最后一条）：

- 所有 SHA 显示为**完整 SHA 的前 7 个字符**（直接切片，不用 `--short`，避免 Git 为唯一性加长到 ≥10 位）。
- 门禁失败只写「合并前门禁失败。命令：`…`。输出末尾：」+ 围栏里的尾部原文；**不写失败提交的 SHA，不写「未通过」
  字样**——那是上一轮门禁的事实，`## 宿主门禁` 段管本轮的门禁结论，两段不能互相冒充。
- `{"kind": "none"}` 写「没有注入：手动重跑或续跑」；`decisions` 写「用户表态」。
- 工作区项目每个仓库一块，提交与路径前加 `<仓库名>/`；单仓库不加。
- 段内只有内核从账本与 Git 算出的事实，不引用实现轮证据（约束 9）。

### 6. prompt 分支（`protocol/prompts/audit.md`）

在「先读 `{doc_dir}/{slug}-task.md` 和 `{doc_dir}/{slug}.md`，找出状态为 `待审` 的里程碑。」一段与其后的空行之后，插入
一个由独占行 `<!-- integration-audit -->` 与 `<!-- /integration-audit -->` 包住的块。块内第一行是空行之后的正文，
最后一行正文之后有一个空行，所以：

- **去掉模式**（非集成复验）：删除两行标记与其间全部内容 → 与今天的模板逐字相同 → 现有金样
  `golden/prompts/audit.md`、`audit-before-merge.md` 不动。
- **保留模式**（集成复验）：只删除两行标记。

块内正文（实现轮可以润色措辞，但以下要点必须都在，金样钉住）：本轮是集成复验、「找出 `待审`」与「结论三选一 / 没有
第四种结论」本轮不适用；逐项复验简报列出的提交：① 改动是否只为解决触发原因，② 有没有削弱既有断言或改测试前提（单列
的测试文件逐个看 diff），③ 对每个提交执行 `git grep -nE '^(<<<<<<<|>>>>>>>) ' <提交> -- . ':(exclude){doc_dir}/'`，
结果必须为空，④ 受影响里程碑 `### M-xx 验收契约` 的验收命令仍然通过；结论二选一（通过：表不动；发现问题：退回 `开放`、
`reopen` 加 1、按协议填领域与收敛模式），两者都写简报指定的证据文件并在 `retro.md` 追加一行；写入边界不变，发现问题
只能退回，不当场修。

取舍函数 `launcher::select_integration_audit(template, keep: bool) -> Result<String>`：与 `select_gate_run` 同样的
独占行规则；不配对、嵌套、非独占行的标记报错（模板错误不应静默渲染给模型）。`PromptSpec` 增加
`integration_audit: bool`，`role_prompt` 在 `select_prompt` 之前调用它（只对审计模板有标记；其他模板没有标记时原样
返回）。评估工具：`protocol/case.rs` 接受可选字段 `integration_audit: true`（默认 false），`eval_run.rs` 在调
`render_prompt` 之前按它取舍——`render_prompt` 签名不变，现有 eval 测试不动。

### 7. 守卫 A：集成复验未推进（约束 6、验收 3）

`guards::Round` 增加 `integration: Option<&'a IntegrationAudit>`（`IntegrationAudit { impl_round, evidence, summary, evidence_touched }`，
来自审计 `session.started` 的 `integration`）。`check` 对审计轮：

- `integration` 为 `None`：与今天完全相同（免查例外与 `audit_moved_the_table` 都保留）。
- `integration` 为 `Some`：用一个函数 `integration_audit_concluded` **取代**免查例外、`evidence_file("audit")` 与
  `audit_moved_the_table`，只产出至多一条 Error：
  - 有证据 = **简报指定的那个路径**（事件里的 `evidence`，即 `M-<表最后一行 ID>-r<impl_round>-audit.md`）在审计会话
    期间被写入过：`apply_guards` 对该路径取当前状态，与 `integration.evidence_stamp`（审计启动时的状态，§4）比较——启动时
    不存在而现在存在，或 `mtime_ns` / `len` 任一变了，`evidence_touched = true`，写进 `IntegrationAudit`。k 取事件里的
    `impl_round` 而不是审计可改的状态块。**文件名里的 k 只决定路径，不再等于「本轮新增」（D1-P02）**：实现轮正常递增 k
    并提前提交同名 `-audit.md`，审计什么都不写时，文件存在但状态没变，不算证据；
  - 有证据 → 通过（表不动是合法的「通过」，表被退回也合法）；
  - 无证据且表与轮前快照相同 → `audit_made_no_progress`，detail 用中文写：本轮是集成复验轮、`summary`（触发原因与
    提交范围）、应写的证据路径、两条出路（逐项复验后写证据、表不动；或退回受影响里程碑 `开放`、`reopen` 加 1 并写
    证据）；
  - 无证据但表动了（退回了里程碑）→ `evidence_missing`，detail 写出确切路径。
- `retro_one_line`、`audit_stayed_in_its_column` 照旧。

「本轮新增」以审计会话期间该路径被写入为准（`printf >` 重写同样内容也会更新 `mtime_ns`），不比较文件内容——现有 e2e
的替身审计每步都重写 `M-01-r$k-audit.md`，所以不改就通过；而「实现轮预写同 k 审计文件、审计不写」被拒绝。
纯函数 `guards::check` 不碰文件系统，`evidence_touched` 由 `apply_guards` 传入。残余风险见 R2。

### 8. 守卫 B：已关闭里程碑的「最新证据」指针（约束 7、验收 4）

`guards::Round` 增加 `design_before: Option<&'a str>`、`design_after: Option<&'a str>`。实现轮多跑一项
`no_retargeting_closed_evidence`，code 沿用 `impl_closed_milestone`（同一精神：实现轮替审计作了关闭里程碑的结论）。

- **指针行**：不在代码围栏（```` ``` ```` / `~~~`）内、去掉前导 `- ` / `* ` 后以 `最新证据` 开头的行。
- **归属**：该行之前最近一个 Markdown 标题（`##` 及更深）里出现的第一个 `M-\d+`；之前没有这样的标题 → 不归属、不检查。
- **基线**：上一条 `task.head` 的 `sha` 处的设计文档（`git show <sha>:<design_doc>`，跨 rebase 也取得到对象）；拿不到
  → `design_before = None` → 不检查（不猜，与 `design_changed_outside_milestones` 一致）。
- **判定**：对 `before` 快照里状态为 `已完成` 的每个里程碑，取 after 文档里归属它的指针行中**不在** before 文档同一
  里程碑指针行集合里的那些；其中任一行含 `-impl.md` → Error。
- **不误伤**：推进中的里程碑（轮前 `开放`）写「最新证据：…-impl.md」不在检查范围；改指向别的 `-audit.md` 不报；围栏
  里的样例不算。
- detail：「实现轮把已完成里程碑 M-08 验收契约的「最新证据」改指向了实现证据 `…-impl.md`。已完成里程碑的最新证据是
  审计复验留下的结论，实现证据替代不了它。本轮的证据写进本轮自己的 `M-xx-r<k>-impl.md` 即可；集成改动由紧随其后的
  集成复验审计。Autome 不会替你改回去。」

所有需求 Loop 的实现轮都跑这项（不止集成轮）；它不受 §4 的版本门约束，因为它不依赖任何 prompt 文字——v6 任务的实现轮
本来也不该这样做。

### 9. 回路逐节点（验收 1、2，约束 8）

**验收 1：全部关闭后 rebase 冲突。** `Audit`（关闭最后一个里程碑）→ `Retro` → `Rebase`（冲突，`rebase.conflicts`）→
`Implement`（带 `RebaseConflict`；会话自己 rebase、解决冲突、提交；兜底；守卫；写 `integration.range`）→ 门禁
`before_merge` 不跑 `Gate` → `Audit`（集成复验：简报段 + prompt 分支）→ 写证据、表不动 → 守卫 A 通过 →
`all_milestones_done` → `after_the_loop` → `Retro` → `Rebase`（分支已在默认分支之上，干净）→ 配了门禁则 `MergeGate`
→ `AwaitMerge`。

**验收 2：合并前门禁失败后退回。** `MergeGate`（失败）→ `Implement`（带 `GateFailed`，修复、提交，k+1）→ `Audit`
（集成复验）→ 把 M-xx 改为 `开放`、`reopen` 加 1、写证据 → 守卫 A 通过（表动了且有证据）；`apply_guards` 照常写
`milestone.contradicted` → 表非全 `已完成`、k < N → `start_impl` → `Implement`（轮前表有 `开放`，§1 (a) 不成立，普通
实现轮）→ `待审` → `Audit`（有 `待审`，今天的流程）→ `已完成` → `Retro` → `Rebase` → `MergeGate` → `AwaitMerge`。
k ≥ N 时审计结束走 `Failed(ImplBudget)`，与今天相同。

**为什么不会无限循环。** 集成复验只由「改了目录外文件的实现轮」触发；审计轮受 `guard_session_scope` 约束改不了目录外
文件，通过之后不产生新的触发。之后再回到实现轮只有三种来源：新的 rebase 冲突（同一文件集连续 3 次 → 失败，现有）、
新的门禁失败（`same_failures` 熔断，现有）、审计退回（每次消耗一个实现轮，受 N 约束，现有）。本设计不新增任何回边。

### 10. 协议、eval 与文档（约束 11）

- `protocol/prompts/audit.md`：§6 的块。
- `protocol/loop-protocol.md`：在 `## 实现循环` 与 `## Backlog 与争议项` 之间新增**二级标题** `## 集成复验`（什么时候
  发生、简报段有什么、四项复验、二选一结论、证据命名、与「没有第四种结论」的关系）。放在 `## 实现循环` 之外是刻意的：
  `brief-map.toml` 给审计摘的是 `实现循环`，写进 `### 审计轮` 会改变所有普通审计简报（约束 1）。「Autome 会检查什么」
  表加两行（集成复验轮留下证据或退回里程碑｜停下等人；实现轮没有把已完成里程碑的「最新证据」改指向 `-impl.md`｜停下
  等人），表前的「五条…前两条」改成「七条…前四条」。`brief-map.toml` 不改（集成复验的规则由简报段里的指针引过去）。
- `protocol/CHANGELOG.md`：末尾新增 `## protocol/v7`，两条：
  - `J-1` `kind: behavioral`，`clause: prompts/audit.md`，`evidence: [T-61 L-05（docs/门禁只在合并前跑一次-gate-run/lessons.md）, T-61 L-06 实现 #9/#10 之后的空转审计, T-68 任务卡]`，
    `predicted_impact: {metric: impl_defects, direction: up, scope: task, horizon: 3}`，`eval: evals/audit-integration-review/`。
  - `J-2` `kind: clarify`，`clause: loop-protocol.md`，同样的证据，`predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}`，`eval: null`。
- `protocol/evals/audit-integration-review/`：`case.yaml`（`role: audit`、`integration_audit: true`）、`scaffold.sh`
  （复用 `auditor-rerun` 的骨架：表全 `已完成`，最后一个提交把某个测试的 `assert_eq!` 改成更弱的 `assert!`，并写好
  `docs/eval-case/brief/audit-1.md` 的集成段）、`graders/`（审计必须：退回该里程碑或写出带复验内容的
  `M-xx-r<k>-audit.md`；不得以「无待审」结束；不得改文档目录外文件）。改前（无块）红、改后绿；`--behaviour` 要花钱，
  不在会话里跑。
- `docs/development/technical-design.md`：§6 补一段「集成复验的提交范围」（§2 的公式与理由）；§8 补一段「集成复验」
  （判定、事件、选择、两条守卫）。`README.md` 用 `grep -n "无待审\|待审里程碑" README.md` 核对，有矛盾就地改那一句，
  没有就不动。

### 11. 需要改动的既有测试（约束 1：只有结构体加字段导致的编译修补）

| 位置 | 改动 | 原因 |
|---|---|---|
| `guards.rs` 测试 helper `round()` | 加 `integration: None, design_before: None, design_after: None` | `Round` 加字段 |
| `guards.rs` `the_first_round_has_nothing_to_compare_against_and_is_not_penalised` 的字面量 | 同上 | 同上 |
| `brief.rs` 测试里的 `crate::guards::Round { … }` | 同上 | 同上 |
| `brief.rs` 测试 helper 的 `Inputs { … }` | 加 `integration: None` | `Inputs` 加字段 |
| `tests/prompt_rendering.rs` 两处 `Inputs { … }` | 加 `integration: None` | 同上 |
| `tests/prompt_rendering.rs` `spec()` 与 `launcher.rs` 测试 helper 的 `PromptSpec { … }` | 加 `integration_audit: false` | `PromptSpec` 加字段 |

其余既有测试（含 §「背景与现状」点名的两个 e2e）一行不改。断言、前提、替身脚本都不动；若实现期发现别的既有测试
因新行为失败，那是设计或实现的缺陷，按缺陷处理，不改测试。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 待审 | 识别里程碑之外的实现轮并记账：start_heads、提交范围口径、测试文件判定、integration.range 事件 | 0 | |
| M-02 | 开放 | 两条守卫与接线：集成复验未推进、已关闭里程碑的最新证据指针、审计启动时的集成判定与版本门 | 0 | |
| M-03 | 开放 | 集成复验的简报段与 audit.md 分支：取舍函数、评估工具、新金样、protocol/v7 J-1 与 eval 用例 | 0 | |
| M-04 | 开放 | 端到端：rebase 冲突到合并卡点、门禁失败后退回、未推进停下、无目录外提交时零变化 | 0 | |
| M-05 | 开放 | 协议正文与技术设计：loop-protocol 的「集成复验」与检查表、CHANGELOG J-2、technical-design §6/§8 | 0 | |

## 测试与验收

依赖顺序：M-01 → M-02 → M-03 → M-04 → M-05。M-01 先做，因为最大的技术不确定性是 rebase 之后的范围口径与 H0 的取法；
它在临时仓库的单元测试里就能钉住。每个实现轮还要在会话里跑 `cargo fmt --all -- --check` 与
`cargo clippy --all-targets -- -D warnings` 并把命令与退出码写进证据（验收 7、任务 §3 补充）；它们不区分里程碑对错，
不写进验收契约。全量门禁由宿主在合并前运行。

| 验收（任务 §3） | 由谁覆盖 |
|---|---|
| 1 rebase 冲突 → 集成复验 → 合并卡点 | M-04 `integration_e2e_rebase_conflict_after_all_done_is_reviewed_then_reaches_await_merge`；M-01 范围单测 |
| 2 门禁失败 → 集成复验退回 → 实现轮、reopen | M-04 `integration_e2e_merge_gate_fix_reopened_by_the_audit_returns_to_implement` |
| 3 不写证据只写无待审 → 未推进 | M-02 守卫单测；M-04 `integration_e2e_audit_without_evidence_stops_with_a_readable_reason` |
| 4 改已关闭里程碑最新证据 → 守卫失败 | M-02 `closed_pointer_*` 单测 |
| 5 无目录外提交 → 行为与今天相同 | M-01 `integration_range_doc_only_commits_are_not_integration`；M-04 `integration_e2e_doc_only_repair_keeps_todays_audit` 与回归子集 |
| 6 金样更新、现有金样与 e2e 不改就通过 | M-03 新金样与「既有金样未改」检查；M-04 回归子集与「e2e 只增不改」检查 |
| 7 fmt / clippy / ci-local | 各实现轮记录 fmt、clippy 退出码；`ci-local.sh` 由合并前门禁 |

### M-01 验收契约

最新证据：docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-01-r1-impl.md · 17 用例通过，待独立审计 · 实现 #1

- 验收命令：`cargo test -p automed --lib integration_range`
- 预期：17 个用例通过，基线 0 个（本轮之前没有名字含 `integration_range` 的用例）。逐个：`integration_range_linear_repair_lists_the_fix_commit`、`integration_range_rebase_lists_only_rewritten_and_new_commits`（同时断言「消失」列表）、`integration_range_clean_rebase_is_empty`、`integration_range_doc_only_commits_are_not_integration`、`integration_range_counts_the_sweep_commit_and_skips_the_brief_commit`、`integration_range_a_crashed_attempt_keeps_its_commits`、`integration_range_test_files_by_path_and_by_content`、`integration_range_workspace_member_paths_carry_the_repo_name`、`integration_range_is_recorded_only_for_an_all_done_table_that_stays_done`（轮前有 `待审`、轮后新增 `开放` 两种都不记）、`integration_range_records_the_trigger_from_the_carried_inject`（rebase / 门禁 / 无注入三种）、`integration_range_bug_loop_records_nothing`；另 5 个（评审 R1 的反例）：`integration_range_conflict_that_drops_the_product_commit_is_still_integration`（冲突中选上游、自己的产品提交变空被丢弃：事件写出，「消失」带该提交的路径与 stat）、`integration_range_conflict_file_commit_with_an_equivalent_patch_id_is_still_listed`（`"hello world"` 解成 `"helloworld"`，patch-id 相同仍被列出）、`integration_range_inline_test_module_with_only_setup_changed_is_listed`（只改内联测试模块里的测试准备行，文件进测试清单）、`integration_range_unreadable_range_fails_closed`（范围读取失败不写事件也不放行）、`integration_range_a_stat_failure_keeps_the_range`（stat 读取失败：事件仍写、字段带「读取失败」）。
- 另含调度器回归 `integration_range_read_failure_stops_task_before_audit`：真实启动记录 start_heads，范围读取失败后明确进入 Failed(Config)，不写 integration.range；补足只测范围函数不足以证明收尾状态迁移的缺口。
- 负向对照：去掉 `--cherry-pick` 时 `…rebase_lists_only_rewritten_and_new_commits` 与 `…clean_rebase_is_empty` 失败（干净重放的提交被列出）；不按 `<doc_dir>/` 过滤时 `…doc_only_commits_are_not_integration` 失败；H0 取重试那次会话的 `start_heads` 而不是本轮第一次尝试时 `…a_crashed_attempt_keeps_its_commits` 失败；(d) 只看「列出」时 `…conflict_that_drops_the_product_commit_is_still_integration` 失败；去掉冲突文件保守纳入时 `…equivalent_patch_id_is_still_listed` 失败；测试文件只看路径与增删行时 `…inline_test_module_with_only_setup_changed_is_listed` 失败；展示信息读取失败时不写事件则 `…a_stat_failure_keeps_the_range` 失败。

### M-02 验收契约

- 验收命令：`cargo test -p automed --lib guards && cargo test -p automed --lib integration_wiring`
- 预期：`guards` 34 个用例通过，比基线 24 个多 10 个（`integration_audit_with_a_prewritten_evidence_file_and_no_new_write_made_no_progress`、`integration_audit_with_a_still_table_and_no_evidence_made_no_progress`、`integration_audit_with_evidence_and_a_still_table_passes`、`integration_audit_that_reopens_without_evidence_is_missing_evidence`、`integration_audit_that_reopens_with_evidence_passes`、`integration_audit_evidence_of_an_earlier_round_does_not_count`、`closed_pointer_retargeted_to_impl_evidence_fails`、`closed_pointer_added_under_a_closed_milestone_fails`、`closed_pointer_rules_leave_open_milestones_and_fences_alone`、`closed_pointer_without_a_baseline_is_not_guessed`）；既有 24 个（含 `rebase_repair_with_all_milestones_done_does_not_require_empty_audit_evidence`）断言不变全部通过。`integration_wiring` 5 个通过，基线 0（`integration_wiring_evidence_stamp_is_taken_at_the_first_audit_attempt_and_detects_a_rewrite`、`integration_wiring_audit_after_a_recorded_range_is_marked`、`integration_wiring_a_later_impl_session_supersedes_the_range`、`integration_wiring_a_pinned_template_without_the_block_keeps_todays_audit`、`integration_wiring_impl_guard_reads_the_design_at_the_last_task_head`）。
- 负向对照：集成复验时仍保留「全 `已完成` 且表未变即免查」的例外，`integration_audit_with_a_still_table_and_no_evidence_made_no_progress` 失败（守卫放行）；指针检查不限于轮前 `已完成` 的里程碑时 `closed_pointer_rules_leave_open_milestones_and_fences_alone` 失败；判定集成时不比对最近实现会话 id 时 `integration_wiring_a_later_impl_session_supersedes_the_range` 失败；证据只看文件名存在时 `integration_audit_with_a_prewritten_evidence_file_and_no_new_write_made_no_progress` 失败。

### M-03 验收契约

- 验收命令：`cargo test -p automed --test prompt_rendering && cargo test -p automed --lib integration_section && cargo run -p automed -- protocol eval protocol/ && git diff --quiet --diff-filter=MD "$(git merge-base HEAD main)" -- crates/automed/tests/golden/`
- 预期：`prompt_rendering` 35 个通过，比基线 31 个多 4 个（`integration_audit_prompt_matches_its_golden`、`integration_audit_prompt_before_merge_matches_its_golden`、`integration_audit_brief_after_a_rebase_conflict_matches_its_golden`、`integration_audit_brief_after_a_gate_failure_matches_its_golden`，对应新增金样 `golden/prompts/audit-integration.md`、`audit-integration-before-merge.md`、`golden/briefs/audit-integration-rebase.md`、`audit-integration-gate.md`）；`integration_section` 6 个通过，基线 0（简报 3：无段时逐字不变、段内容齐全且 SHA 为 7 位且不含失败提交 SHA 与「未通过」、证据路径确切；launcher 2：标记去掉 / 保留、不配对标记报错；eval_run 1：`integration_audit` 字段决定取舍）；静态门退出码 0；最后一条退出码 0（`tests/golden/` 下没有被修改或删除的既有文件，只有新增）。
- 负向对照：去掉模式漏删块后的空行时 `before_merge_role_prompts_match_their_goldens` 与 `every_role_prompt_renders_exactly_what_the_constants_did` 在 `audit` 金样上失败；`integration` 为 `None` 也输出段标题时 `before_merge_audit_brief_matches_its_golden` 失败；SHA 用 `--short` 渲染时简报那条 `integration_section` 用例失败。

### M-04 验收契约

- 验收命令：`cargo test -p automed --test end_to_end integration_e2e && cargo test -p automed --test end_to_end before_merge && cargo test -p automed --test end_to_end a_gate_failing_after_the_automatic_rebase && cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back && ! git diff -U0 "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs | grep -q '^-[^-]'`
- 预期：`integration_e2e` 4 个通过，基线 0（`integration_e2e_rebase_conflict_after_all_done_is_reviewed_then_reaches_await_merge`：审计 prompt 含集成块、简报含「本轮要审的集成改动」与冲突文件名和改到的测试文件，节点序列 `implement, audit, retro, rebase, await_merge`；`integration_e2e_merge_gate_fix_reopened_by_the_audit_returns_to_implement`：简报含门禁命令与尾部，审计退回后下一节点 `implement`、状态块 reopen 1、有 `milestone.contradicted`，最终到 `await_merge`；`integration_e2e_audit_without_evidence_stops_with_a_readable_reason`：实现步骤用 `World::EVIDENCE`（预写同 k 的 `M-01-r$k-audit.md`，正是评审的绕过场景），审计步骤什么都不写；任务 `failed`，`guard.failed` 的 code 为 `audit_made_no_progress`，detail 含「集成复验」、提交范围与「开放」；`integration_e2e_doc_only_repair_keeps_todays_audit`：里程碑全 `已完成` 时的修复轮只提交文档目录内的文件，没有 `integration.range` 事件，其后审计的 prompt 不含集成块、简报不含该段，审计不写证据也照常继续）；`before_merge` 11 个与另两个各 1 个既有用例全部通过，数量不变；最后一条退出码 0（`end_to_end.rs` 只有新增行，没有删除或改动的既有行）。
- 负向对照：`apply_guards` 不把审计 `session.started` 的 `integration` 传进 `Round` 时 `integration_e2e_audit_without_evidence_stops_with_a_readable_reason` 失败（任务继续到 `retro`）；简报段把门禁失败提交 SHA 写成 10 位以上时既有的 `brief_before_merge_audit_brief_never_shows_a_stale_result` 失败；判定不要求轮后表仍全 `已完成` 时不影响这四个用例，但 M-01 的 `…all_done_table_that_stays_done` 会红（由 M-01 契约拦）。

### M-05 验收契约

- 验收命令：`cargo run -p automed -- protocol eval protocol/ && cargo test -p automed --test prompt_rendering && test "$(grep -c '^## protocol/v' protocol/CHANGELOG.md)" = 7 && grep -q '^## 集成复验$' protocol/loop-protocol.md && grep -q '集成复验' docs/development/technical-design.md`
- 预期：静态门退出码 0；`prompt_rendering` 35 个通过，与 M-03 结束时相同（证明新增的协议正文没有流进普通审计、实现简报的金样）；`## protocol/v1..v7` 恰 7 个标题；两条 grep 命中。
- 负向对照：把「集成复验」写成 `### 审计轮` 下的小节时 `before_merge_audit_brief_matches_its_golden` 与 `gate_quick_audit_brief_matches_golden` 失败（简报摘到了新文字）；rebase 丢掉任一版本标题时标题计数不等于 7；CHANGELOG 条目的 `metric` 写成不在 `TaskMetrics::METRIC_NAMES` 里的名字时静态门失败。

## Backlog

- B-01 内核自己对集成范围内每个提交执行冲突标记扫描，把结果写进简报或作为守卫（本任务只要求审计轮执行；L-06 的检查可以机械化）。
- B-02 审计角色关闭的项目里，里程碑之外的实现轮仍然没人审；需要时另立任务决定是强制插入一轮审计还是只告警。
- B-03 集成复验证据的「本轮新增」靠 mtime / 长度变化认定（R2）；若出现审计只 `touch` 证据文件而不写内容的情形，可再比对内容或要求证据含复验提交的 SHA。
- B-04 （评审 R1-N01，已在 §4 澄清）实现 `integration_for_audit` 时用「最近实现会话 I 再向前取 `task.head`」定位 S，并用「审计收尾已写新 `task.head`」作测试前提。
- B-05 （评审 R1-N02）守卫失败的实现尝试已修产品、重跑只修文档时，旧 `integration.range` 会因新实现会话失效，是否把守卫失败的尝试与重跑累计为同一审计单元，待用户另行决定；本任务不扩大触发范围。
- B-06 rebase 由会话自行完成、触发原因不是 rebase 冲突时冲突文件未知，§2 只按 patch-id 列出；可考虑让内核在会话前后各取一次文件级三方合并结果。

## 风险与未决问题

- **R1 patch-id 对上下文敏感。** 干净 rebase 时，若默认分支改了某个分支提交的上下文行，该提交 patch-id 会变而被「列出」。
  结果是多列不少列，与约束 9「宁可多列」一致；简报写明「rebase 前后补丁不变的提交不列出」，审计能看出哪些只是被重放。
- **R2 「本轮证据」按路径状态变化认定。** 路径固定为简报指定的 `M-xx-r<k>-audit.md`；是否「本轮新增」比较审计启动时与
  收尾时该路径的存在 / `mtime_ns` / 长度。实现轮没递增 k 或预写同名文件，都不会让审计不写证据而通过；残余是审计只 `touch`
  文件（B-03）。选择它而不是比较内容，是因为现有 e2e 的替身审计每步重写同样内容的同名文件，改测试不是允许的做法。
- **R3 运行中的旧版本任务。** 守卫 A、简报段、prompt 分支都受 §4 版本门约束，v6 任务逐字不变；守卫 B 对所有需求任务
  生效（它只拦一种本来就不该发生的改动）。合并后正在跑的任务若真被守卫 B 拦下，detail 写明原因与做法，用户 `rerun`。
- **R4 并行任务改同一批文件。** 本仓库 `parallel = 3`，其他任务也改 `scheduler.rs`、`guards.rs`、`protocol/`。新逻辑放进
  新函数（`integration_*`、`no_retargeting_closed_evidence`、`select_integration_audit`），对既有函数只做插入调用，减少
  冲突面；本任务被 rebase 时按约束 14 在证据里记录逐提交冲突标记扫描结果，`CHANGELOG.md` 冲突时两侧版本标题都保留，
  若 main 先发布了 v7，本任务顺延为 v8 并相应修改 M-05 契约里的标题计数（这是事实变化，不是放宽验收）。
- **R5 事件体积。** 提交数与 stat 行数有上限（§3）；门禁尾部沿用注入里已有的尾部，不另取。
- **R6 范围算不出来。** 判定所需事实读取失败时失败关闭（§3 ①），不当作空轮也不放行；已确认集成轮后只有展示信息缺失时，
  事件与简报段照常（§3 ②）。
- 未决问题：无。需要真人确认的验收项：无（全部验收由测试命令覆盖），故不设 `## 人工验收清单`。
