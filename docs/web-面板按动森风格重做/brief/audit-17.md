# 审计轮 简报 · 第 17 轮

这份简报是索引，不是设计文档的替代品。设计文档 `docs/web-面板按动森风格重做/web-面板按动森风格重做.md` 仍然是权威，本轮需要什么就去读什么——但先读这里，多数轮次不必整份拉进来。

## 本里程碑

还没有里程碑表，或者所有里程碑都已完成。

## 本轮要审的集成改动

本轮是**集成复验**：实现 #21 开始时里程碑表没有 `开放` / `待审`，却在 `docs/web-面板按动森风格重做/` 之外留下了提交。规则见 `docs/web-面板按动森风格重做/protocol/loop-protocol.md` 的「集成复验」一节。

触发原因：合并前门禁失败。命令：`sh scripts/ci-local.sh`。输出末尾：
```text
面板启动失败或超时：
Cannot find module '/private/var/folders/f6/r83yddpn0wvftcfn57z_b8440000gn/T/autome install SZOZvO/lib/autome/web/app/lib/waiting.ts' imported from /private/var/folders/f6/r83yddpn0wvftcfn57z_b8440000gn/T/autome install SZOZvO/lib/autome/web/server/overview-summary.ts
[39m
[36m [2m❯[22m Object.run test/install-alignment.test.ts:[2m45:49[22m[39m
    [90m 43|[39m     PREFIX: prefix, AUTOME_WEB_DEFAULT_PORT: '0', AUTOME_WEB_OPENER: o…
    [90m 44|[39m   for (const key of ['NODE_ENV', 'NODE_OPTIONS', 'AUTOMED_BIN', 'AUTOM…
    [90m 45|[39m   const run = (file: string, args: string[]) => execFileSync(file, arg…
    [90m   |[39m                                                 [31m^[39m
    [90m 46|[39m   const install = () => run('/bin/sh', [join(root, 'scripts/install-cl…
    [90m 47|[39m   return { dir, prefix, home, env, run, install, cli: join(prefix, 'au…
[90m [2m❯[22m test/install-alignment.test.ts:[2m72:5[22m[39m

[31m[2m⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/2]⎯[22m[39m

[41m[1m FAIL [22m[49m [30m[43m exclusive [49m[39m test/w9-http.test.ts[2m > [22m设置 action 与 single fetch 安全头和超长保存经网关拒绝
[31m[1mAssertionError[22m: expected 405 to be 200 // Object.is equality[39m

[32m- Expected[39m
[31m+ Received[39m

[32m- 200[39m
[31m+ 405[39m

[36m [2m❯[22m test/w9-http.test.ts:[2m66:27[22m[39m


[31m[2m⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[2/2]⎯[22m[39m


[2m Test Files [22m [1m[31m2 failed[39m[22m[2m | [22m[1m[32m158 passed[39m[22m[90m (160)[39m
[2m      Tests [22m [1m[31m2 failed[39m[22m[2m | [22m[1m[32m1350 passed[39m[22m[90m (1352)[39m
[2m   Start at [22m 04:07:30
[2m   Duration [22m 501.06s[2m (tests 99%, import 1%)[22m

    FAILED (501s) · 输入 f15c38902834d4a074b6929c1e3da3f1d08d147b

==> Web e2e
    沿用 ← T-101/gate@195db5472d81 2026-11-17T19:51:38Z · 输入 29cae744e1877fc44eda17a5676f63912385ca5c

==> 汇总
────────────────────────────────────────
  ok      Test (unit, end to end, adapter flags)
  ok      Desktop install
  ok      Build core
  ok      Desktop
  ok      Contract
  ok      Scripts
  ok      Web install
  FAILED  Web test
  沿用    Format ← T-101/gate@68e1ac6f119e 2026-11-17T19:26:06Z · 输入 3fe5efb8b078269d937b8484affde3972bfcee0f
  沿用    Clippy ← T-101/gate@68e1ac6f119e 2026-11-17T19:26:06Z · 输入 c8e36736a8ea33f150e226c898fa5bdbb231d9ba
  沿用    Web lint ← T-101/gate@195db5472d81 2026-11-17T19:51:38Z · 输入 5846d90b66c8655412347eaadb5c0f3587d87212
  沿用    Web typecheck ← T-101/gate@195db5472d81 2026-11-17T19:51:38Z · 输入 6fc7a4891fdec8706e1794dab9a3f8f2ddbdaa6f
  沿用    Web build ← T-101/gate@195db5472d81 2026-11-17T19:51:38Z · 输入 f5b142d3bb80ad5913708634cd34a0a325a5d155
  沿用    Web e2e ← T-101/gate@195db5472d81 2026-11-17T19:51:38Z · 输入 29cae744e1877fc44eda17a5676f63912385ca5c
失败用例 · Web test（2）：
  - exclusive  test/install-alignment.test.ts > 空 PREFIX 安装、打开生产面板、同包内核、重复安装与停止
  - exclusive  test/w9-http.test.ts > 设置 action 与 single fetch 安全头和超长保存经网关拒绝
执行 8 步，沿用 6 步，失败 1 步。
有步骤未通过，用时 1208s。
```

