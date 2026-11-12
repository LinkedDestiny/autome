# Bug Loop 三个步骤的 prompt 列出任务附件与 `--doc` 参考文档

status: 实现中
design-round: 1/15
implementation-round: 4/10
current-milestone: M-03
current-milestone-reopens: 0
convergence-mode: normal
next-action: 独立复验 M-03 的 rebase 集成、双门禁模式与输入渲染

任务文件：`docs/bug-loop-的定位-修复-复核/bug-loop-的定位-修复-复核-task.md`（验收标准见它的第 5 节 A1–A8，硬性约束见第 6 节 C1–C9，待定问题见第 8 节 Q1–Q6）。
任务卡正文：`docs/bug-loop-的定位-修复-复核/bug-loop-的定位-修复-复核-card.md`。

## 背景与现状

以下都在基线 `83c80e9` 上核对过；行号是基线上的行号。

1. **数据已经在 spec 里，只是没渲染。** `scheduler.rs` 组装 `PromptSpec`（约 2583–2599 行）时，不论会话类型都传
   `attachments: &task.attachments`、`doc_refs: &task.doc_refs`。`bug_prompt`（`launcher.rs` 492–540 行）只替换
   `{doc_dir}`、`{slug}`、`{budget_line}`，再以 `{request}` 切段拼接，不读这两个字段。三个模板
   `protocol/prompts/{locate,fix,fix-review}.md` 也没有 `{inputs}`。
2. **整理轮的渲染拼出的附件路径是错的（Q1 实测结论）。** `intake_prompt`（678–731 行）把每个附件渲染成
   `- {doc_dir}/attachments/{a}`，`a` 是 `task.attachments` 里的原值。两个真实入口存进去的都是**源文件的绝对路径**：
   - 命令行 `autome task new … --attach <f>`：`crates/autome-cli/src/main.rs` 410–418 行对每个 `--attach` 做
     `std::fs::canonicalize`，传给 `task.create` 的是绝对路径；
   - Web 网关 `apps/web/server/gateway.ts` 44–54 行把上传 id 解析成 `<home>/web/uploads/…` 下的绝对路径再转发；
   - `dispatch.rs` 1395–1412 行只校验 `is_file()`，原样存下。

   而 `write_task_inputs`（`scheduler.rs` 2878–2897 行）复制附件时只取 `Path::file_name()`，副本落在
   `<docs_root>/<doc_dir>/attachments/<文件名>`。于是一个真实附件 `/Users/x/crash.log` 在整理轮 prompt 里是
   `docs/<slug>/attachments//Users/x/crash.log`——这个路径不存在。现有单测
   `the_intake_prompt_lists_attachments_and_doc_refs_when_present`（1816 行）用裸文件名 `promo.csv`，替身和调用方
   对齐了，没有和真实入口对齐，所以一直是绿的。**该缺陷本任务不修**：整理轮渲染保持基线（方案 1），只有 Bug 侧用对的路径。
3. **参考文档原样渲染。** `doc_refs` 是用户在 `--doc` 里写的仓库内路径，`dispatch.rs` 不校验、原样存，整理轮逐行
   `- {d}`。Web 网关会删掉 `doc_refs`（gateway.ts 55 行），所以只有命令行会带进来。
4. **用户文本里的花括号。** `check_no_placeholders_left` 只认 `{lower_snake}` 形式。`bug_prompt` 先按 `{request}`
   切段、逐段检查、再把需求拼回去，所以需求里的 `{…}` 不会被误判。`doc_refs` 与附件文件名同样是用户给的文本，
   而整理轮是**先**把 `inputs` 替换进去**再**检查——参考文档写成 `docs/{x}.md` 时整理轮会拒绝启动。
