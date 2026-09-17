# Loop 机制的剩余优化空间与自我改进体系（v2，经审计精细化）

日期：2026-09-17。承接 `2026-09-16-loop-v1-protocol-audit.md`（S1–S7 已落地，脚手架 v5，尚无真实运行验证）。

v2 相对 v1 的修订来自两处：对代码现状的逐条核对（§2 的"代码核对"列），以及一次对抗式审计（30 条，主要修正见 §9）。

## 0. 结论

Loop v1 已经具备自我改进体系的骨架：文件即状态、每轮重置上下文、生成与评测分离、retro 里的协议改进建议。缺三样东西：

1. **观测层。** 内核不记录任何会话用量与结果指标。Claude 的原始 JSONL 已经落盘且末尾 `result` 事件带完整费用与用量，只是没人读；Codex 目前不带 `--json` 跑，什么都抓不到。
2. **教训的流转通道。** 任务内的教训在任务结束后死在任务目录里，不会变成项目规则、回归测试或协议条文。
3. **确定性守卫。** 协议里的"不得 / 必须"靠模型自觉，其中一半可以在会话结束、内核解析状态块的同一处用代码检查。

自我改进功能（§6）的设计是：**协议变成版本化数据，"改协议"变成一个普通 Loop 任务，跑在协议仓库上；两道人工门，回滚是向前的新版本而不是移动指针。**

会话复用不在方案内，理由见 §7。

## 1. 参照物

| 来源 | 对 Loop 有用的结论 |
|---|---|
| Anthropic《Effective harnesses for long-running agents》2025-11 | 初始化轮 + 增量轮、每轮一个 feature、进度文件 + git 作跨会话记忆；压缩不够，要完全重置上下文并从结构化交接文件重建。Loop 已是此形态。 |
| Anthropic《Harness design for long-running application development》2026-03 | 规划 / 生成 / 评估三角；**Sprint Contract**：开工前就"什么算完成"达成一致；模型变强后砍掉 sprint 与逐轮评估。"每个 harness 部件都假设模型做不到某件事，这些假设会过期。" |
| Anthropic《Effective context engineering for AI agents》2025-09 | 最小高信号 token 集；按需检索优于预加载；带清晰里程碑的迭代开发适合笔记法而非压缩。 |
| OpenAI《Harness engineering》2026 | 一份大 AGENTS.md 失败，改分层文档 + 渐进披露；架构规则做成 linter 而非条文；专门清理过期文档；评审 agent 对 agent，人的瓶颈变成 QA。 |
| OpenAI Cookbook《Agent improvement loop》2026-05 | trace + 反馈 → 诊断 → 一份 handoff（诊断、排序建议、证据、实施指引）→ 改 harness → eval 门 + 人审 diff。弱 eval 会把优化引偏，评估集要人把关。 |
| Lilian Weng《Harness engineering for self-improvement》2026-07 | ACE 的条目化增量合并；Self-Harness 的"弱点归根因、有界提案、无回归才接受"；AHE 的"每次改动是可证伪的主张，带预测影响"。评估器与权限在被演进的循环之外；人往上层走。 |
| Karpathy autoresearch 2026-03 | 棘轮只保留改善指标的改动。局限：只接受单步改善；样本多才有效。 |

## 2. 现状对照（含代码核对）

| 能力 | 现状 | 代码核对 | 差距 |
|---|---|---|---|
| 文件即状态、每轮重置 | 有 | `scheduler::reap_session` → `read_outcome` → `advance` | 无 |
| 生成 / 评测分离 | 有 | `Role::same_model_counterpart`；实现轮不得标已完成只在协议条文里 | 后者无代码守卫 |
| 开工前完成契约 | 半有 | 里程碑表有验收命令列 | 缺预期与负向对照 |
| 观测指标 | 无 | `sessions` 表只有 exit_code、outcome、log_path；`stream_render.rs` 只渲染；Claude JSONL 落在 `.autome/output/sessions/<task>/<session>.jsonl`，`result` 事件含 `total_cost_usd`、`usage.{input,cache_read_input,cache_creation_input,output}_tokens`、`num_turns`、`duration_api_ms`、`modelUsage`；Codex adapter 是 `exec --sandbox workspace-write`，无 `--json` | 加 Codex `--json`；reap 时抽字段；store 迁移 v2 |
| 任务内教训 → 项目规则 | 无 | `.autome/` 对会话只读（AGENTS 标记段明文） | 缺提炼者与审批通道 |
| 任务内教训 → 回归测试 | 有（S4） | 协议条文 + 实现 prompt | 无 |
| 多任务教训 → 协议 | 半有 | retro 末尾三条建议，人工汇总 | 缺聚合、证据链、版本化 |
| 条文的确定性守卫 | 只有状态块解析 | `status_block::parse` 一处 | 证据文件、retro、文档体积无检查 |
| 条文来源与退休 | 无 | 协议只增不减 | 无 CHANGELOG |
| prompt / 协议 eval | 1.x 有 3 个 case | `legacy/.../evals/<case>/{case.yaml, scaffold.sh, graders/*.md}`，`runs: 3`、`max_turns`、机检优先、阳性对照 | 2.0 未接 |
| 协议是否可迭代 | 否 | `init.rs::LOOP_PROTOCOL_MD`、`SESSION_PROTOCOL_MD` 与 `launcher.rs::role_prompt` 都是编译期常量，约 20 个单元测试断言 prompt 里的固定短语 | 协议出仓是自我改进的前提 |
| 节点与角色 | 11 个 Node，5 个 Role | `Node::role()` 映射；Intake 是固定系统步不占角色 | 复盘轮需要新 Node + 第六个 Role |