提交范围：`597d0f5..6c356f8`（默认分支基线 `7326a91`），rebase 前后补丁不变的提交不列出。

列出的提交 1 个：
- `6c356f8` fix(web): 补齐发布依赖并对齐设置路由验收
  - apps/web/scripts/stage-panel.ts
  - apps/web/test/install-alignment.test.ts
  - apps/web/test/settings-onboarding.test.ts
  - apps/web/test/w9-http.test.ts

改动统计（`docs/web-面板按动森风格重做/` 之外）：
```text
 apps/web/scripts/stage-panel.ts           | 3 ++-
 apps/web/test/install-alignment.test.ts   | 4 ++++
 apps/web/test/settings-onboarding.test.ts | 4 ++--
 apps/web/test/w9-http.test.ts             | 6 +++---
 4 files changed, 11 insertions(+), 6 deletions(-)
```

rebase 前有、rebase 后找不到同一补丁的提交 0 个：

改到的测试文件（逐个看 diff，留意测试前提有没有被改弱）：
- apps/web/test/install-alignment.test.ts
- apps/web/test/settings-onboarding.test.ts
- apps/web/test/w9-http.test.ts

本轮证据必须写入 `docs/web-面板按动森风格重做/evidence/M-13-r21-audit.md`。结论二选一：通过（里程碑表不动）；或把受影响的里程碑退回 `开放`、`reopen` 加 1。两种都要写这个证据文件。

## 上一轮对手轮的结论

没有待审的里程碑。

## 本轮相关的协议小节

全文在 `docs/web-面板按动森风格重做/protocol/`。下面是与本轮直接相关的几节。

## 共同原则

1. **产品结果优先。** 以上规范是为了让任务完成，不是为了让流程文件完善。
   不得为了补全流程记录而推迟主要工作。
2. **证据优先。** 涉及代码现状、运行行为或外部接口的判断，优先使用源码、
   现有测试、实际命令或最小验证实验。纯逻辑矛盾可以用准确引用和具体反例证明。
3. **一次处理同类问题。** 提出的问题必须说明根因和检查范围，并列出该范围内
   全部同类问题。修复时再次检查同类位置，一次处理完毕。
4. **不追求完美。** 设计阶段只消除会使方向失效、目标受损或里程碑不可执行的
   问题；审计只以「产品行为不符合设计或任务」为缺陷。「还可以更好」一律进
   Backlog。
5. **验证用项目原有体系。** 优先使用项目已有的测试、契约、运行脚本和真实环境。
   只有确有需要时才新增专用验证程序。
6. **协议版本固定。** 本次运行始终使用任务文件内的这份规则，不得中途换版本。
7. **会话自主。** 每轮在独立会话中完成，不得向用户提问如何继续。遇到未明确
   规定的事项，依据任务目标、项目规则和已有证据作出决定并记录理由。