5. **静态门与 eval 已经认识 `{inputs}`（Q6）。** `protocol/eval.rs` 的 `KNOWN_PLACEHOLDERS`（390 行）已含 `inputs`；
   `eval_run.rs::render_prompt`（293–311 行）对任何模板都把 `{inputs}` 替换成空串，Bug 的 eval 用例照样能渲染。
   两处生产逻辑都不用改。eval.rs 的测试夹具固定追加 v3，与真实新版本重名；夹具及对应查询改用测试专用 v999，保留断言。
6. **版本号。** `protocol/CHANGELOG.md` 最新一节是 `## protocol/v2`。`protocol/mod.rs` 的单测
   `the_version_in_force_is_the_newest_changelog_heading_over_the_seed`（401 行）断言 `protocol/v2`；
   `version_page.rs` 与 `autome-domain/src/changelog.rs` 里出现的 `protocol/v2` 用的是测试自带的样例文档，与真实
   CHANGELOG 无关，不用改。
7. **工作区项目（Q4）。** `PromptSpec.doc_dir` 已经是 `layout.doc_dir_seen_from(&session_cwd, …)`，附件副本写在
   `layout.docs_root` 下的同一个任务目录（`scheduler.rs` 2440 行），所以 `{doc_dir}/attachments/<文件名>` 在工作区项目里
   同样指得到文件。`--doc` 是用户原样给的路径，工作区会话从工作区目录启动，仓库内相对路径未必能直接打开——这在整理轮
   今天就是如此，本任务沿用（见 Backlog B-03）。
8. **基线测试数**（2026-11-12 实测，原始输出在 `.autome/output/t41-plan-*.txt`）：
   `cargo test -p automed --lib launcher` 57 个；`cargo test -p automed --test prompt_rendering` 16 个；
   `cargo test -p automed --test end_to_end bug_loop` 14 个；`cargo test -p automed --lib protocol::` 61 个；
   `cargo run -p automed -- protocol eval protocol/` 8 项通过、0 项不通过、1 项警告，CHANGELOG 条目 5 条。

## 目标与非目标

**目标**

- G1（A1）有附件或参考文档时，定位、修复、复核三个会话的 prompt 都列出附件在任务目录里的路径
  `{doc_dir}/attachments/<文件名>` 与参考文档路径。
- G1b 需求 Loop 的渲染结果对任意输入都与基线逐字节相同（含带绝对路径附件的整理轮）。
- G2（A2）两者都没有时，三个 prompt 与基线逐字节相同：没有空段落，没有 `{inputs}` 字面量。
- G3（A3–A5）launcher 单测覆盖有 / 无附件；新增三份带输入的 Bug 金样，已有 12 份金样不变；e2e 以真实入口的取值
  （绝对路径）创建带附件与 `--doc` 的 Bug 任务，断言三个会话的 prompt 都列出两类路径。
- G4（A6）协议升到 `protocol/v3`，CHANGELOG 记三条 `clarify`，静态门通过。
- G5（A7、A8）`cargo test --workspace`、`sh scripts/ci-local.sh`、`sh plugins/autome/tests/run.sh evals` 通过；
  `plugins/` 无改动。

**非目标**

- 不改需求 Loop 的任何模板文本，不改它们的金样（`intake`、`intake-card`、`plan`、`review`、`adjudicate`、`impl`、
  `audit`、`onboarding` 及 `retro.md` 的模板）。
- 不改附件上传与存储：`task.create` 的参数校验、账本里 `attachments` / `doc_refs` 的存法、`write_task_inputs`
  的复制与提交。
- 不改 Bug Loop 的状态机、转换、预算、守卫；不改 Web 面板、桌面壳、插件；不改 eval 框架（`eval.rs`、`eval_run.rs`）。
- 不处理整理轮非任务卡路径「先替换 `{request}` 再替换 `{inputs}`」的既有顺序（B-02）。
- 不修正整理轮对绝对路径附件拼出不存在路径的既有缺陷（现状第 2 条；B-05）——它属于需求 Loop 的渲染结果，任务第 4 节
  明令不改，留给另行授权的任务。

## 方案