## 3. 数据模型（新增）

所有后续条目都引用这里的结构，避免"面板显示""内核聚合"这类空话。

### 3.1 SessionMetrics（store 迁移 v2，`sessions` 表加列）

```text
session_id · task_id · role · runtime · model
protocol_ref        协议版本引用（§6.2 的 tag + 内容哈希）
rules_hash          .autome/rules/ 内容哈希
input_tokens · cache_read_tokens · cache_write_tokens · output_tokens
cost_usd            Claude 取 result.total_cost_usd；Codex 为 NULL（Codex 不报价，不自造价格表）
turns               Claude 取 result.num_turns；Codex 取 turn.completed 事件计数
duration_ms         Claude 取 duration_api_ms；Codex 取墙钟
mean_request_input  每次模型请求的输入 tokens 均值（含缓存命中），从流里逐条 assistant 事件的 usage 求均值。
                    这是"每 turn 上下文"的定义；不用 cache_read ÷ turns，那个在缓存未命中时失真
design_doc_bytes · evidence_files   会话结束时量 worktree
```

跨 runtime 比较只用 tokens 与 turns，不用费用。

### 3.2 TaskMetrics（任务终止时聚合，存 `tasks` 表 JSON 列，同时写进 retro 终止总结）

```text
protocol_ref · rules_hash
design_rounds_used / limit · impl_rounds_used / budget_n
milestones · reopen_total · reopen_by_domain{领域: 次数}
impl_defects · verification_gaps          来自审计文件的三选一结论计数
protocol_failures                          Failed(protocol) 次数（含重跑）
closed_then_contradicted                   已完成后被后续审计退回、或合并卡点人工验收未通过的里程碑数
manual_items_open                          合并时人工验收清单未确认条数
total_cost_usd（仅 Claude 会话之和）· total_tokens · total_turns
```

`closed_then_contradicted` 是"审计放水"的直接测量，替代 v1 里"缺陷数降且 reopen 降"的启发式，后者会在协议真的变好时误报。

### 3.3 Lesson（复盘轮产出，`docs/<slug>/lessons.md`，每条一个 YAML 块）

```yaml
- id: L-01
  domain: verification            # 固定词表：design | verification | implementation | protocol-format | tooling | process
  symptom: 审计 #3 在 M-02 因情形表第 4 行未覆盖退回
  root_cause: 实现轮自审清单"情形表逐行"被写成"不适用"但未说明
  evidence: docs/voice-schedule/evidence/M-02-r5-audit.md
  level: rule | test | brief | protocol
  proposal: 所有标"不适用"的自审项必须引用设计文档中证明其不适用的条款   # 可检验的一句话
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

跨任务同一教训的识别键：`domain` + 归一化后的 `proposal`（去空白与标点、全角转半角）。不做语义聚类。

### 3.4 ChangelogEntry（协议仓库 `CHANGELOG.md`，每个版本一节，每条改动一个 YAML 块）

```yaml
- id: C-07
  kind: behavioral | clarify | retire
  clause: loop-protocol.md#实现循环/自审清单
  evidence: [voice-schedule L-01, island-workbench L-04]
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
  eval: evals/self-check-not-applicable/        # behavioral 必填；clarify 可空；retire 填被移除的 eval
  realized_impact: null                          # 内核在 horizon 个任务后回填