8. **问题必须可追溯。** 每条评审或审计意见都必须写明它违反的任务要求编号、
   设计条款或里程碑验收命令。建立不了这种追溯的意见一律进 Backlog，
   不得触发复审、reopen 或额外轮次。

## 文件

- 设计文档 `docs/<slug>/<slug>.md` —— 头部状态块 + 正文，覆盖式维护。
  标题下一行、首个二级标题前写状态块；不得复制展示用围栏。
- 设计评审 `docs/<slug>/<slug>-review.md` —— 每轮覆盖写，只保留当前结论。
- 裁决记录 `docs/<slug>/<slug>-adjudication.md` —— **只增不改**，设计循环
  唯一的跨轮记忆。
- 实现审计 `docs/<slug>/<slug>-audit.md` —— 每轮覆盖写。
- 证据文件 `docs/<slug>/evidence/M-xx-r<k>-impl.md` 与 `-audit.md` —— 每轮为
  本轮推进或复验的里程碑写一份，**只增不改**。验收证据、自审清单、修复说明、
  审计的独立复验结论都写在这里。
  **文件名末尾的角色后缀不能省。** 审计轮不增加 `k`，所以同一个 `k` 的实现轮
  和审计轮会写到同一个路径，后跑的那个把先跑的覆盖掉——退回的理由就这样丢了。
- 运行记录 `docs/<slug>/retro.md` —— 每轮追加**一行**，任务结束时补总结。
- 本轮简报 `docs/<slug>/brief/<角色>-<轮次>.md` —— Autome 生成，会话只读。
- 协议副本 `docs/<slug>/protocol/` —— 本任务固定的那一版协议，Autome 在创建
  任务时放进来。**它是本任务规则的唯一一份**，任务文件不再抄写协议正文。

超过 5 行的命令原始输出和临时验证产物写进 `.autome/output/`，不要进任务目录
（怎么做见会话协议「读文件的开销」）。证据文件写的是结论与判据，不是原始输出
的转存。

### 证据不写进设计文档

设计文档是**当前状态**，不是日志。每个里程碑在设计文档里只留一行指针：

`最新证据：docs/<slug>/evidence/M-03-r7-audit.md · 通过 · 审计 #4`

展开的内容——命令与结果、逐条对照表、修复说明、审计结论——写进那一轮的
证据文件。设计文档的其它小节同样不得追加轮次记录：背景与现状写的是现状，
方案写的是当前方案，事实变了就地改写，不要在后面接一段「实现轮 #7 记录」。
审计结论只写进 `<slug>-audit.md` 与证据文件，不再抄进设计文档。

这条有可核对的来历：三次真实运行的设计文档分别长到 335KB、323KB、230KB，
其中约七成是逐轮追加的证据块。每个会话开场都要读它，之后每一次往返还会
带着它。2026-09-16 的一次协议失败也出在这里——一张写在里程碑节中间的
对比表被当成了里程碑表，整份文档读不出来，任务停摆。

### 运行记录只有一行

`retro.md` 每轮**一行**，固定五段，不超过 200 字：

`轮次 | 里程碑 | 结果 | 证据 | 阻塞`

- 轮次：`裁决 #1` / `实现 #7` / `审计 #4`
- 结果：`设计定稿` / `待审` / `通过` / `退回 + 领域名`
- 证据：本轮证据文件的路径
- 阻塞：没有就写「无」

叙述、理由、命令输出都在证据文件里，不在这里重复。只有任务终止时的那段
总结可以成段。

## 实现循环

实现轮推进**最小编号的开放里程碑**，取得通过证据后标为 `待审`，然后结束会话。
一轮只推进一个里程碑。

### 标 `待审` 之前的自审清单

验收命令通过之后、把里程碑标成 `待审` 之前，实现轮必须在本轮证据文件里写下
这四项并逐项执行：