### 1. 共用的输入渲染（Q1、Q2）

在 `launcher.rs` 里、`intake_prompt` 旁边新增私有函数：

```rust
fn render_inputs(doc_dir: &str, attachment_names: &[String], doc_refs: &[String]) -> String
```

- 函数只负责**拼装**：附件段标题「用户提供的附件（已复制到任务目录）：」，逐行
  `- {doc_dir}/attachments/<attachment_names 的元素>`；参考段标题「用户指定的仓库内参考文档：」，逐行
  `- <原样路径>`；每段后跟一个空行（段落以 `\n\n` 结尾）；两者都空时返回空串。输出格式与整理轮今天的内联代码一致。
- **附件显示名由调用方定**，函数本身不改写它：
  - `intake_prompt` 删掉内联拼接，改调 `render_inputs`，传 `spec.attachments` **原值**——渲染结果对任意输入都与基线
    逐字节相同（含绝对路径附件，这一点由单测 5 固定）。
  - `bug_prompt` 先把每个附件映射成副本文件名再传入：`Path::new(a).file_name()` 的 lossy 字符串，与
    `write_task_inputs` 的复制规则同源；取不到文件名时（只会是畸形路径，`write_task_inputs` 已先行报错）退回原值。
    映射写成 `bug_prompt` 旁的小函数，不放进 `render_inputs`。

**关于整理轮（Q1 的界定）。** 任务第 4 节要求需求 Loop 的 prompt「文本与渲染结果都不变」，任务卡也写明「不改需求 Loop
的 prompt」，C1 要求不扩大范围。现状第 2 条的缺陷（整理轮对绝对路径附件拼出 `…/attachments//绝对/路径/x.log`）真实存在，
但修它就会改变整理轮渲染结果，不在本任务授权之内。所以：

- 整理轮保持基线，缺陷记 Backlog B-05，留给另行授权的任务；
- 「复用整理轮的 `inputs` 渲染」落在共用的标题、列表、空段落拼装上，路径怎么写由两边的调用方各自决定；
- Bug 侧不照搬错误路径：任务卡要的是「附件相对路径列表」，且 A5 要求 prompt 里的路径指得到任务目录里的副本。

这样 12 份既有金样与既有整理轮单测不变，新增单测 5 再把「整理轮对绝对路径附件仍按存储原值渲染」固定下来，防止以后谁
顺手把两边改成一样而越界。

### 2. `bug_prompt` 填 `{inputs}`，用户文本不参与占位符检查（C7）

附件文件名与参考文档都是用户文本，可能含 `{…}`。沿用「先切再填」：

1. 模板先按 `{request}` 切段；
2. 每段再按 `{inputs}` 切片，每片替换 `{doc_dir}`、`{slug}`、`{budget_line}` 后各自过 `check_no_placeholders_left`；
3. 片之间用 `render_inputs(...)` 的结果拼回，段之间用 `spec.request` 拼回。

这样需求里的 `{inputs}`、参考文档里的 `{slug}` 都按字面保留，不会被替换，也不会让会话拒绝启动。定位轮在有任务卡时
前置的那段说明不变。

### 3. 三个模板放 `{inputs}` 的位置（Q2、Q3）

三个步骤都列同样的两段（Q3）：定位轮要据此找根因；修复轮要对照日志、截图确认修的是同一现象；复核轮要判断「根因是否
站得住」，同样需要原始材料。

`{inputs}` 放在一行的行首、紧贴该行原有文字，**不另起一行**，与 `intake.md` 的 `{inputs}请完成三件事：` 同一写法。
所选的行前面本来就有一个空行，所以：有输入时是「空行 + 段落 + 空行 + 原文」，没有输入时与基线逐字节相同。

| 模板 | 行号 | 改后该行开头 |
|---|---|---|
| `protocol/prompts/locate.md` | 7 | `{inputs}定位根因，提交一个能稳定失败的回归测试，…` |
| `protocol/prompts/fix.md` | 6 | `{inputs}以最小改动修复根因，…` |
| `protocol/prompts/fix-review.md` | 6 | `{inputs}独立复验复现命令与项目门禁，…` |