```

三种 kind 的门槛不同：`behavioral` 必须带"改前红、改后绿"的 eval case；`clarify` 不需要 case，靠评审轮签字；`retire` 必须带指标证据，且以"移除实验"的形式提出（§6.6）。

### 3.5 ProtocolRef

```text
tag: protocol/v7 · hash: sha256(所有协议文件按路径排序拼接)
```

任务创建时把当时协议全文复制到 `docs/<slug>/protocol/`（受跟踪），Task 记录 `protocol_ref`。会话读的是任务目录里这份，不是全局仓库，协议原则 6"版本固定"与归档可复现性由此保留。tag 只是人读的名字，比较用 hash。

### 3.6 Eval case（沿用 1.x 布局）

```text
evals/<name>/
  case.yaml        name · role · runs(默认 3) · max_turns(默认 15) · timeout · scaffold · graders[]
  scaffold.sh      从 fixture/ 搭出最小 worktree（只含脱敏后的任务文档，不含项目代码）
  fixture/         docs/<slug>/ 快照
  graders/*.md     机检优先（读改后文档、看 transcript 里的工具调用），必带阳性对照
```

`max_turns` 上限使 case 只断言前几步行为（读了哪些文件、写了哪个文件、第一条命令是什么），不断言任务完成。这是成本上限的来源。

## 4. 方案 A–G

### A. 观测层（前提，小）

改动：
- Codex adapter 加 `--json`；包装脚本对 Codex 也保留 `.jsonl`。
- `reap_session` 在 `read_outcome` 之后读 `.jsonl`，填 §3.1；store 迁移 v2。
- 任务进入 Done / Failed / Cancelled 时聚合 §3.2。
- 面板：任务卡片显示费用、turns、轮次；项目页一张按任务的表；设置页"协议版本"页（§6.5）。

验证：跑一个真实任务，`sessions` 表里每个会话有非空 tokens 与 turns；Claude 会话的 cost_usd 与 JSONL 里 `total_cost_usd` 一致；Codex 会话的 turns 与 JSONL 里 `turn.completed` 计数一致。

### B. 按需上下文（中）

1. **简报。** `start_session` 前内核生成 `docs/<slug>/brief/<node>-<round>.md`，内容固定四段：当前里程碑表行；上一轮该角色的对手文件对本里程碑的结论（实现轮看审计文件中该里程碑的段落，审计轮看该里程碑最新证据文件路径）；该角色相关的协议小节（按 `## 角色` 表与标题静态映射，映射表在内核）；预算行与用户表态。**不**尝试抽"对应设计小节"，那需要里程碑表加列，属于内核契约改动，不值。prompt 改为"先读简报，再按需读设计文档"。
2. **协议单份。** 任务文件不再内嵌协议，只写 `protocol_ref` 与 `docs/<slug>/protocol/` 路径。
3. **规则按路径分层。** `.autome/rules/*.md` 支持可选前置字段 `paths: [...]`；内核在初始化时镜像到 `.claude/rules/autome-<name>.md`（带 `paths`）与各目录 `AGENTS.md` 的 Autome 标记段内。只写标记段，不碰用户内容。

验证：同一条需求原文在重置分支上改前改后各跑一次，比较 `mean_request_input` 与 `turns`。不同任务之间不比。

### C. 里程碑契约（中）

里程碑表不加列。设计文档为每个里程碑在表外固定小节 `### M-xx 验收契约` 写三行：验收命令、预期（含用例总数）、负向对照（至少一种错误实现会怎么失败）。评审轮六类问题里"里程碑不可执行"的判据：三行缺一即不可执行。审计轮先跑负向对照再自造检查。

### D. 确定性守卫（中）

原则：内核能用代码检查的，不写成"不得"。落点是 `read_outcome` 返回 `Ok` 之后、`advance` 之前。

| 检查 | 处置 |
|---|---|
| 实现 / 审计轮必须新增 `evidence/M-xx-r<k>-impl.md` 或 `-audit.md`（文件名加角色后缀，否则审计轮与同 k 的实现轮撞名，这是协议文本改动） | `protocol_error` |
| 实现轮把任一里程碑从非 `已完成` 改成 `已完成` | `protocol_error`。不由内核改写文档，改写会让提交记录与会话日志不一致 |
| 审计轮修改了设计文档里程碑列以外的内容（按 diff 行判断） | 警告事件 |
| retro 本轮新增行数 ≠ 1 或 > 200 字 | 警告事件 |
| 设计文档 > 80KB 或较上轮增长 > 10KB | 警告事件 |

对应条文改写为"Autome 会检查：……"。警告事件进 `events` 表并在任务卡片上显示，不阻断。

### E. 教训流转

**E1 复盘轮（新 Node + 第六个 Role）。** 明确是状态机改动：

- `Node::Retro`，`Node::role() = Some(Role::Retro)`，占槽位。
- 转换：`Audit(全部已完成) → Retro → Rebase`；`Implement(审计关闭且全部非开放) → Retro → Rebase`。Failed / Cancelled 状态下用户可点"复盘"手动触发一次 `Retro`，结束后回到原状态。
- `RoleConfig` 扩为 ×6；SAME-MODEL 约束 `retro ≠ impl`（它评价实现轮的产出）。`Role::same_model_counterpart(Retro) = Some(Impl)`。
- 读：retro、全部 evidence、audit 历史、adjudication。写：`docs/<slug>/lessons.md`（§3.3）。不得写 `.autome/`。
- 复盘轮 prompt 首行由内核给出 §3.2 的 TaskMetrics，让它对着数字写而不是对着印象写。

**E2 项目级策展（内核代码，不是会话）。** 内核按 §3.3 的键聚合项目内所有 `lessons.md`。同一键在第二个任务出现且 `level: rule` 时，面板出现"建议入规"卡片：展示将写入 `.autome/rules/<domain>.md` 的 diff，用户批准才写入，写入时带出处注释 `<!-- since: <date> · from: <task> L-xx, <task> L-yy -->`。

**E3 规则退休。** 不以"没再出现"为退休依据，规则的存在本身就是不再出现的原因。退休只能作为一次移除实验：面板对超过 6 个任务未被任何新教训引用的规则提示"可发起移除实验"，用户确认后规则移除并记录 `predicted_impact: {metric: <该规则对应的领域 reopen>, direction: flat, horizon: 3}`，到期回填，恶化则一键恢复。

**E4 协议级** 由 §6 承担，v1 里的"内核生成审计草稿"变成 §6.4 的 `inputs/`。

### F. eval（中）

移植 1.x 三个 case 到 §3.6 布局，放进协议仓库 `evals/`。新增：`impl-handles-audit-return`（实现轮先处理退回项）、`impl-writes-evidence-not-design`（证据出仓）、`audit-no-history-rerun`（不重跑历史矩阵）。每个 case 的 grader 必须有阳性对照。launcher 里约 20 个断言固定短语的单元测试迁到 §6.5 第一层的"模板必含短语"检查，对着当前生效模板跑而不是种子。

### G. 降 turn 数（小）

session-protocol 加"同一文件一轮内只读一次，超过 200 行用 `sed -n` 分段读"；包装脚本把超长命令输出截为头尾各 50 行、全文落 `.autome/output/`；实现轮 prompt 加"不跑与本里程碑无关的测试子集，但项目测试体系全量至少跑一次"。

## 5. 依赖关系（修正 v1）

不依赖 A 就能做、且不需要数据支持的：D（守卫）、E1（复盘轮产出文件即可）、F 的静态与解析层、G。
需要 A 才有意义的：B 的验证、C 的验证、E2 / E3（要跨任务聚合与回填）、§6 全部。
需要协议出仓（§6.2）的：B.2、F 的运行、§6 全部。

## 6. 内置 Self-improve：Loop 迭代 Loop

### 6.1 一句话设计

把协议变成版本化数据，把"改协议"做成一个普通 Loop 任务，跑在协议自己的仓库上。设计轮提改动，评审轮挑毛病，实现轮改文本并配 eval，审计轮跑 eval 门，合并卡点就是人批准。SAME-MODEL、里程碑三态、预算、worktree 全部复用。**这一节不新增状态机**；E1 的 Retro 节点是任务层的改动，在 §4 里声明。

### 6.2 协议出仓

```text
~/.autome/protocol/                 独立 git 仓库；首次启动用二进制内置默认初始化并打 protocol/v1
  loop-protocol.md
  session-protocol.md
  prompts/{intake,plan,review,adjudicate,impl,audit,retro}.md    模板；内核填 {slug} {budget_line} {decisions} 等占位符
  brief-map.toml                    角色 → 协议小节标题 的映射（B.1 用）
  evals/                            §3.6
  CHANGELOG.md                      §3.4；v1 由内核用 09-16 审计的 S1–S7 与协议现有条文预填，每条 evidence 指向审计文档小节
  contract.toml                     内核契约区清单：文件 + 标记名 + 期望内容哈希
```

- 内核契约区在协议文件里用 `<!-- kernel-contract: status-block -->` … `<!-- /kernel-contract -->` 包住：状态块字段、里程碑表列、`## Backlog` / `## 争议项` 格式、四条守卫条文（实现轮不得标已完成、审计独立复验、SAME-MODEL、`.autome/` 只读）。**期望哈希存在内核里**（`contract.toml` 随二进制更新时重写），不存在被编辑的文件里；检查同时校验标记对数与区内哈希，标记被移动或删除即失败。
- 项目 `.autome/config.toml` 的 `[loop]` 加可选 `protocol = "protocol/v7"`，缺省跟随全局仓库最新 tag。任务创建时复制协议全文到 `docs/<slug>/protocol/` 并记录 `protocol_ref`（§3.5）。**会话只读任务目录里的副本。** 团队成员各自的 `~/.autome/protocol/` 不一致不影响已创建任务；影响的只是新任务用哪个版本，而 `protocol_ref` 的 hash 会暴露差异。
- 版本变更对运行中任务无影响：它们已经持有副本。
- 二进制升级带新的内置默认时，不覆盖用户仓库，只在仓库里新增一个 `protocol/vN-upstream` tag 供用户对比合并。

### 6.3 可改与不可改

| 面 | 可改？ |
|---|---|
| 协议正文、prompt 模板、`brief-map.toml`、eval case | 可改 |
| 内核契约区（§6.2） | 不可改，eval 第一层拒绝 |
| 状态机、转换表、预算、守卫代码 | 不在本功能内；那是开发 autome-v2 本身 |
| `.autome/rules/` | 走 E2，不走本节 |

**自指问题。** 元任务的所有会话运行在**当前** tag 上，提议的是下一版。评审轮判的是"改动是否越界、证据是否足够、predicted_impact 是否可测"，六类问题重映射为：越界（碰契约区）、证据不足（少于两个任务）、不可测（metric 不在 §3.1 / §3.2 词表内）、与既有条文冲突、eval 缺失（behavioral 无 case）、改动不可追溯（无 CHANGELOG 条目）。**对评审 prompt 或审计 prompt 本身的改动，评审轮无资格判**，只能靠 eval 第三层 + 人在两道门看。内核在 Design 结束时检查：若 diff 触及 `prompts/review.md` 或 `prompts/audit.md`，该里程碑标记 `需人工特批`，在两道门上高亮。

### 6.4 一次迭代的流程

```text
触发 → Intake → Design → Review → Adjudicate → AwaitDesignApproval（门 1：看改什么、为什么）
     → Implement → Audit → Rebase → AwaitMerge（门 2：看 diff、eval 报告、证据链）→ Merging → 新 tag
     → 之后 horizon 个任务 → realized_impact 回填 → 版本页对比 → 保留 / 向前回滚
```

**协议仓库作为内置 Project。** 内核在 `projects` 表里预置一条记录：path = `~/.autome/protocol/`，`onboarding = skipped`，`parallel_limit = 1`，无 `.autome/rules/`，默认分支 main。Rebase、合并前置、worktree 清理、`docs/<slug>/` 归档全部按普通项目走，不写特例。元任务的 `docs/<slug>/` 就在协议仓库里，随之受跟踪。

**触发。** 设置页"改进 Loop"按钮；或以下任一成立时面板提示（只提示，不自动开跑）：自上一 tag 以来完成 ≥ 3 个任务；`lessons.md` 中 `level: protocol` 的同键条目 ≥ 2 个任务；任一任务 `Failed(protocol)`。

**Intake 输入由内核组装到 `docs/<slug>/inputs/`：**

- `metrics.md`：§3.2 按任务的表 + §3.1 按角色均值，都带 `protocol_ref`。
- `lessons.md`：全部项目中 `level: protocol` 的条目按键合并，附出现的任务列表。
- `retro-suggestions.md`：各任务 retro 末尾建议，带任务名。
- `failures.md`：`Failed(protocol)` 的原始状态块与解析错误；上一版本 `realized_impact` 与预测相反的条目。
- `deferred.md`：上一次元任务 Backlog 里的条目（见下）。

任务原文固定："依据 inputs/ 的证据，提出对协议正文与 prompt 模板的改动。每条改动是 CHANGELOG 里的一条（§3.4），引用至少两个任务的证据，predicted_impact 的 metric 只能取 §3 词表。不得触碰契约区。"

**Design / Review / Adjudicate** 同普通任务；里程碑表每行一条改动，验收命令固定为 `autome protocol eval --changed`。设计定稿停在门 1。

**Implement** 改文本、写 eval case（behavioral 必须）、追加 CHANGELOG 条目。**Audit** 跑 `autome protocol eval` 全部三层。

**Backlog 与争议项在元任务里的语义：** Backlog = 推迟到下一次迭代的改动，合并后由内核写进下次的 `inputs/deferred.md`；争议项 = 设计与评审对某条改动的分歧，门 1 由用户裁定。

**AwaitMerge** 展示：协议 diff、eval 报告（三层各自结果）、每条 CHANGELOG 条目的证据链接、`需人工特批` 标记。批准后合并、打 tag `protocol/v<n>`、计算 hash。

**回滚是向前的。** 版本页每个版本旁"回滚到此版本"= 在协议仓库 `git revert` 生成新提交并打 `protocol/v<n+1>`，CHANGELOG 追加一条 `kind: retire`，evals 随内容一起回退，指标表里旧版本的记录不动。不移动 tag。

### 6.5 eval 门 `autome protocol eval <dir> [--changed]`

| 层 | 内容 | 何时跑 | 成本 |
|---|---|---|---|
| 1 静态 | 契约区标记对数与哈希；协议总字节 ≤ 20KB；每条含"必须 / 不得 / 不要"的行能匹配到某条 CHANGELOG 的 `clause`；模板占位符齐全；模板必含短语表（从 launcher 单元测试迁来）；`behavioral` 条目有 `eval` 且目录存在 | Implement 验收、Audit | 秒级 |
| 2 示例可解析 | 协议正文里所有 ```text 示例状态块与里程碑表必须通过 `status_block::parse` | 同上 | 秒级 |
| 3 行为 | §3.6 的 case，真实 CLI，模型取当前配置，`runs: 3` 取多数，`max_turns` 默认 15 | 只在 Audit；`--changed` 只跑 CHANGELOG 新条目引用的 case + 三个基线 case | 每 case 约 3 × 15 turns |

v1 的"解析回归用真实文档"层删除：它测的是解析器不是协议文本，契约区哈希已覆盖。

### 6.6 目标函数与防跑偏

不做自动优化，不做自动回滚。版本页按版本列出 §3.2 各项的均值与 n，与 CHANGELOG 里每条的 `predicted_impact` / `realized_impact` 并排。

- 显示规则：n < 3 显示"样本不足"；对比只在同一项目内做，跨项目只列不比。
- 警示（不是判决）：`protocol_failures` 上升；`closed_then_contradicted` 上升；`manual_items_open` 上升。任一成立时版本页红标，并把该版本写进下次元任务的 `inputs/failures.md`。
- `retire` 类改动只能以移除实验形式提出：`predicted_impact.direction = flat`，到期若对应指标恶化，版本页提示恢复。

autoresearch 式棘轮不适用：样本每月个位数，指标多维，且难度不同的任务不可比。系统的职责是把证据摆整齐，判断留给人。

### 6.7 落地顺序

| 步 | 内容 | 依赖 | 主要改动 | 验证 |
|---|---|---|---|---|
| 1 | 协议出仓：`~/.autome/protocol/` 仓库、内置 Project 记录、`protocol_ref`、任务目录副本、prompt 模板化、`contract.toml`、CHANGELOG v1 预填 | 无 | `init.rs`（常量变种子）、`launcher.rs`（模板渲染）、`config.rs`（pin 字段）、`store.rs`（tasks 加 protocol_ref）、`scheduler.rs`（创建任务时复制） | 现有 544 + 115 测试不变；dry-run 记录的 prompt 与出仓前逐字节一致 |
| 2 | A 观测层 | 无 | Codex `--json`、包装脚本、`reap_session`、store v2、面板 | §4.A 的验证 |
| 3 | D 守卫 + G | 无 | `read_outcome` 之后、协议条文改写（走第 1 步的仓库，作为 protocol/v2） | 用 09-16 真实文档构造违反样本，各守卫命中 |
| 4 | E1 复盘轮 | 1 | `Node::Retro`、`Role::Retro`、转换表、RoleConfig ×6、界面路由图与角色配置 | 单元测试覆盖新转换行；真实任务终止后 `lessons.md` 存在且每条通过 §3.3 schema 校验 |
| 5 | eval 第 1、2 层 + F 的 case 移植 | 1 | `automed protocol eval` 子命令 | 改一处契约区被拒；删一条 CHANGELOG 引用的条文被拒 |
| 6 | B 简报 + 协议单份 + C 契约 | 1、2 | `start_session`、`brief-map.toml`、prompt 模板、协议文本 | §4.B 的同需求对比 |
| 7 | 元任务：`inputs/` 组装、触发提示、`需人工特批` 标记、门 2 的 diff 与 eval 报告展示、向前回滚 | 1、2、4、5 | `scheduler.rs`、面板 | 端到端跑一次元任务到门 2，人工核对 inputs 与真实数据一致 |
| 8 | eval 第 3 层 | 5 | 真实 CLI 跑 case | 三个基线 case 在当前协议上 3/3 通过 |
| 9 | E2 策展 + E3 移除实验 + 版本页对比与 `realized_impact` 回填 | 2、4、7 | 内核聚合、面板 | 两个任务后出现第一张"建议入规"卡片 |

第 1 步是唯一的架构改动，可以用 Loop 在 autome-v2 仓库上跑。第 2、3 步与它无依赖，可并行。

## 7. 不建议做的

- 会话复用（`claude -p --resume` / `codex exec resume`）：每 turn 背历史，第二轮即触发压缩；破坏裁决与审计独立性；替换掉可审查的文件记忆。
- 任何会话直接改 `.autome/`、协议仓库主分支或契约区。
- 自动接受或自动回滚协议版本。
- 用 Codex 的 tokens 自造价格表算费用；跨 runtime 只比 tokens。
- 让内核改写会话已提交的文档（D 表第二行的反面）。

## 8. 待用户决定

1. 第六个角色 `retro` 是否进角色配置页与路由图（方案按"是"写）。
2. Failed / Cancelled 任务的复盘是手动按钮（方案）还是自动。
3. 协议仓库放 `~/.autome/protocol/`（方案，按机器）还是放进每个项目的 `.autome/protocol/`（按仓库，团队共享但每个项目独立演进）。方案选按机器是因为 Loop 的教训跨项目才够样本。
4. 协议总字节上限 20KB 与设计文档警告阈值 80KB 是否合适。

## 9. 审计修订记录

对抗式审计 30 条，采纳 28 条，主要修正：

- 状态机：v1 同时声称"不新增状态机"与"Cleanup 前加复盘轮"。v2 明确 Retro 为新 Node + 第六 Role，并把"不新增状态机"限定在 §6。
- 协议按机器与按仓库：v1 用 tag 引用全局仓库，团队成员会解析到不同内容。v2 任务目录持有副本、按 hash 引用，tag 只是名字。
- 回滚：v1 移动默认 tag 会连 evals 一起回退且与指标表脱节。v2 回滚是向前的 revert 提交。
- 放水检测：v1 的"缺陷降且 reopen 降"会在协议真的变好时误报。v2 用 `closed_then_contradicted` 直接测量。
- 自指：v1 未处理评审轮判自己的 prompt。v2 元任务运行在当前 tag、评审六类重映射、评审 / 审计 prompt 改动需人工特批。
- 契约区：v1 的标记在被编辑的文件里，可被删。v2 期望哈希存内核，校验标记对数。
- eval 成本：v1 未设上限。v2 `max_turns` 默认 15，只断言前几步行为；`--changed` 只跑相关 case。
- 教训识别键、predicted_impact 结构、指标 schema：v1 全是自由文本。v2 见 §3。
- 退休：v1 以"没再出现"为据。v2 只允许移除实验。
- 依赖表：v1 "A 之前不动任何条目"与步骤表矛盾。v2 见 §5 与 §6.7 的依赖列。
- 证据文件名：审计轮与同 k 实现轮撞名，加角色后缀。
- 守卫不改写文档：实现轮标已完成判 `protocol_error`，不由内核回滚单元格。

未采纳 2 条：把复盘轮并入最后一次审计（会让复盘与审计同模型，且 Failed 任务没有最后一次审计）；把解析回归层保留（与契约区哈希重复）。

## 10. 落地情况（2026-09-17）

§6.7 的九步全部落地，加上界面。每一步一个提交，测试 659 → 929（Rust 807 + 桌面
122，含端到端 29 条与真 Electron DOM 51 条）。

| 步 | 提交 | 验证 |
|---|---|---|
| 1 协议出仓 | `协议出仓，规则变成带版本的数据` | 出仓前把六份 prompt 存成基准，出仓后逐字节一致 |
| 2 A 观测层 | `观测层——会话用量、任务指标、版本页` | 端到端：每个会话有非空 tokens 与 turns，Claude 有费用、Codex 没有 |
| 3 D 守卫 + G | `内核检查五条，协议不再靠自觉` | 端到端：造违反样本，两条硬守卫命中并停下任务，警告不停 |
| 4 E1 复盘轮 | `复盘轮进循环、进路由图……` | 端到端：Loop 尾部跑复盘轮，lessons.md 通过 schema；失败任务手动补一次 |
| 5 eval 1、2 层 | `eval 门的静态层与示例层` | 改一处契约区被拒；删一条被引用的条文被拒；`automed protocol eval` 退出码 0/1/2 |
| 6 B 简报 + C 契约 | `每轮简报、协议只留一份、里程碑验收契约` | 端到端：每个角色都有简报，审计轮拿到路径并被告知先别读 |
| 7 元任务 | `元任务——改协议是一个跑在协议仓库上的普通 Loop 任务` | 端到端：跑到 inputs 齐全，引用了真实任务的指标 |
| 8 eval 第 3 层 | `第 3 层——用例真跑一遍 CLI` | Runner 是 trait，多数表决 / turn 上限 / grader 输入都有不花钱的测试 |
| 9 E2 + E3 + 回填 | `教训变成规则，规则的移除是一次实验……` | 端到端：同一条教训两个任务 → 入规卡 → 批准 → 写进规则 → 移除实验 → 放回 |
| 界面 | `用量、协议版本页、建议入规` | 真 Electron DOM 断言 7 条 |

### 与方案不同的地方

1. **协议正文上限 20KB → 24KB。**（§8.4 本来就留给用户）20KB 是写方案时拍的
   数，当时没量过真正的正文——出仓前已经 19.3KB，两条有依据的新条文就破了。
   能拦住「协议开始膨胀」的阈值才有用。用户拍板提到 24KB。
2. **§G 的「包装脚本把超长命令输出截为头尾各 50 行」没有按原样做。** 包装脚本
   包的是 CLI，不是 CLI 里跑的每条命令；它能截的只有日志，而进模型上下文的是
   工具返回值，脚本碰不到。改成协议里的一条具体写法（重定向 + `tail`）。
3. **没有制造 protocol/v2。** 方案说第 3 步的协议改动作为 v2 发布。但种子在同
   一个未发布的二进制里，硬切一个版本边界是仪式而不是事实。新条文进 v1 的
   CHANGELOG，v2 会在第一次真的元任务跑完时出现。
4. **「每条祈使句都要有 CHANGELOG 出处」是警告不是拒绝。** 协议正文大半从 1.x
   继承，那几个月没有把理由写下来。回头要求每句话都有条目，结果要么永远红，
   要么被编出来的理由填满。现在它是一张工作清单，27 这个数字应该往下走。
5. **`max_turns` 事后核对。** 两个 CLI 都没有 turn 上限的参数（2026-09-17 对着
   Claude Code 2.1.261 与 Codex 0.153.4 查过），所以超了判用例失败而不是拦住。
6. **手动复盘不进状态机。**「读一遍这次运行」不改变任务站在哪里，而一个只为了
   能回来而存在的状态，本身就是一个状态。它在 reap 时单独处理。

### 顺带修掉的三个 bug

都不是这次改动引入的，是这次改动撞上的：

1. **任务号按项目编，主键却是全局的。** `tasks.id` 是主键，`sessions` 和
   `decisions` 都引用它，而编号从每个项目各自的 T-1 开始。第二个项目的第一个
   任务必然撞主键——注册协议仓库为项目时立刻发生。之前没发现是因为所有会建任务
   的代码路径都只用一个项目。编号改成全局。
2. **insert_task 把所有约束违反都报成「项目内已存在 slug」。** 上面那个主键
   冲突因此被报成重名问题，会把人引去找一个从来不存在的任务。按类型分开报。
3. **刚启动的会话会被判成「消失」。** `log_idle_secs` 把「日志文件还不存在」
   当成无限空闲，而内核记录会话到终端真正拉起包装脚本之间有一个窗口。机器一忙
   就会在任务还没跑时判它失败。会话的空闲时间现在以它自己的存在时长为上限。

### 还没有做的

- **第 3 层没有真的花钱跑过。** 十个用例 × 3 次 × 两次模型调用，需要用户决定
  什么时候跑第一次。管道有测试，`--changed` 与 `--behaviour` 都接好了。
- **`realized_impact` 还没有真实数据。** 它要等第一批任务在 v1 上跑完、第一次
  元任务发布 v2、再跑完 horizon 个任务。回填代码有测试，路径是通的。