1. **情形表逐行** —— 对照设计里本里程碑的情形表或落法表，逐行列出检查与结果。
2. **失败分支** —— 设计点名的每条失败分支各跑一个检查：拒绝授权、读盘失败、
   超时、空输入、外部服务不可达。
3. **全部迁移** —— 有状态机或多通道的，列出全部迁移（状态 × 事件、通道 ×
   方向）并各跑一次，不只跑正路径那一条。
4. **域边界** —— 有输入域声明的，跑它的边界：前导零、闰日、跨年、空串、
   最大值、单元素。

某一项不适用就写「不适用」并说明一句，不要跳过不写。

这不替代审计，也不是要把审计的活提前干完。它挡的是「拿着设计就能列出来」的
那一类漏洞：一次真实运行的 9 个实现缺陷，全部落在上面四类里，而每一个都花掉
了一对实现轮与审计轮。

### 验收结果报什么

报**用例总数，以及它与上一轮基线的差**，并说明这个差是从哪来的。只报
「通过 / 失败 / 跳过」三个数不够。

证据：一次真实运行里，同一份代码的两次全量，一次 736 个用例、一次 822 个，
两次的「跳过」都是 0——整层被跳过时这个 0 是瞎的，少掉 86 个用例它藏得住，
用例总数藏不住。

### 审计轮

审计轮独立复验：自己跑验收命令，自己构造能区分错误实现的检查，不看实现轮的
推理过程。结论三选一：

- **通过** —— 标为 `已完成`。
- **实现缺陷** —— 产品行为不符合设计或任务。退回 `开放`，`reopen` 加 1，
  并在 `领域` 列按稳定的行为领域名归组。同一审计轮的同一领域只计一次；
  领域名按根因复用，不得改名规避升级。
- **验证缺口** —— 现有验收可能放过错误的实现，但当前产品实现**没有被证明
  是错的**。审计轮当场加强验收或补检查，把临时的缺陷注入完全还原，然后立即
  复验。复验通过则里程碑照常关闭：**不计 reopen，不退回实现轮**。

三者的分界只有一条：产品行为错了没有。错了是实现缺陷；没错但证明不住是验证
缺口；都没有是通过。代码风格、超出验收范围的健壮性、性能微优化、测试还可以
更多，一律进 Backlog。

**没有第四种结论。** 三种都会动里程碑表：通过标 `已完成`，实现缺陷退回 `开放`，
验证缺口补强复验后照常关闭。审计轮结束时表和开始时一样，就是落在了不存在的
第四种上——Autome 会在这里停下任务等人。

最常见的那一种长这样：里程碑剩下的未取证项**全是人工验收项**（真实鼠标、
麦克风、系统弹窗、肉眼看横幅），审计既拿不到证据关闭它，又没有产品缺陷可以
退回它。这时**不得**保持 `待审`。按「验收必须是会话自己能跑的」处置：把这些项
逐条移进 `## 人工验收清单`（沿用 `H-xx` 编号，一条一行），然后按剩下那些
会话自己能跑的验收项下结论并关闭里程碑。不计 reopen，不退回实现轮。人工那几条
由用户在合并前确认。

保持 `待审` 看起来最诚实，实际上是最贵的：下一个实现轮会发现没有 `开放` 的
里程碑、无事可做，这一对会一直空转到预算耗尽。一次真实运行这样烧掉了十三轮。

**复现不了不等于不存在。** 审计怀疑有缺陷却复现不出来时，不得就此结案：
要么给出「为什么复现不出来」的机制性解释，要么用时序或因果证据替代复现。
在并发与时序问题上，「复现不了就放过」会系统性地漏报。

### 审计造的检查归谁

审计**发现实现缺陷**时所用的判别检查，由下一轮实现轮逐字搬进项目测试体系，
作为这个缺陷的回归用例。搬之前先原样跑一遍复现（应为红），修复后再跑（应为
绿），两次都记进证据文件。

审计**通过**时新造的检查可以丢弃，只在审计文件里记一行：构造了什么、结果如何。