段落措辞沿用整理轮的两句标题，不另写一套：标题本身对三个 Bug 角色都成立，另写会让两条 Loop 的共用函数分叉。

### 4. 协议版本（Q5）

- `protocol/CHANGELOG.md` 末尾新增 `## protocol/v3`，一句说明 + 三个 yaml 块，一个模板一块：

  ```yaml
  - id: I-1
    kind: clarify
    clause: prompts/locate.md
    evidence: [web-dashboard-via-loop §3.1, T-41 任务卡]
    predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
    eval: null
    realized_impact: null
  ```

  `I-2`、`I-3` 同形，`clause` 分别为 `prompts/fix.md`、`prompts/fix-review.md`。
- **为什么是 `clarify` 而不是 `behavioral`**：参考文档 §3.1 规定附件与参考文档本来就是任务卡的一部分，`bug-protocol.md`
  与模板已经要求定位轮完整读任务卡；这次改动是把「任务卡的另外两部分在哪里」写进 prompt，不新增或改变会话应遵守的
  规则。`behavioral` 需要「改前红、改后绿」的 eval 用例，而 eval 框架（`eval_run.rs::render_prompt`）固定把 `{inputs}`
  渲染成空串，用例根本拿不到附件；要让它拿到就得改 eval 框架，超出 C1 的范围。这条取舍同样列进「人工验收清单」。
- `evidence` 的两个任务按 `evidence_tasks()` 取首个词：`web-dashboard-via-loop` 与 `T-41`，满足至少两个任务。
  `predicted_impact` 与 v2 的 B-1…B-4 一致（`protocol_failures` 持平），在 `METRIC_NAMES` 内。
- `protocol/mod.rs` 401 行的断言改为 `protocol/v3`。不改 `KNOWN_PLACEHOLDERS`、`eval_run.rs`（见现状第 5 条）。
- 只改仓库根 `protocol/`，不碰 `docs/*/protocol/` 下任何任务的副本（C5）；正在跑的任务（包括本任务，固定在 v2）
  不受影响。

### 5. 改动清单

| 文件 | 改什么 |
|---|---|
| `crates/automed/src/launcher.rs` | 新增 `render_inputs` 与 Bug 侧附件显示名小函数；`intake_prompt` 改调 `render_inputs`（传原值，渲染不变）；`bug_prompt` 按方案 2 填 `{inputs}`；新增 5 个单测 |
| `protocol/prompts/locate.md`、`fix.md`、`fix-review.md` | 各改一行，行首加 `{inputs}` |
| `protocol/CHANGELOG.md` | 末尾加 `## protocol/v3`，三条 `clarify` |
| `crates/automed/src/protocol/mod.rs` | 版本断言改 `protocol/v3` |
| `crates/automed/src/protocol/eval.rs` | 仅测试夹具与对应查询改用 v999，避免与真实 v3 重名；生产逻辑不变 |
| `crates/automed/tests/prompt_rendering.rs` | 新增 3 个带输入的金样用例；`bug_loop_bug_prompts_leave_no_placeholder` 的检查清单加 `{inputs}` |
| `crates/automed/tests/golden/prompts/` | 新增 `locate-inputs.md`、`fix-inputs.md`、`fix-review-inputs.md`；已有 12 份不动 |
| `crates/automed/tests/end_to_end.rs` | 新增 1 个 e2e；在既有 `bug_loop_a_bug_runs_from_creation_to_a_merge_commit` 里加「无附件段」断言 |

以上之外的文件不动。

## 测试与验证

### launcher 单测（M-01，新增 5 个，57 → 62）

替身取值与真实入口对齐（C9）：附件一律给**绝对路径**，至少一个带空格。