审计轮不重跑前几轮留在 `.autome/output/` 里的临时检查。复验范围就三样：项目
测试体系（全量跑一次即可，不必反复单跑子集）、本里程碑的验收命令、本轮自己
新造的检查。已经闭合的缺陷由回归用例守住，不靠一轮一轮手工重跑——那既慢又会
随轮次线性变贵，而且合并之后那些临时检查一个都不会留下。

### 轮次与预算

实现轮数 `k` 由实现轮增加，审计轮不增加。

**总预算 `N` 由 Autome 计算，写在每轮 prompt 的第一行，分母以它为准。**
不要自己按里程碑数推算，不要因为一次停顿或放行改写它。`k` 达到 `N` 时任务
停下等用户，是否追加由用户决定。

### 收敛模式

- 同一领域第二次 reopen → `convergence-mode: domain-review`
- 同一里程碑第三次 reopen → `convergence-mode: milestone-review`

`milestone-review` 优先级更高。收敛模式一旦触发，直到该里程碑关闭前不得
恢复为 `normal`。

触发收敛模式的审计轮必须在审计文件顶部维护 `## Convergence Note`，固定四部分：
未闭合的行为领域与当前失败证据；根因、同类检查范围以及前轮为何漏检；
下一轮应完成的具体工作与完成条件；当前应红、完成应绿、不得退化的命令。
后续审计轮覆盖写审计文件时必须保留并更新这个 Note，直到里程碑关闭。

## Backlog 与争议项

<!-- kernel-contract: backlog-disputes -->
`## Backlog` 收非阻塞的改进建议：评审提出的有跨轮价值的实现注意事项由裁决轮
追加，审计提出的非阻塞建议由审计轮追加。每条一行，以稳定 ID 开头。
Backlog 条目**不触发轮次**，等用户处置。

`## 争议项` 收复提两次冻结的主张，格式同上。
<!-- /kernel-contract -->

用户在界面上对这两节作出的决定，会由 Autome 在下一个停顿点注入到会话的
prompt 里。看到这类注入时，按其中写明的处置执行：纳入的 Backlog 条目要成为
新的里程碑并实现；忽略的和已裁定的要写进 `retro.md`。

## 状态块

设计文档 `<doc_dir>/<slug>.md` 的头部必须维护以下字段，格式严格：

<!-- kernel-contract: status-block -->
```text
status: 设计中 | 实现中 | 已完成 | 不可实现 | 协议失败
design-round: d/N
implementation-round: k/N
current-milestone: M-xx | 无
current-milestone-reopens: r
convergence-mode: normal | domain-review | milestone-review
next-action: <下一实现轮首先完成的具体工作；没有时写"无">
repos: <本任务要改动的仓库，逗号分隔；单仓库项目省略这一行>
```
<!-- /kernel-contract -->

`repos:` 只在工作区项目里有意义——那种项目的工作目录下是若干个各自独立的仓库。
**写下一个仓库名，就是请 Autome 为它建检出和分支**；你自己不要建 worktree、不要
切分支。只出现在这一行里的仓库才会被检出，也才会在合并时被合并。

这一行只增不减：后面的轮次发现还要动别的仓库，追加名字；**不要删掉已经在里面的**
——删掉不会撤销任何东西，只会让那个仓库的改动在合并时被漏掉。

里程碑用固定表格，Autome 按列读取：

<!-- kernel-contract: milestone-table -->
```markdown

## 预算与用户表态

本轮是第 17 轮，实现预算 N = 65。分母由 Autome 计算。

## 宿主门禁

本轮宿主只跑了快检：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`，在提交 `6c356f82d7` 上通过，退出码 0。完整输出：`/Users/dannie/project/autome-v2/.autome/output/gates/T-101/gate-6c356f82d78f.log`。
快检不是全量门禁：它只说明这条命令里的检查是否通过，不能当作全量回归的证据；全量门禁只在合并前运行。复验时可以引用它，但它不替代你自己的负向对照。