1. `bug_prompts_list_attachments_and_doc_refs_when_present`——三个步骤逐一渲染，附件
   `["/tmp/autome-src/crash.log", "/tmp/autome-src/screen shot.png"]`、参考 `["docs/notes.md"]`；断言含
   `- docs/checkout-flow/attachments/crash.log`、`- docs/checkout-flow/attachments/screen shot.png`、`- docs/notes.md`
   与两句标题，且不含 `/tmp/autome-src`。
2. `bug_prompts_have_no_inputs_paragraph_when_there_are_none`——三个步骤，附件与参考都空；断言不含两句标题、不含
   `{inputs}`、不含连续三个换行。
3. `bug_prompts_render_only_the_inputs_section_they_have`——只有附件时只有附件标题，只有参考时只有参考标题。
4. `bug_prompts_keep_braces_in_doc_refs_and_file_names_literally`——参考 `docs/{unknown}.md`、附件 `/tmp/a/{slug}.log`、
   需求含 `{inputs}`；`build_prompt` 返回 `Ok`，结果按字面含 `docs/{unknown}.md`、`attachments/{slug}.log` 与需求原文。
5. `the_intake_prompt_keeps_rendering_stored_attachment_values_verbatim`——整理轮附件给 `/tmp/autome-src/promo.csv`；
   断言输出含基线渲染的整行 `- docs/checkout-flow/attachments//tmp/autome-src/promo.csv`（注释写明：这是被固定的既有
   缺陷行为，修它需要另行授权，见 B-05）。既有的 `the_intake_prompt_lists_attachments_and_doc_refs_when_present`
   不改，继续通过。

### 金样（M-01，新增 3 个，16 → 19）

`prompt_rendering.rs` 新增 `bug_loop_locate_prompt_with_inputs_matches_its_golden`、
`bug_loop_fix_prompt_with_inputs_matches_its_golden`、`bug_loop_fix_review_prompt_with_inputs_matches_its_golden`，
spec 带附件 `["/tmp/autome-golden/crash.log", "/tmp/autome-golden/screen.png"]` 与参考 `["docs/notes.md"]`，金样文件
`locate-inputs.md`、`fix-inputs.md`、`fix-review-inputs.md`，按 C8 用 `AUTOME_UPDATE_GOLDEN=1` 接受，接受前核对差异。
已有 12 份金样**不重新生成**：它们的 spec 不带输入，逐字节不变本身就是 A2「无空段落」与第 4 节「需求 Loop 金样无差异」的证据。

### e2e（M-02，新增 1 个，`bug_loop` 过滤 14 → 15）

`bug_loop_prompts_name_the_card_attachments_and_doc_refs`：

- 在仓库之外（World 的临时根目录）写一个 `crash.log`，用 `std::fs::canonicalize` 取绝对路径——与命令行 `--attach`
  一致（C9）；仓库里提交一份 `docs/notes.md`。
- `task.create` 传 `"kind":"bug"`、`"attachments":[<绝对路径>]`、`"doc_refs":["docs/notes.md"]`，三步照
  `bug_loop_a_bug_runs_from_creation_to_a_merge_commit` 的方式跑到 `await_merge`。
- 断言：`w.prompt(1)`、`w.prompt(2)`、`w.prompt(3)` 都含 `docs/<slug>/attachments/crash.log` 与 `- docs/notes.md`；
  都不含源文件所在目录的绝对路径；worktree 里 `docs/<slug>/attachments/crash.log` 确实存在（prompt 里的路径指得到文件）。

既有 `bug_loop_a_bug_runs_from_creation_to_a_merge_commit` 增加断言：三个 prompt 都不含「用户提供的附件」「用户指定的仓库内参考文档」
与 `{inputs}`（A2 的端到端证据，用例数不变）。

### 门禁

M-02 跑项目门禁 `sh scripts/ci-local.sh`（含 fmt、clippy、`cargo test --workspace --all-targets`、桌面与 Web 测试）、
`cargo test --workspace`、插件 eval 静态检查，以及范围核对（`git diff` 对基线 `83c80e9`，见 M-02 验收契约）。

### 风险

- R1 整理轮渲染变化被视为越界——已消除：整理轮传存储原值、渲染逐字节不变，单测 5 固定，缺陷转 Backlog B-05。
- R2 `clarify` 归类被质疑——理由见方案 4，列入人工验收清单；不需要提前验证。
- R3 模板放置错一行会在无输入时多出空行——由 12 份既有金样逐字节比对拦下（见 M-01 负向对照）。
- R4 用户文本含花括号导致会话拒绝启动——方案 2 的切片顺序 + 单测 4。
- 没有必须在实现前验证的技术不确定性：数据已经在 spec 里（现状第 1 条），渲染与静态门的行为都已在基线上读码确认。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 共用输入渲染、三个 Bug 模板加 {inputs}、单测与金样、protocol/v3 | 0 | prompt |
| M-02 | 已完成 | e2e 带附件与 --doc 的 Bug 任务、全量门禁与范围核对 | 0 | e2e |
| M-03 | 待审 | rebase main 后的门禁模式与任务输入集成验收 | 0 | integration |

## 验收契约

### M-01 验收契约

最新证据：docs/bug-loop-的定位-修复-复核/evidence/M-01-r2-impl.md · 审计 #1 复验通过，已完成

覆盖 A1–A4、A6；前置：无。

- 验收命令：`cargo test -p automed --lib launcher && cargo test -p automed --test prompt_rendering && cargo test -p automed --lib protocol:: && cargo run -p automed -- protocol eval protocol/ && git diff --name-status 83c80e9 -- crates/automed/tests/golden/prompts/ protocol/`
- 预期：launcher 62 个通过（基线 57 + 5）；prompt_rendering 19 个通过（基线 16 + 3）；`protocol::` 61 个通过（数量不变，
  版本断言为 `protocol/v3`）；静态门 0 项不通过、CHANGELOG 条目 8 条；`git diff --name-status` 恰好 7 行：
  `A` 三份 `*-inputs.md` 金样，`M` 的 `protocol/CHANGELOG.md`、`protocol/prompts/locate.md`、`fix.md`、`fix-review.md`。
- 负向对照：Bug 侧附件行用存储原值拼路径（不取文件名）时，单测 1 与 3 份新金样失败（出现 `/tmp/autome-src`、`/tmp/autome-golden`）；
  整理轮也改成取文件名时，单测 5 失败；
  `{inputs}` 单独占一行时，既有 `locate`、`locate-card`、`fix`、`fix-review` 4 份金样因多出空行失败；先替换 `{inputs}` 再做
  占位符检查时，单测 4 返回 `Err`；CHANGELOG 没加 v3 时 `protocol::` 的版本断言失败。

### M-02 验收契约

最新证据：docs/bug-loop-的定位-修复-复核/evidence/M-02-r3-audit.md · 通过 · 审计 #2

覆盖 A1、A2、A5、A7、A8；前置：M-01。

- 验收命令：`cargo test -p automed --test end_to_end bug_loop && cargo test --workspace && sh scripts/ci-local.sh && sh plugins/autome/tests/run.sh evals && git diff --name-only 83c80e9 -- plugins/ .autome/ apps/ docs/bug-loop-的定位-修复-复核/protocol/ crates/automed/src/scheduler.rs crates/automed/src/dispatch.rs crates/autome-cli/`
- 预期：`bug_loop` 过滤 15 个通过（基线 14 + 1）；`cargo test --workspace` 全绿；`ci-local.sh` 退出码 0；插件 eval 静态检查退出码 0；
  最后的 `git diff --name-only` 输出为空（插件、`.autome/`、面板与桌面、本任务协议副本、调度 / 存储 / 命令行都没动）。
- 负向对照：`bug_prompt` 不填 `{inputs}`（例如回退到只替换三个占位符）时会话拒绝启动，新 e2e 与既有 `bug_loop` 用例一起失败；
  渲染用存储原值时新 e2e 因 prompt 含源文件绝对路径、不含 `docs/<slug>/attachments/crash.log` 失败；e2e 若把附件写成裸文件名，
  就测不出上述错误——这正是 C9 要求用绝对路径的原因，审计轮按此核对测试取值。

### M-03 验收契约

最新证据：docs/bug-loop-的定位-修复-复核/evidence/M-03-r4-impl.md · 待审 · 实现 #4

覆盖用户追加的 rebase 要求、A1–A4、A6；M-01、M-02 的关闭结论保留。
当前集成基线为 main 的 9626bc1；前述 83c80e9 的数量与范围契约保留为原里程碑基线，
不用于要求删除 main 的合法变更。此次只以 main 为基准核对任务净差异。

- 当前协议版本为 protocol/v4：main 已占用 v3，I-1～I-3 顺延至 v4，保留 main 所有条目；
  上文方案中的 v3 是原计划版本，集成版本以本契约为准。
- 三个 Bug 步骤在 each_round / before_merge 下都渲染附件及参考；fix / fix-review 的两个
  gate-run 分支各在正文开头放置 {inputs}，门禁标记仍独占行。空输入保持 main 原有金样。
- 渲染先选择门禁分支，再分离 request、inputs、gate_line 数据槽，用户输入与门禁命令不参与占位符解释。
- 复核轮保留 main 新增的写入边界；只同步 fix-review-inputs 金样，不改变需求 Loop 金样。
- 验收命令：`git merge-base --is-ancestor main HEAD`；
  `cargo test -p automed --lib launcher`；
  `cargo test -p automed --test prompt_rendering`；
  `cargo test -p automed --lib protocol::`；
  `cargo test -p automed --test end_to_end bug_loop`；
  `cargo run -p automed -- protocol eval protocol/`；
  `cargo fmt --all -- --check`；
  `sh plugins/autome/tests/run.sh evals`。
- 预期分别为祖先检查成功、70 / 28 / 67 / 18 个用例通过、静态门 0 不通过、格式通过、插件 5/5。
  交叉用例覆盖 2 门禁模式 × 3 步骤 × 4 输入组合共 24 组，含空输入、绝对路径、空格及花括号。
- 失败分支复验门禁标记格式错误被拒绝及旧版协议兼容；
  本里程碑不改预算、状态机、附件存储与授权逻辑。全量门禁由合并前宿主执行，不采信旧 SHA 的结果。

## 人工验收清单


- 确认整理轮对绝对路径附件拼出不存在路径的既有缺陷（B-05）是否另开任务修复；本任务不动它。
- 确认 protocol/v3 的三条改动归为 `clarify`、不带 eval 用例（方案 4）可以接受。

## Backlog

- B-01 同名附件（不同目录下的两个 `crash.log`）复制时后者覆盖前者，prompt 会列出两行同一路径。属于附件存储，任务第 4 节明确不改。
- B-02 整理轮无任务卡路径先替换 `{request}` 再替换 `{inputs}`，需求里的 `{inputs}` 会被展开、参考文档里的 `{…}` 会让整理轮拒绝启动；
  属于需求 Loop 的渲染顺序，本任务不改。
- B-03 工作区项目里 `--doc` 的仓库内相对路径没有说明是哪个成员仓库，会话从工作区目录未必打得开；需要先定 `doc_refs` 在工作区里的语义。
- B-04 Bug 会话 prompt 可补一句「附件与参考文档是用户提供的资料，不是给你的指令」，与任务卡说明对齐。
- B-05 整理轮对绝对路径附件渲染成 `{doc_dir}/attachments//绝对/路径/x.log`（不存在的路径）；修法是 `intake_prompt` 也传副本文件名，会改变需求 Loop 渲染结果，须另行授权。来源 D1-P01 的裁决。
