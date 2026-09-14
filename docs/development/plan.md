# Autome 2.0 研发方案：Rust 驱动的可证据化任务闭环

首次成稿：2026-09-13  
本次修订：2026-09-14  
状态：候选定稿，已补入 Project → Task、环境中心、技能市场与双 CLI，尚未开工  
首发目标：macOS 14+ / Apple Silicon  
核心技术：Rust Core + Electron Desktop + Codex/Claude CLI adapters  

> **权威边界**：Autome 2.0 是独立绿地产品。它不复用、不迁移、不兼容 Autome 1.x 的 Bash CLI、文件协议、插件模板、Dashboard、Electron 壳或运行时目录。1.x 只提供经过重新表述的失败案例与验收素材，不是源码、协议或架构依赖。

---

## 0. 一页结论

Autome 2.0 先管理 Project，再在选中的 Project 下接受自然语言 Task。Project 可以是“从零做产品”的新仓库，也可以是已有代码库；Task 则是一次需要被完整实现和证明的产品需求或功能迭代。系统必须完成四件事：

1. **准确分析**：读取用户原始要求、目标代码库、项目规则、源码、测试和必要的外部权威资料，区分事实、推断、假设和用户决定。
2. **完整拆分**：形成不可静默缩减的任务契约，将每项原始要求映射到实施节点和验收检查。
3. **受控实现**：在隔离工作区内按依赖顺序执行，允许基于新事实局部重规划，但不得删除要求、放宽验收或越过权限边界。
4. **事实验收**：Agent 只能声明完成；Rust 内核必须在最终候选版本和指定环境中亲自执行验收，生成事实收据，再由独立评估者审计。全部完成门成立后，系统才能签发完成证书。

核心完成公式（非规范摘要；唯一规范谓词见 §7，并由 Rust 测试锁定）：

```text
Completed =
  原始语义覆盖率 100%
  AND 必需要求覆盖率 100%
  AND 所有 mandatory AcceptanceCheck 在最终候选版本通过
  AND 每个通过结论都有有效 EvidenceReceipt
  AND ResolvedProjectConfig、步骤路由、AttemptPermissionProfile 与 SkillSetSnapshot 全部冻结且仍有效
  AND 最终独立 AuditVerdict = pass
  AND 没有未解决的重大假设、阻断问题、新增回归或越权变更
  AND 交付演练、用户批准、DeliveryReceipt 与只读交付树核对均绑定同一 tree
```

Autome 2.0 不承诺抽象意义上的“绝对正确”。它承诺一个可以被核对、可以被证伪、不会假完成的工程边界：**没有事实证据，就没有完成。**

### 0.1 现有实现评估：保留经验，不保留实现

2026-09-13 对当前 1.x 仓库的只读核对结论如下；它们只用于解释 2.0 的边界，不构成代码复用许可：

| 当前事实 | 已证明的价值 | 2.0 决策 |
|---|---|---|
| `plugins/loop/skills/loop-task/template.md` 用设计、评审/裁决、实现/审计分开角色，并含 6 处会话接力 | 阶段分离、独立复核和有界循环是正确方向 | 保留目标，删除 Markdown 自接力；Rust scheduler 独占转换权 |
| `bin/autome` 是 Bash CLI，运行态主要落在 `.autome/` 和任务文档 | 简单、透明、易于诊断 | 不迁移；改为 Rust 类型、SQLite 事务、显式 event/receipt |
| `autome-desktop/main.cjs` 明确把 UI 动作转交 CLI，并扫描任务文件形成状态 | 证明桌面工作台有价值 | 不复用壳或扫描逻辑；Electron 仅显示 Rust projection |
| 当前 `autome task` 路径包含 `--dangerously-skip-permissions` | 证明“能自动执行”不等于“可控” | 2.0 所有项目命令必须经过资格认证 sandbox，禁止 unrestricted fallback |
| 本轮 `bash skill/testing/run-gates.sh` 实测 T1–T4 廉价门禁 **318/318 通过** | 机械门禁、阳性对照和 mutation discipline 值得继承 | 只继承测试原则；2.0 重新建立 Rust/Electron T1–T6，不复制脚本 |

因此，2.0 不是把 Bash 改写成 Rust，也不是给现有 Dashboard 换壳；它从新的领域模型、威胁边界、证据模型和交付模型开始。

本轮只读环境核对也直接验证了为什么不能只做 `which`：当前机器同时存在 Homebrew 的 Codex 0.153.4 与 ChatGPT.app 内置的 0.154.0 alpha 候选；Claude Code 2.1.261 与 iTerm2 3.7.0 已安装；Homebrew 6.0.22 使用用户配置的镜像；`$HOME/.agents/skills`、`$HOME/.codex/skills`、`$HOME/.claude/skills` 均有内容，而用户提到的 `.agent/skill` 不存在。这些只作为 Environment Center fixture，不写成产品常量。

---

## 1. 已确认的硬边界

### D1　从零建设，不从 1.x 演进

- 2.0 必须在物理独立的新 Git 仓库中开发；不使用 1.x 仓库的普通分支、worktree 或 orphan branch。
- 构建、测试和发布环境中不得依赖任何 1.x 路径、符号链接、脚本、数据文件或生成产物。
- 不设计 1.x 数据迁移、兼容模式、降级模式或双运行时。
- 允许继承的只有本方案、公开技术资料、重新表述的失败案例和事实验收；禁止复制旧源码、协议模板与配置。初始文件与关键依赖记录 provenance，独立 reviewer 做来源审计。

### D2　Rust Core 是唯一业务与状态权威

Rust Core 独占：

- Project、ProjectIntentRevision、ProjectIntentAmendment、ProjectInitializationReceipt、ProjectTargetTransitionReceipt、ExecutionQueue、HarnessLease、GlobalConfigRevision、GlobalConfigImpactPreview、ProjectConfigPatch、ResolvedProjectConfig、StepExecutionRoute、PlanningRunSpec、PlanApprovalReceipt、ExecutionRunSpec、PlanningPolicyRestart、RunPolicyAmendment、ReplanApprovalReceipt、ContractAmendment、BudgetGrantReceipt、UserCorrectionReceipt、CarriedPlanningReviewBundle、HumanReviewReceipt、HumanReviewFinding、SafeParkReceipt、SkillInventory、SkillPackageSnapshot、SkillAuditReceipt、SkillInstallPlan、SkillInstallationTransaction、SkillInstallReceipt、SkillBindingPlan、SkillBindingReceipt、GlobalSkillBinding、ProjectSkillBinding、SkillSetSnapshot、SignedDependencyCatalog、EnvironmentSnapshot、EnvironmentRemediationPlan、UserActionChallenge、EnvironmentInstallationTransaction、EnvironmentChangeReceipt、CredentialRecord/Receipt，以及 TaskContract、Requirement、ApplicableProjectRuleSnapshot、TaskGraph、Run、Attempt、AttemptPermissionProfile、Gate、FactReceipt、EvidenceReceipt、ExecutableOracleSnapshot、TestInventorySnapshot、AuditVerdict、SupportedEnvironmentProfile、ReadinessReceipt、HarnessCapabilitySnapshot、ModelSelectionIdentity、QualificationReceipt、CandidateCertificate、DeliveryRehearsalReceipt、DeliveryApprovalReceipt、DeliveryReceipt、DeliveredTreeCheckReceipt、CompletionCertificate；
- 状态机与合法转换；
- SQLite 持久化、事件账本和恢复；
- Harness 调度、进程管理、预算、超时与取消；
- disposable clone、验证环境、权限和 capability broker；
- 证据签发和完成裁决。

Electron、Renderer、Harness 和 Agent 均无权直接修改业务状态或签发完成结论。

### D3　Electron 是唯一用户界面，但不是第二个内核

- Electron Renderer 只展示 Rust 投影并提交用户命令。
- Preload 只暴露按业务领域命名的有限 API。
- Electron Main 只负责窗口、通知、单实例、IPC 校验和 Rust sidecar 生命周期。
- Electron 不读业务数据库、不解析 Git/日志推断状态、不计算进度、不决定下一角色或完成。
- Renderer 不直连 Rust、不开放 localhost HTTP/SSE 控制面。
- 绿地候选预览使用与主 Renderer 完全分离、无 preload/IPC 的一次性 PreviewWindow（§9.6）；候选页面永远不能进入主工作台 origin 或 Electron 业务 IPC。

### D4　完成声明与完成事实分离

- Agent 的 `complete_task` 只产生 `AgentClaim`，表示本次会话愿意结束。
- Harness 正常退出只说明进程结束，不说明任务成功。
- 只有 Rust verifier 可以产生 `EvidenceReceipt`。
- 只有 Rust completion gate 可以在同一事务中产生 `RunCompleted`、对应的 `TaskCompleted` 与 `CompletionCertificate`。

### D5　任务原文不可覆盖，契约只能版本化修订

- 用户原始消息和附件按原文与内容哈希保存。
- TaskContract 冻结后不可原地修改。
- 语义变更使用 append-only `ContractAmendment` 产生新版本。
- 删除要求、降低环境等级、接受功能缺失或放宽验收必须得到用户明确决定。

### D6　生产者与评估者机械隔离

- producer 与 evaluator 必须使用不同且当前有效的 `model_choice_key_hash`；该 key 只由 provider + resolved model identity + 可见 snapshot/fingerprint 组成，换 CLI binary、账号、Effort、权限、配置或 Skill 都不能伪装成换模型。2.0.0 能证明的是两个模型选择键分别通过资格测试，不声称同一 Provider 内部 lineage 必然不同。
- evaluator 使用独立、只读工作区和新上下文。
- evaluator 默认只看用户原文、冻结契约、最终 diff 与事实收据，不读取生产者的说服性总结。
- evaluator 不能修改业务代码、契约、验收或证据；发现缺口后由内核创建新的生产/验证节点。
- 不满足隔离策略时状态为 `Blocked`，不得静默降级。
- 模型差异只用于降低自产自评的利益冲突，不构成正确性 oracle；正确性仍来自独立证据路径、黑盒检查和必要的人类判断。

### D7　2.0.0 先做单任务、顺序执行

2.0.0 不以并发作为卖点。全局一次只允许一个可运行 Task 持有 HarnessLease；其它可运行 Task 进入持久化 FIFO `ExecutionQueue`，按 `enqueued_event_seq + task_id` 确定顺序，只能查看、取消或等待，不支持隐式优先级。等待用户决定、Paused 或 Blocked 的 Task 只有取得 SafeParkReceipt、证明无 active tool/verifier 且 provider session 已安全中断/可精确恢复后才释放 HarnessLease；条件恢复后以新 enqueue event 回到队尾，避免一个人工卡点冻结所有 Project。Task 投影必须保存 `dispatch_state(queued|running|waiting|none)`、queue position 与 `blocked_by_task_id?`，取消幂等且不能留下 lease。一个 Run 内的 TaskGraph 也按依赖顺序执行。全局同时只允许一个 Environment 或 Skill 写事务；它若会改变活动 Run 引用的 CLI、模型、工具链或 Skill，必须等待安全停靠。只有单任务闭环达到发布门后，2.1 才评估任何同时持有多个 HarnessLease 或节点并行。

### D8　2.0.0 资格认证 Codex 与 Claude Code 两个 CLI Harness

- `codex` 通过 `codex app-server` v2 JSON-RPC 接入；`claude` 通过 `--print --input-format stream-json --output-format stream-json` 接入，并用每 Attempt 的唯一 session ID 管理 resume/interrupt。
- 两者共享同一个 Rust `HarnessAdapter` 生命周期和同一组 Autome Control Tools：Codex 使用 client-executed dynamic tools；Claude 使用只包含 Autome 工具的临时 stdio MCP 配置。传输不同，领域语义、权限和完成门相同。
- 每个 AI 步骤通过 StepExecutionRoute 明确选择 `codex | claude`、模型和 provider-native Effort；选择的组合未安装、未登录、不可用或未通过资格测试时进入 Blocked，绝不静默换 CLI、模型或 Effort。
- Rust 领域层保持 Harness-neutral；2.0.0 只实现这两个 adapter，不提供第三 Harness、任意 Provider plugin 或运行期 adapter 下载。
- 两个 adapter 都不得以 stdout 文案、PID 或“最新会话”推断状态。Provider、CLI、协议、模型、Effort、权限策略与 skill snapshot 写入每次 Attempt 和发布清单。
- 两个 executable 都必须由用户显式选择或从已资格认证安装位置发现；Core 记录 canonical path、binary digest、签名、架构、版本和协议/capability hash，运行中任一变化都会撤销 Harness readiness，不能每轮从可变 `PATH` 重新解析。
- Autome 不随安装包捆绑 Codex CLI 或 Claude Code CLI；Environment Center 负责检测、展示官方安装渠道并通过受控安装事务补齐。
- Codex 使用 owner-only 的独立 `CODEX_HOME`，固定 `cli_auth_credentials_store="file"`，通过 app-server account API 管理 device-code 登录；认证只能写入该根下身份已核对的 `auth.json`。启动与资格测试必须同时核对 effective store、文件 owner/mode/no-follow identity 和 account identity；若 admin/MDM 强制 keyring/auto、store 无法证明隔离或身份漂移，Codex route 直接 Blocked。Claude 的 `CLAUDE_CONFIG_DIR` 只用于隔离设置、插件和会话，**不能隔离 macOS Keychain OAuth 身份**；而安全执行所需的 `--bare` 明确不读取 OAuth/Keychain。因此 `shared_macos_keychain` 在 2.0.0 只做状态监测和“在外部终端打开 Claude”的便利能力，不能成为 StepExecutionRoute。自动 Run 的 Claude route 只接受 `dedicated_api_key`：用户把专用 API key 存入 Autome Keychain item，经固定、签名、受权限保护的 `apiKeyHelper`/等价 handoff 传给 Claude 自身请求层，并须经 S0 证明不会进入 Agent、Bash/项目子进程、argv、env、日志和错误；若做不到，Claude adapter 不能发布。Autome 永不对 shared 身份调用 `claude auth login/logout`；Codex 独立账号才允许应用内 login/cancel/logout。

### D9　2.0.0 为本地、单用户、macOS 首发

- 首发只支持 macOS 14+ Apple Silicon。
- 不做多人协作、云端控制面、远程操作、Windows、Linux、Intel Mac。
- 用户关闭窗口时应用进入后台，Rust Core 和任务继续；用户明确退出时必须先安全停靠。

### D10　自我改进不是首发范围

2.0.0 只采集结构化运行事实和人工反馈，不让 Agent 自动修改 Prompt、已批准 Skill、策略或内核。用户主动安装、升级、启停或回滚 Skill 属于受审计配置操作，不算自我改进；Agent 只能提出变更。自动自我改进必须等到跨任务 golden corpus 足够后，以独立项目进入 2.1+。

### D11　首次候选代码写入前必须确认执行契约

- Task 创建时，Core 先从当时的 ResolvedProjectConfig 与 Skill binding 生成不可变 `PlanningRunSpec`，只覆盖 `fact_analysis`、`contract_drafting`、`contract_review`、`task_graph_planning`、`graph_review` 五个只读步骤及其 AttemptPermissionProfile；其 `scope=planning` ReadinessReceipt 必须证明这些 CLI route、只读 sandbox 与 Skill 投影可用，analyst、planner 与独立 contract reviewer 才能据此完成事实、TaskContract 与 TaskGraph 候选。
- Core 再对契约选定的完整环境画像生成 `scope=execution` ReadinessReceipt；项目工具链或执行 route 不就绪时在任何候选代码写入前阻塞，但不把“尚待分析才能确定的项目依赖”倒过来阻止只读规划。
- 用户在一个界面中确认原始要求覆盖、重大假设、非目标、验收方式，以及候选 `ExecutionRunSpec` 中每一步的 CLI installation/version/source、provider/account fingerprint/auth mode、model/Effort/HumanReview、SkillSetSnapshot、环境能力、预算与执行范围；展示摘要 hash 进入 PlanApprovalReceipt。
- 该批准将 TaskContract、TaskGraph 与 `ExecutionRunSpec` 原子冻结并挂到同一 Run；批准前 Harness 只有 PlanningRunSpec 声明的只读分析权限，不能修改目标代码。若规划期间的 config、Skill binding、CLI identity 或权限发生变化，当前 Run 必须 Supersede 并从原始任务创建新的 PlanningRunSpec，不能热改或把旧文档直接提升为执行契约。
- 用户确认不是正确性的替代品：系统仍必须完成独立验证与最终审计。

### D12　环境能力必须先被证明，不能靠 Agent 猜测

- 每个 Run 必须绑定版本化 `SupportedEnvironmentProfile` 与当前 `ReadinessReceipt`；缺少工具、版本不兼容、依赖不完整或 sandbox 不支持时进入 `Blocked(EnvironmentNotReady)`。
- Environment Center 在应用启动、项目切换、配置保存、CLI/Skill 文件变化和 Run 开始前更新 EnvironmentSnapshot；至少检查 iTerm2、Xcode Command Line Tools/Git、Homebrew、Node/npm、Codex CLI、Claude Code CLI、Codex 独立认证、Claude 已选认证模式、受支持 sandbox，以及原生 Skill 目录和用户要求监测的 `$HOME/.agent/skill` / `<repo>/.agent/skill` legacy roots。
- “一键补齐”只处理 Autome 控制面依赖与用户明确选择的项目 profile；它先生成可预览的 EnvironmentRemediationPlan，再由用户一次确认。系统/Keychain/登录弹窗仍由 macOS 或对应 CLI 单独确认，不能绕过管理员权限或账号登录。
- 2.0.0 不承诺任意技术栈。绿地首版只资格认证一个 `greenfield-web-v1` host profile；现有仓库也只在发布清单列出的 host profile 上执行。
- 不得因为宿主机“碰巧能运行”就跳过资格判断。任务执行期间禁止隐式安装；所有下载、安装、升级、迁移和卸载只能由 Environment/Skill broker 的独立事务执行并产生 Receipt。

### D13　明确本地数据保护边界

- 控制数据根、ProjectHome、Run、Skill Vault/quarantine、安装日志、截图、备份以及 Codex/Claude 隔离配置根均由 Core 创建并验证 owner、权限和 symlink 边界。
- 2.0.0 不自建数据库字段加密、密钥轮换或跨机密钥恢复；它依赖 macOS 账户隔离与 FileVault 提供静态磁盘加密，启动时探测并明确显示 FileVault 状态，未启用时不得声称“静态数据已加密”。
- 该边界不抵御已经控制当前解锁用户会话的恶意进程；通过最小保留、脱敏导出和明确删除降低暴露面。Autome Keychain item 只存 Claude dedicated API、研究 provider 等明确采用 broker 的外部凭证，不作为任务数据库加密系统；Codex OAuth 固定在专用 `CODEX_HOME/auth.json`，不与这些 item 混用。

### D14　管理层级固定为 Project → Task

- Task 创建前必须先选择并完成 ProjectInitialization；不存在“脱离项目的临时 Task”。
- 每个 Project 必须有用户批准的 ProjectIntentRevision；它保存产品目标、目标用户、跨 Task 约束、明确非目标与关键决定。每个 TaskContract 固定其 hash；新 Task 与当前意图冲突时先提出 ProjectIntentAmendment 并展示对其它活动 Task 的影响，用户批准新 revision 后才能继续，不能让后一个 Task 静默推翻前一个产品决定。
- Project 分为 `new_product` 与 `existing_repository`。前者绑定仍不存在的目标目录，首个任务交付后转换为已有 Git 仓库；后者登记当前 repository identity。TaskContract 仍固定精确 target snapshot，不能只信 Project 的当前路径。
- “在 Project 下初始化”指 Core 在 macOS Application Support 的 `projects/<project-id>/` 创建 owner-only ProjectHome，并初始化 manifest、config override、skill lock、任务索引与检查缓存；运行记录仍归 SQLite。默认不向用户仓库写隐藏目录、不修改 `.gitignore`，避免初始化动作污染工作树或破坏绿地 destination-absent proof。
- 如需随仓库共享配置，用户可显式导出无秘密的 `autome.project.toml`；它只是下一次导入的候选配置，不是活动 Run 权威，写入仍走普通候选/交付和审计流程。
- 全局设置提供默认值，ProjectConfigPatch 只记录项目覆盖项；UI 必须逐字段显示“继承/已覆盖”，支持一键恢复继承。Task 创建时冻结 PlanningRunSpec，契约批准时冻结绑定 ResolvedProjectConfig hash 的 ExecutionRunSpec；后续全局或项目设置变化只影响未来 spec。活动规划 Run 只能通过 PlanningPolicyRestart 采用，活动执行 Run 只能通过 RunPolicyAmendment 创建新 Run 后采用，绝不热改。

### D15　技能市场是受控供应链，不是直接执行入口

- Skills 页面接入 `find-skills`/skills.sh 搜索与安装流程，同时盘点本机和 Project 的原生 Skill 目录。
- 市场 Skill 先下载到 quarantine，固定 source/ref/commit/content digest，完成结构、脚本、依赖、权限、prompt-injection、license 和兼容性审查；搜索热度、安装量和作者名只用于排序，不能作为安全证明。
- 通过审核并经第一次用户确认后，Skill 只安装到 Autome 内容寻址 Skill Vault，默认 disabled；第二个独立 SkillBindingPlan/用户决定才允许 Global/ProjectSkillBinding 指定哪些 Project、步骤和 CLI 启用。每个 Run 只加载不可变 SkillSetSnapshot，不直接执行可变市场目录或用户全局目录。
- 安装/更新字节与启停/切换 binding 分属独立、可恢复事务；安装成功绝不能自动启用。Vault/binding 切换可回滚，外部目录迁移则明确标注 rollback class。2.0.0 对 Skill 自带 hooks、第三方 MCP、plugin、直接凭证读取固定 hard-unsupported，审计发现即不能绑定。网络或额外 binary 只有在基础 StepPolicy 本来就允许时才可共享使用；Skill 永远不能扩大权限。

---

## 2. 产品范围

### 2.1 支持的 Project 与 Task

首版先创建或导入 Project，再从项目页创建 Task：

| Project 类型 | 初始化输入 | Task 路线 | 目标终态 |
|---|---|---|---|
| `new_product` | 项目名 + 尚不存在的目标目录 | 首个 Task 使用 `greenfield_product` playbook | 原子交付新 Git 产品后，Project 转为 `existing_repository` |
| `existing_repository` | 项目名 + 已有本地 Git 仓库 | Task 使用 `existing_repo_change` playbook | 在专用分支形成完整增量且不新增回归 |

Task 输入始终是“一句话需求 + 当前 Project 上下文”，不能再临时选择其它 workspace。“完成”由 TaskContract 决定：用户没有要求安装包、部署或生产验证时，系统不得擅自添加；用户明确要求时，对应环境检查必须进入 mandatory AcceptanceCheck。

### 2.2 2.0.0 非目标

- 不兼容或导入 1.x 任务与运行记录。
- 不支持非 Git 的现有代码库；绿地任务由 Core 创建新 Git 仓库和初始基线。
- 不把未知或恶意仓库当作已隔离运行环境。2.0.0 只执行用户明确标记为受信的本地仓库；未受信仓库只能做不执行项目代码的静态分析。
- 不执行生产发布、数据库迁移、付费业务动作或任意业务系统写入；这些操作首版只能形成待人工执行的计划。允许的本机管理写入仅包括经用户批准的 Project 初始化、Environment 安装事务、Skill 事务以及候选的专用 Git ref、绿地目录与制品交付。
- 不做任意技术栈承诺；以 golden task 覆盖的栈为已验证边界。
- 不做任务间并行、多项目并行或分布式调度。
- 不做自动模型竞价、学习型路由和 Codex/Claude 之外的第三 Harness。
- 不自动修改用户当前 checkout 或已有分支；已有仓库的 2.0.0 交付目标是新建的专用本地分支，后续合并由用户自行决定。
- 不在 Run 内隐式安装工具或依赖；只有用户从 Environment Center 发起的独立受控事务可以补齐已支持组件。
- 不把 LLM 自评分作为通过依据。

如果原始 must 要求需要 2.0.0 deny-list 中的生产发布、迁移、付费或外部写入，系统必须停在契约阶段请求 ContractAmendment。只有用户明确把交付物改为“可人工执行且已验证的计划”后，计划本身才能成为新契约的 mandatory Check；用户不接受缩小目标时，本 Run 进入 `Infeasible`，不得用 plan-only 结果替代原要求。

---

## 3. 总体架构

```text
┌──────────────────────── Electron Renderer ────────────────────────┐
│ Projects · Tasks · Environment · Skills · Config · Evidence       │
│ 无 Node / 无文件系统 / 无 shell / 无业务状态机                    │
└──────────────────────────────┬────────────────────────────────────┘
                               │ contextBridge 白名单 API
┌──────────────── Electron Main + Preload ──────────────────────────┐
│ 窗口/托盘/通知 · sender 校验 · Rust binary 校验与生命周期          │
│ 不读业务 DB · 不判定完成                                           │
└──────────────────────────────┬────────────────────────────────────┘
                               │ framed JSON-RPC 2.0 over stdio
┌──────────────────────────── Rust Core ─────────────────────────────┐
│ Project/Task Service · State Machine · Config Resolver · Scheduler │
│ SQLite/Event Journal · Workspace/Git · Verifier · Evidence         │
│ Harness Manager · Environment/Skill/Capability/Research Brokers    │
│ Skill Vault · Environment Readiness · Recovery                     │
└───────────────────────┬──────────────────────┬──────────────────────┘
                        │                      │
          JSON-RPC v2   │                      │ stream-json + stdio MCP
              ┌─────────▼─────────┐  ┌─────────▼──────────┐
              │ Codex App Server  │  │ Claude Code CLI    │
              │ dynamic tools     │  │ Autome tools only  │
              └─────────┬─────────┘  └─────────┬──────────┘
                        └──────────┬────────────┘
                         ┌────────▼─────────┐
                         │ disposable clone │
                         │ skill snapshot   │
                         │ scratch/candidate│
                         └──────────────────┘
```

发布资格是另一条信任链：独立 `ReleaseEvalOrchestrator + OracleController` 位于产品仓库、Electron 和 Rust Core 之外，把整个待发布应用视为不可信黑盒。它向应用投递 sealed task、收集应用产物/证书，再独立运行 ProtectedOracle；oracle 结论在两次 pass² Run 全部结束前不返回产品 Core 或实现团队。

### 3.1 为什么不使用 HTTP 控制面

Electron Main 与 Rust Core 使用 4-byte 长度前缀的 JSON-RPC 2.0 stdio：

- stdout 只传协议帧，stderr 只传 Core 诊断日志；
- 不产生监听端口、CORS、CSRF、DNS rebinding 或本机其它进程访问面；
- Main 是唯一具有完整业务命令面的 Core 客户端，Renderer 不能绕过 preload；
- 大日志、diff 与制品通过分页引用读取，不塞入事件流。

只有未来明确要求“用户真正退出 Electron 后任务继续运行”时，才升级为独立 service，并使用 Unix Domain Socket / Windows Named Pipe；仍不开放 localhost HTTP。

M4 的限时 loopback `PreviewSession` 是候选产品的数据面，不是 Core 控制面：它只能由 `preview_id` 打开，只服务一个隔离候选，不能读取或提交任何 Autome 命令。

### 3.2 事件与重连

所有命令必须包含：

```text
request_id · command_id · expected_revision · protocol_version · method · params
```

所有事件必须包含：

```text
event_seq · event_id · aggregate_id · aggregate_revision · event_type · occurred_at · payload
```

Renderer 重载：先取带 `snapshot_seq` 的快照，再从 `snapshot_seq + 1` 订阅。遇到序号缺口，废弃局部缓存并重新同步；命令响应丢失时按 `command_id` 查询，不盲目重发。

协议必须配置最大帧、最大队列、日志速率、单任务磁盘配额和订阅积压上限；超过上限时丢弃的是可重建的增量展示并触发 resync，绝不丢失状态事件或让不可信 Agent 输出无限占用 Main/Core 内存与磁盘。

---

## 4. Rust 工程结构

2.0 源码仓库只保留两个 Rust crate，避免为单一实现提前拆出 store/harness/verifier/ipc 抽象包：

```text
autome-v2/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  crates/
    autome-domain/       纯领域模型、状态 reducer、完成谓词
    automed/             application service、SQLite、调度、IPC、恢复，
                         内含 runtime/harness/verifier/policy 模块
  apps/desktop/
    src/main/            Electron Main
    src/preload/         白名单 IPC
    src/renderer/        React 工作台
  contracts/             Rust 生成的 JSON Schema 与 TS bindings
  profiles/              host profile、CLI capability 与步骤路由 schema
  install-recipes/       签名、版本化的 Environment 安装配方
  skill-policies/        Skill 扫描与兼容策略，不包含市场 Skill 本体
  playbooks/
    greenfield-product/
    existing-repo-change/
  tests/
    fixtures/
    golden/
    recordings/
  docs/
    adr/
    development/
```

### 4.1 依赖原则

- Rust edition 2024；M0 以当前已安装 `rustc 1.93.1` 为初始工具链并在 `rust-toolchain.toml` 固定。
- Tokio 提供异步进程、IPC、超时和任务调度。
- `serde` / `serde_json` 负责协议；`schemars` 由 Rust 类型生成 JSON Schema，再生成 TypeScript bindings。
- `rusqlite` 使用 bundled SQLite；单写者 DB actor 持有连接，所有状态迁移在事务内完成。
- `tracing` 输出结构化日志；`thiserror` 定义可序列化错误域；`sha2` 绑定契约、候选树和证据。
- Git 操作调用经探测的系统 `git` 稳定机器接口，使用 argv 数组和 `shell=false`；控制路径禁用 system/global config、credential helper 与项目 hooks，不解析面向人的彩色输出。
- 应用依赖全部进入 lockfile；RC 使用 `cargo test --locked`，离线资格测试使用已预取依赖。
- Electron 自带的 Node 只用于桌面应用自身；目标项目和 Skills CLI 不得借用 Main/Renderer 的 Node。`greenfield-web-v1` 所需 host Node、包管理器和浏览器的精确范围在 S0 后冻结并由 ReadinessReceipt 核对。
- 现有仓库使用宿主工具链前必须匹配发布清单中的 profile。缺失组件可在 Run 外由用户启动 EnvironmentRemediationPlan；包管理器安装、浏览器下载和依赖预取永远不是任务执行的隐式步骤。
- 安装配方只包含版本化的程序、固定 argv、允许来源、预期签名/Team ID、探测和撤销策略；不得把市场返回文本拼成 shell，也不得直接执行未检查的 `curl | sh`。Homebrew 的 origin/API/bottle/core 镜像也进入 EnvironmentSnapshot；非默认但 catalog 允许的镜像必须展示并确认，未列入 allowlist 的来源直接 Blocked。Autome 不改写用户的包管理器配置。

### 4.2 关键 Rust 接口

`HarnessAdapter` 必须覆盖完整生命周期：

```text
probe(InstallationIdentity) -> HarnessCapabilitySnapshot
list_model_efforts(CapabilitySnapshot) -> ModelEffortMatrix
auth_status(AuthMode) -> AuthState
start(AttemptSpec) -> SessionHandle
events(SessionHandle, cursor) -> EventStream
respond(SessionHandle, PendingRequest, Response)
interrupt(SessionHandle, reason)
resume(ProviderSessionId) -> SessionHandle
await_terminal(SessionHandle) -> TerminalResult
usage(SessionHandle) -> UsageSnapshot
```

禁止：

- 用 PID 作为唯一会话身份；
- 使用 `resume --last`；
- 从自然语言最终回复解析 verdict；
- 因未知事件崩溃或静默丢弃。未知事件必须保留原始 payload、标记 unsupported，并根据影响 fail closed。

#### 4.2.1 Codex adapter

所有 analyst、planner、implementer、contract-reviewer 和 auditor turn 固定 `approvalPolicy=never`，并按 app-server schema 二选一显式传入冻结的 sandbox policy 或已资格认证 permission profile；不得继承用户默认值。app-server 发来的 `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/permissions/requestApproval` 以及未来未知的权限升级请求一律由 adapter 自动 `deny`，不展示为可批准 Gate。只有 Autome 的 `request_decision` dynamic tool 可以请求业务决定，其返回值不能改变文件系统、网络、进程或 session 级权限。Attempt 与 QualificationReceipt 必须记录 approval policy、实际选择的 sandbox/profile hash 和未选择项为空的事实。

每个 Attempt 的专用 Codex config 还必须固定 `cli_auth_credentials_store="file"`、把 clone 标为 `untrusted` 以跳过项目 `.codex/` config/hooks/rules，并用 `model_instructions_file` 指向 Core 生成的只读指令文件；该文件只包含签名 role prompt 与冻结 ApplicableProjectRuleSnapshot，不从 candidate 重新读取 `AGENTS.md`。启动后 adapter 枚举 effective config、instruction、hook、plugin、MCP 与 rule 来源，结果必须精确等于 core-manifest allowlist。任何无法关闭、无法枚举或由 admin/MDM 强制加入的额外 hook/规则/指令都会使该 installation 对自动 Run 不合格；独立 evaluator 尤其不能因 producer 修改 candidate 中的 `AGENTS.md` 或 `.codex/` 而改变控制上下文。

入站 `ServerRequest` 采用穷举 allowlist：任务运行期只接受与本 Attempt 注册表逐项匹配的 `item/tool/call`；managed ChatGPT auth 不接受客户端 token refresh 请求。`item/tool/requestUserInput`、`mcpServer/elicitation/request`、旧式 `applyPatchApproval`/`execCommandApproval`、所有新旧审批以及未知 request 都立即返回结构化 deny/unsupported，记录原始 payload 后按影响中断 Attempt，不能悬挂，也不能转成用户 Gate。每次 Codex schema 变化都做枚举 diff；新增 request 在人工分类和资格测试前默认拒绝。

#### 4.2.2 Claude Code adapter

Claude 以固定 argv 启动 `claude -p`，使用双向 stream-json、显式 UUID session、`--model`、`--effort`、`--bare`、`--no-chrome`、`--permission-prompts none`、`--permission-mode dontAsk`、按角色精确的 `--tools` + `--allowedTools`、`--strict-mcp-config` 和唯一 Autome stdio MCP。read-only 角色根本不获得 Edit/Write；implementer/repair 只预批准 clone 内 Read/Edit/Write 与已资格 sandbox Bash。生成的 settings 强制 permissions allow/deny、`sandbox.enabled=true`、`sandbox.failIfUnavailable=true`、`sandbox.allowUnsandboxedCommands=false`、空 `excludedCommands` 和明确 deny-read/write；外层 sandbox 仍限制整个 CLI 进程到 clone/scratch/SkillSetSnapshot。

Claude `-p` 可能静默忽略无效 settings，因此 Core 必须先用当前版本 schema 本地校验生成文件，再从 init/diagnostic 事件核对实际 permission/sandbox/tool/config digest；任一不匹配立即中断，不能靠“进程启动成功”推定策略生效。

`--bare` 用于禁止用户/项目 hooks、skills、plugins、MCP、memory 和命令自动发现；本 Run 已批准的 SkillSetSnapshot 通过只读 `--add-dir` 投影加载。CLI 的 AskUserQuestion、permission prompt、WebFetch/Chrome、动态 MCP 和 `dangerouslyDisableSandbox` 均不可用；业务提问只能走 Autome MCP 的 `request_decision`。任何工具或子进程不能证明处于双层限制内时，该 Claude installation 显示为“已安装，未资格认证”，不能被 StepExecutionRoute 选择。

### 4.3 Autome Control Tools

Rust 为每个 Attempt 注册同语义的限作用域工具；Codex adapter 映射为 client-executed dynamic tools，Claude adapter 映射为唯一的临时 stdio MCP：

```text
get_project_context
read_resolved_config
propose_project_change
propose_config_change
get_task_context
get_environment_status
list_enabled_skills
search_skills
propose_environment_change
propose_skill_change
report_progress
request_decision
report_blocker
complete_task
```

读工具只返回当前 Project/Run 冻结快照；Task Attempt 不存在列举其它 Project 的接口，project_id/run_id 在建 Attempt 时固化且不能由请求体覆盖。proposal 工具只能生成当前 Project 下待用户确认的方案，不能直接安装或改配置。每个调用由 Rust 校验 project/run/attempt ID、provider session ID、tool call ID、active lease、输入 schema、大小上限和方法白名单。`complete_task` 只写入 AgentClaim；工具集中不存在 `mark_completed`、`approve_gate`、`apply_config`、`install_skill`、`repair_environment`、`write_evidence` 或任意状态转换能力。S0 必须分别证明 dynamic-tool 与 stdio-MCP 的请求/响应、取消、重复事件和 resume 语义稳定；任一 adapter 失败就不能成为可选路由。Project 列表只由本地 Electron 通过独立 Core query 获取，不发送给模型。

### 4.4 外部事实通道

需要当前外部资料时，Agent 不能用模型记忆或搜索摘要冒充事实。2.0.0 默认关闭 Codex 原生 `web_search`：当前 app-server 只暴露事后 action，不能保证 Core 在请求发出前审查 query/URL，因此它不能与含用户原文、附件或仓库内容的任务上下文并用。

Rust 只为允许研究的只读角色注册两个 client-executed dynamic tools：

1. `research_search(query)`：Core 在请求发出前做 secret/DLP、高熵片段、代码块、路径、长度、域名与数据策略检查，再调用 S0 资格认证的单一只读搜索 provider；结果只是带 opaque `source_id` 的候选 URL，不能直接进入 TaskContract；
2. `research_fetch(source_id)`：只接受搜索结果或用户提供 URL 生成的 source ID，不接受 Agent 任意拼接的 raw URL；执行 HTTPS GET，不发送 cookie、凭证或自定义认证头，逐跳拒绝 loopback/private/link-local 地址、DNS 重绑定和跨策略重定向，并限制域名、响应体、MIME、时间与频率。

只有 `research_fetch` 实际捕获的正文才能生成可引用的 `FactReceipt { request, status, redirect_chain, final_url, fetched_at, source_version_or_etag?, mime, snapshot_ref, body_hash, extraction_version, cited_ranges, freshness_policy, policy_hash }`。snapshot 是权限保护、内容寻址的只读对象；正文按不可信内容处理。最终审计前，所有 mandatory fact dependency 必须按 freshness policy 重验：带不可变版本的资料绑定 digest；其余外部资料默认 TTL 为 24 小时，契约可设得更短，过期后重新抓取。内容改变会使依赖它的契约、Check 和证据失效，并进入 ContractAmendment 或重新验证。

S0 无法资格认证受控 search provider 时，2.0.0 只允许用户提供 URL；缺少来源的任务进入 `Blocked(MissingAuthoritativeSource)`，不得回退到无来源结论。候选代码、项目脚本和测试始终默认无网络，也不能调用研究通道。

本次调研在本机 Codex CLI 0.153.4 生成的 app-server schema 中，同时观察到 sandboxed `command/exec`、明确不受 sandbox 保护的 `process/spawn`/`thread/shell_command`，以及可以申请扩大权限的三类 approval request。Autome 只允许带显式 sandbox 的前者；每个受支持版本都重新生成并审计 schema，其余入口、权限升级请求以及任意新增的 unrestricted 能力必须在 adapter 层硬拒绝并由负向测试锁定。

---

## 5. 核心领域模型

### 5.1 Project、Task 与配置继承

```text
Project
  id · display_name · kind(new_product|existing_repository)
  locator = GreenfieldDestination { parent_identity, destination }
            | ExistingRepository { repository_identity }
  lifecycle(active|archived) · project_revision
  project_home · initialization_receipt_id
  active_intent_revision · intent_hash
  active_config_override_revision · skill_binding_revision

ProjectIntentRevision
  project_id · revision · source_anchors[] · approved_by/at
  product_goal · target_users[] · durable_cross_task_constraints[]
  explicit_non_goals[] · key_decisions[] { id, statement, rationale, source_ref }
  supersedes? · intent_hash

ProjectIntentAmendment
  project_id · from/to revisions · trigger_task?
  semantic_diff · affected_active_tasks[] · user_decision_receipt · amendment_hash

ProjectInitializationReceipt
  project_id · project_revision · subject_identity_hash
  trust_decision_ref? · environment_snapshot_id · skill_inventory_id
  project_home_manifest · result(ready|blocked) · issues[] · receipt_digest

Task
  id · project_id · project_revision · original_request_ref
  latest_contract_ref? · active_run_ref?
  lifecycle(draft|active|completed|cancelled) · status_projection
  dispatch_state(queued|running|waiting|none)
  queue_entry? { enqueued_event_seq, projected_position, blocked_by_task_id? }
```

没有 `active + initialized + identity current + intent approved` 的 Project，Core 拒绝创建或启动 Task。ProjectIntentRevision 是跨 Task 的产品连续性边界，不存每个 Task 的临时实现细节；已有仓库初始化时 Core 可从 README/规则/当前行为提出候选，但用户必须确认，不能把模型推断直接升为产品原则。ProjectHome 是逻辑项目命名空间，不是 target repository；切换项目会同时切换 Task 列表、产品意图、配置、技能、环境判断和证据索引，任何 ID 跨项目串线都属于 ProtocolViolation。Task 的 status_projection 由 active Run 的 phase/hold/terminal 派生；Run 被 Superseded 后 Task 仍为 active 并指向新 Run，只有某个 Run Completed 或用户取消整个 Task 才改变 Task lifecycle。ExecutionQueue 与 HarnessLease 由同一 Rust 事务更新；`projected_position` 只是当前排序投影，真正授权来自未过期 lease 与队首 entry。

配置使用“全局默认 + 项目稀疏覆盖 + Run 冻结快照”：

```text
GlobalConfigRevision
  revision · step_defaults · human_review_default
  environment/budget defaults · skill_policy_default_ref · content_hash

ProjectConfigPatch
  project_id · revision
  step_overrides: Map<LoopStepId, inherit | replace(AgentExecutionProfile)>
  human_review_overrides: Map<LoopStepId, inherit | off | required>
  environment/budget overrides · skill_policy_override? · content_hash

AgentExecutionProfile
  adapter_id(codex|claude) · installation_id
  model_id · effort_id · skill_policy_ref  // 只约束能力，不选择 Skill

ResolvedProjectConfig
  global_revision · project_patch_revision
  steps[{step_id, profile, human_review, provenance(global|project)}]
  environment/budget refs · skill_policy_ref · skill_binding_revision_ref
  safety_policy_hash
  validation_result · snapshot_hash

GlobalConfigImpactPreview
  base_global_revision · proposed_config_hash · observed_project_set_hash
  affected_projects[] {
    project_id · affected_steps[] · before/after route+human+policy hashes
    installation/provider/account/skill/readiness changes[] · projected_state
  }
  blocking_projects[] · requires_second_confirmation · expires_at · preview_hash
```

2.0.0 的可配置 AI 步骤固定为：`fact_analysis`、`contract_drafting`、`contract_review`、`task_graph_planning`、`graph_review`、`implementation`、`repair`、`node_evaluation`、`final_audit`。Rust verifier、receipt signing、completion gate、Git/artifact delivery 不是 AI 步骤，不提供 CLI/model/Effort 配置。

Project 只保存 override，不复制一份全局配置。全局值变化会动态影响未来创建的 PlanningRunSpec / ExecutionRunSpec；已经存在的 spec 始终绑定当时的 ResolvedProjectConfig 与 SkillSetSnapshot，Attempt 再绑定对应 step profile、step projection 与权限 hash。安全下限在继承层之外，项目不能覆盖；无效 CLI/model/Effort/skills 组合不得静默钳制或 fallback。

GlobalConfigRevision 不能从表单直接落库。Core 必须先针对所有 active 且含 inherit 字段的 Project 生成 GlobalConfigImpactPreview，逐 Project/步骤显示 before→after，并单独高亮 installation、provider、account fingerprint/auth mode、Skill policy 和 readiness 变化。保存命令绑定 preview hash 与 project-set hash；预览过期或项目集合变化即拒绝。存在任何 projected blocked、账号/provider 切换或 Global Skill 影响时需要第二次明确确认，但仍只影响未来 spec。

配置和绑定的生效单位固定为“新 spec”，不是含糊的“新 Task”或“未来 snapshot”：

| 变化 | 尚无活动 Task / 未来 Task | 规划中 Run | 已冻结 ExecutionRunSpec | Completed/Cancelled Task |
|---|---|---|---|---|
| GlobalConfigRevision / ProjectConfigPatch | 下个 PlanningRunSpec 解析最新值 | 当前 Run 不变；用户选择采用时 `PlanningPolicyRestart`，旧 Run Superseded，从原始输入重跑规划 | 当前 Run 不变；只允许 RunPolicyAmendment 创建新 Run | 历史不变；新需求创建新 Task |
| Global/Project SkillBinding | 下个 SkillSetSnapshot 解析最新 binding | 同上；安装本身不等于启用 | 同上；新 Run 重新生成逐步骤投影 | 历史 digest 继续保留 |
| Project 默认预算/重试策略 | 只影响新 spec | 同 PlanningPolicyRestart | 不改硬上限；临时追加预算只走单独的 BudgetGrantReceipt，或新 Run | 历史不变 |
| environment profile / dependency mode / network class / AcceptanceCheck 能力 | 只影响新契约候选 | 旧规划失效，从事实分析重新开始 | 必须 ContractAmendment，并创建新 contract/graph/spec/Run | 历史不变 |
| CLI binary、auth、model capability 或已冻结 projection 的观测漂移 | 不改配置，Readiness/Qualification 显示 blocked/stale | 当前 Attempt 中断；恢复 exact identity 或 PlanningPolicyRestart | `ConfigurationInvalidated`；恢复 exact identity 或新 Run | 只产生历史 integrity finding |

任何 restart/amendment 都先展示新旧 spec、影响范围和失效对象。2.0.0 不跨 Run 复用 Attempt、HumanReviewReceipt、EvidenceReceipt、AuditVerdict 或 candidate；策略变化即使只影响最后一步，也从冻结 base 重新执行全部实现、验证和审计，以换取唯一、可证明的语义。TaskContract/TaskGraph 可以作为已批准的 Task 级资产被新 Run 引用，但不冒充新 Run 的收据：如果新 ExecutionRunSpec 继续使用先前规划输出，Core 必须在进入 Ready 前为新 Run 展示 `CarriedPlanningReviewBundle`，对当前配置要求人审的 planning/contract/graph 输出逐项重新取得 HumanReviewReceipt；未要求人审的步骤仅以 origin chain + 内容 hash 证明沿用。GraphReplan 的新 graph-review 人审和 ReplanApprovalReceipt 与新 Run 创建处于同一事务。

```text
PlanningPolicyRestart
  task/current_run · old_planning_spec_hash · proposed_planning_spec_hash
  trigger/config/skill/capability revisions · invalidated_document_attempts[]
  approval_receipt · restart_digest

RunPolicyAmendment
  task/current_run · old_execution_spec_hash · proposed_execution_spec_hash
  unchanged_contract/graph/base hashes · policy_diff
  invalidated_attempt/evidence/audit/candidate refs[]
  approval_receipt · amendment_digest

BudgetGrantReceipt
  run/current_budget_hash · added_hard_or_soft_limits · reason
  operator · expiry? · grant_digest
```

首次设置向导基于当次 capability snapshot 提出而不硬编码模型 ID：`fact_analysis`、`contract_drafting`、`task_graph_planning`、`implementation`、`repair` 优先选已资格 Codex 默认模型；`contract_review`、`graph_review`、`node_evaluation`、`final_audit` 优先选已资格 Claude review 模型。Effort 取各自官方默认或用户选择，人工审计默认“final audit 后”。用户确认后才生成 GlobalConfigRevision；任一推荐组合不可用时向导保持未完成，不偷偷把所有步骤改到同一模型。

“人工审计”是可选的额外质量 Gate：首版提供关闭、仅 final audit 后、contract review 与 final audit 后三个预设，也可逐步骤 required/off。关闭它不关闭 D11 契约确认、独立 AI reviewer/auditor、Rust verifier 或交付批准。

```text
HumanReviewReceipt
  project/task/run hashes · spec_subject = Planning { planning_spec_hash }
                                   | Execution { planning_spec_hash, execution_spec_hash }
  step_id · operator · decided_at
  decision(pass|reject) · review_output_hash · reason · finding_ids[]
  subject = DocumentStep { input_snapshot_hash, output_hash }
            | CandidateStep { candidate_tree, evidence_set_hash }
  receipt_digest

CarriedPlanningReviewBundle
  new_run/execution_spec hashes · origin_chain_hash
  contract/graph/document_output hashes · required_step_ids[]
  presentation_hash · newly_issued_human_review_receipt_ids[]
  bundle_digest

HumanReviewFinding
  id · review_receipt_id · subject_hash · step_id
  anchor { requirement_id?, path?, line?, ui_region?, evidence_ref? }
  expected_change · severity · status(open|resolved|superseded)
  successor_attempt_id? · resolution_subject_hash? · finding_digest
```

`fact_analysis`、`contract_drafting`、`contract_review`、`task_graph_planning`、`graph_review` 使用 DocumentStep，不要求不存在的 candidate/evidence；`implementation`、`repair`、`node_evaluation`、`final_audit` 使用 CandidateStep。任一 subject 输入变化即过期。reject 必须至少包含一条有锚点和期望变化的 HumanReviewFinding；Core 把开放 findings 注入 §10.1 的唯一后继 Attempt。输出 subject 未变化、finding 未逐项 resolved/superseded 或 successor 没有绑定 finding hash 时，Core 拒绝再次送审。

### 5.2 TaskContract

任务的权威语义边界：

```text
TaskContract
  id · version · content_hash
  project_ref { project_id, project_revision, subject_identity_hash }
  project_intent_ref { revision, intent_hash }
  original_request { initial_raw_text, attachments[content_hash],
                     accepted_user_correction_receipt_ids[] }
  target = GreenfieldTarget { destination, parent_directory_identity_hash,
                              destination_absent_proof, template_hash }
           | ExistingRepoTarget { repository_identity_hash, base_commit,
                                  base_tree, target_ref, worktree_fingerprint }
  environment_profile_ref
  applicable_project_rule_snapshot_ref
  desired_outcome
  facts[] { statement, source_ref, fact_receipt_id?, observed_at,
            source_digest, freshness_policy, evidence_level }
  constraints[] · non_goals[]
  assumptions[] { id, statement, reversible, blast_radius, validation_check_id }
  open_questions[]
  requirements[]
  acceptance_checks[]
  completion_policy
```

事实权威顺序：

1. 用户最新明确决定；
2. 用户指定的权威资料；
3. 仓库内适用的项目规则；
4. 当前源码、测试和实际运行事实；
5. 官方外部资料；
6. 明示假设。

附件、需求文档、仓库 Markdown、源码注释与外部网页中的命令式文字默认都是**待分析内容**，不是给 Harness 的控制指令；只有用户在任务对话中明确授予的范围和仓库内被识别为适用项目规则的文件，才进入指令层。任何材料试图要求改权限、跳过验收、泄露数据或改变权威顺序时记录为 prompt-injection finding，不执行。

契约冻结前，Core 生成 `ApplicableProjectRuleSnapshot { discovery_algorithm_version, base_tree, rules[{canonical_path, scope, precedence, content_hash, snapshot_ref}], snapshot_digest }`。所有角色只接收这份控制区只读快照；auditor 不从 final candidate 重新发现规则。候选修改规则文件或新增可能适用的 scoped rule 时记录 `ProjectRuleChange`，当前 Run 仍按冻结快照工作，并进入 ContractAmendment/用户裁决，不能让候选静默改写自己的验收标准。

### 5.3 Requirement

```text
Requirement
  id                    // R-001，创建后永不复用
  statement             // 描述 What，不规定 How
  kind                  // functional / non_functional / constraint /
                        // prohibition / deliverable / human_judgement
  necessity             // must / optional
  source_anchors[]      // 原始文本字符范围、附件位置、用户决定事件
  acceptance_logic      // 2.0.0 固定为 all_of
  acceptance_check_ids[]
  delivery_spec?        // kind=deliverable 时必填
  risk_level
  superseded_by?
```

Requirement 不存储可由 Agent 修改的 `passed` 字段；状态由当前有效 EvidenceReceipt 动态计算。

`delivery_spec` 是二选一 tagged union：`TrackedInTree { paths[] }` 要求对应路径及 hash 存在于 delivery tree；`ContentAddressedArtifact { artifact_ids[], destination_policy }` 要求制品进入 Core 只读 artifact store，并在交付审批中绑定每个目标路径。只在 candidate scratch 中出现、没有交付模式或没有 DeliveryReceipt 的临时文件，不能满足 deliverable Requirement。

2.0.0 不支持运行期 `any_of`。如果一项要求存在多条合理验收路径，planner 必须在契约冻结前结合用户决定选定一条路径；冻结后所有 mandatory ContractAcceptanceCheck 必须全部通过。这样完成谓词只有一套真值，不把“任一通过还是全部通过”留给实现阶段解释。

### 5.4 三类检查边界

- **ContractAcceptanceCheck**：TaskContract 的组成部分，冻结后只能经 ContractAmendment 产生新版本；它定义当前任务何时满足。
- **ImplementationTest**：producer 可以随代码新增或修订的项目测试；它不能替代 ContractAcceptanceCheck，最终 verifier 会将其纳入测试库存和回归检查。
- **ProtectedOracle**：只存在于 Autome 自身的外部 golden 资格环境，对待发布 Electron、Rust Core、执行 Agent、普通 evaluator 及其 sandbox **全部不可见**；它只能验证用户已经表达的要求与系统安全不变量，不能偷偷增加产品需求。

下文的 `AcceptanceCheck` 均指 ContractAcceptanceCheck。2.0.0 只实现四种通用检查：

- `ProcessCheck`：固定程序与 argv，并必须引用 ExecutableOracleSnapshot；可运行构建、测试、HTTP 探针或 Playwright，由受保护 result parser 记录测试库存、断言、请求 ID、截图与制品。
- `FileCheck`：路径、格式、内容或哈希。
- `GitCheck`：基线/head、变更范围、候选 commit 和脏工作树。
- `UserDecisionCheck`：无法机器判定的产品取舍，由 Rust 根据经过校验的 IPC 决定生成收据。

HTTP、UI、TestSuite 与 Artifact 在首版不是独立执行框架，而是 ProcessCheck 的受保护 runner/profile。只有至少两个真实任务无法由上述通用合同准确表达时，才允许在后续版本提升为新的一等 Check 类型。

每项 Check 必须声明：关联 Requirement、mandatory、预期观察、负向场景、所需环境等级、隔离策略、重复策略、库存策略和 freshness policy。所有 must Requirement 使用 `all_of`：其全部 mandatory Check 有当前有效 pass Receipt 时才算 satisfied；optional Check 不参与完成门，但结果必须展示。

ProtectedOracle 由独立 release-eval 基础设施的 `OracleController` 持有。它从访问受控的 oracle 包读取期望值，将黑盒输入送到应用交付的产品/功能，只接收外部可观察输出，并把结果返回 ReleaseEvalOrchestrator，**永不返回被测 Core**。oracle 文件、路径、期望值、解析器源码和密钥不得通过 argv、环境变量、进程查询、父目录、相邻文件、错误栈、日志、事件、证书或研究/provider 出站通道暴露。sealed 两轮结束前实现团队只看到“评测进行中”；结束后一次性公布汇总结论并轮换失败样例。development/regression oracle 也在单次 Run 结束后才给外部测试报告，不能进入该 Run 的自动返工反馈。

每个 mandatory ProcessCheck 还必须冻结它“实际上执行了什么、由谁判断”的 `ExecutableOracleSnapshot`：

```text
ExecutableOracleSnapshot
  check_id · program_identity{canonical_path, binary_digest, profile_ref}
  argv · cwd · env_policy_hash · timeout
  resolution_manifest[{candidate_path, base_content_hash, role}]
  oracle_assets[{control_store_ref, content_hash}]
  result_parser{runner_id, version, binary/source_hash, config_hash}
  snapshot_digest
```

程序必须是 ReadinessReceipt 中的固定 host binary 或 core-manifest 绑定的 qualified runner；验收 spec、HTTP/UI 断言和 parser 必须在冻结前复制到 Agent 不可写的控制存储并以只读方式执行。若命令经 `package.json`、shell script、测试 config 或其它候选文件间接解析，qualified adapter 必须把完整 resolution closure 列入 manifest；不能证明闭包完整时 Check 为 Inconclusive。candidate 中任一 resolution 文件变化都会使该 Check 失效；如果变更是实现所必需，必须通过 ContractAmendment 重建 Check，不能让 `verify.sh=true`、改 npm script 或替换 Playwright spec 获得通过。ImplementationTest 仍按下述库存规则单独处理，不能成为唯一 mandatory oracle。

“测试数量没减少”不是充分条件。已有仓库在基线与最终树分别生成 `TestInventorySnapshot`：

```text
TestInventorySnapshot
  runner_binary/config/discovery hashes
  tests[{ stable_id, framework_id, source_path, source_location,
           source_hash, declared_tags, observed_status }]
  inventory_digest
```

stable ID 由受保护 adapter 根据 framework ID、源码位置与测试名生成，不能由 producer 自报。删除、改名、等量替换、源码 hash 变化或 runner/config 变化都会产生 `TestInventoryChange`；每一项必须映射冻结 Requirement，并由独立 reviewer 明确判为必要且没有削弱断言。无法稳定发现测试身份的 runner 对“库存不可缩减”检查返回 Inconclusive，不能退化为只比较总数。

### 5.5 TaskGraph

TaskGraph 是可修改的实施策略，不是需求权威：

```text
TaskGraph
  id · version · graph_hash · contract_ref
  nodes[] {
    id · kind · title
    requirement_ids[] · acceptance_check_ids[]
    depends_on[] · expected_outputs[]
    write_scope[] · risk_level · estimated_budget
  }
  coverage_snapshot
```

冻结门：图无环；所有 must Requirement 都映射到至少一个节点与 mandatory Check；节点说明服务的要求；重叠写域必须串行；纯基础设施节点必须指向被解锁的业务节点。

### 5.6 Run / Attempt / AgentClaim

```text
PlanningRunSpec
  project id/revision/identity · original_request_ref
  resolved_project_config_ref · planning_step_routes[]
  planning_skill_set_snapshot_ref · planning_permission_profile_refs[]
  read_only_scope · environment/readiness policy · planning budgets
  planning_spec_hash

ExecutionRunSpec
  origin = InitialPlanning { planning_spec_ref, plan_approval_receipt_ref }
           | PolicyAmendment { prior_execution_spec_ref, amendment_ref }
           | GraphReplan { prior_execution_spec_ref, replan_approval_ref }
           | ContractAmendment { prior_execution_spec_ref, contract_amendment_ref }
  project id/revision/identity · contract/graph refs
  resolved_project_config_ref · skill_set_snapshot_ref
  step_routes[] · attempt_permission_profile_refs[]
  environment/readiness policy · rule/oracle snapshots
  budgets · retry/approval/delivery policies · execution_spec_hash

Run
  planning_spec_ref? · execution_spec_ref? · origin_ref
  base tree · current_readiness_receipt_id · phase/hold/terminal
```

`PlanApprovalReceipt` 在 ExecutionRunSpec 之前计算，绑定 planning spec、候选 contract/graph、ResolvedProjectConfig、SkillSetSnapshot、逐步骤 route/permission digest、execution readiness、预算、展示摘要与用户决定；它不引用尚未生成的 ExecutionRunSpec，因此没有摘要自引用。`ReplanApprovalReceipt` 同理绑定 prior execution spec、ReplanProposal、独立 graph review、新 graph 与用户决定，再由新 ExecutionRunSpec 引用。

- 初始 Task 与 PlanningPolicyRestart 先固定 PlanningRunSpec 和 base tree；它只能产生只读文档型 Attempt。D11 批准在同一事务写入 PlanApprovalReceipt + ExecutionRunSpec、冻结 contract/graph 并使 Run 进入 ContractFrozen。后续 PolicyAmendment/GraphReplan/ContractAmendment Run 通过 tagged origin 引用其批准链，不伪造一次新的初始规划。后续 readiness 只追加 revision，由 `current_readiness_receipt_id` 指向当前项。
- Attempt 明确绑定 `planning_spec_hash | execution_spec_hash` 之一，并固定 LoopStepId、AgentExecutionProfile hash、AttemptPermissionProfile hash、node、purpose、Harness/ModelSelectionIdentity/QualificationReceipt、输入 commit/tree、当前 `(LoopStepId, adapter_id)` Skill projection fingerprint 和 provider session ID。执行/repair/evaluation/final audit 不得绑定 PlanningRunSpec；批准前五个只读步骤不得获得 ExecutionRunSpec 的 candidate-write profile。
- AgentClaim 只包含声称完成的工作、声称执行的检查、阻塞项和建议下一步，始终视为不可信输入。

`AttemptPermissionProfile` 是每个步骤实际权限的唯一权威，而不是 Prompt 中的建议。它在对应 PlanningRunSpec 或 ExecutionRunSpec 冻结前由 Core 根据安全下限、Project/Task/Node scope、CLI capability 与 SkillSetSnapshot 生成：

```text
AttemptPermissionProfile
  id · loop_step_id · node_id? · adapter_id · installation_id
  subject_scope_hash · skill_set_snapshot_hash
  tool_surface {
    provider_available_tools[] · provider_allowed_tools[] · provider_denied_tools[]
    autome_control_tools[] · dynamic_tool_or_mcp_allowlist[]
  }
  filesystem_policy { read_roots[], write_roots[], deny_roots[], nofollow }
  command_policy { qualified_runner_ids[], argv_policy_hash, shell_allowed }
  network_policy { mode, allowed_brokers[], allowed_destinations[] }
  sandbox_policy { mechanism, required_capabilities[], fail_closed }
  secret_policy_hash · safety_policy_hash · profile_hash
```

`provider_available_tools` 记录 CLI 当时暴露的全集，`provider_allowed_tools` 是交集后的精确集合，`provider_denied_tools` 明示所有相邻高风险能力；三者在 adapter 启动后通过 provider 事件/探针再核对。Skill 共享这一个 Attempt 级 profile，不能形成权限并集或增加工具。任何 profile、provider 可见工具、文件范围、网络、sandbox 或 SkillSet 漂移都会阻断 Attempt；不得用 Prompt、默认配置或 provider fallback 替代机械限制。

### 5.7 EvidenceReceipt

EvidenceReceipt 由 Rust verifier 生成。它使用公共 envelope + 按 Check 类型区分的 tagged payload，避免给文件检查伪造命令或测试计数：

```text
receipt_id · nonce · run_id · check_id
contract_hash · check_hash · executable_oracle_snapshot_hash?
project_rule_snapshot_hash · candidate_commit · candidate_tree_hash
verifier_version · policy_hash
environment_class · OS/arch · toolchain/lock/service fingerprint
payload =
  ProcessResult {
    program, args[], cwd, timeout, exit_code,
    discovered?, executed?, passed?, failed?, skipped?, filtered?,
    baseline_inventory_digest?, final_inventory_digest?, inventory_changes[],
    assertions[], request_ids[], screenshot_refs[], artifact_hashes[]
  }
  | FileResult { path, content_hash, assertions[] }
  | GitResult { base, head, diff_hash, changed_paths[], clean }
  | UserDecisionResult { gate_id, action_hash, decision, operator, decided_at }
stdout_digest · stderr_digest · raw_log_ref
started_at · finished_at · result(pass|fail|inconclusive)
receipt_digest
```

contract、check、project-rule/oracle snapshot、candidate tree、依赖、环境或 freshness 任一不匹配，收据立即失效。最终候选一旦变化，旧收据只能作为历史，不得进入完成门。

`UserDecisionResult` 只证明当前本机操作者在看见绑定内容后作出了决定，不证明决定本身客观正确。它必须保存界面展示摘要的 hash、gate/action/policy revision、nonce 与时间；撤销决定会产生新事件并使受影响节点和收据失效。

### 5.8 AuditVerdict、CandidateCertificate 与 CompletionCertificate

独立 evaluator 针对每项 Requirement 输出 `satisfied / not_satisfied / unverified`，引用具体 Receipt。Rust 内核随后重新检查候选资格谓词；只有事实收据与独立审计同时通过，才签发 CandidateCertificate。

最终候选的所有验收与审计通过后，Core 先签发 `CandidateCertificate`，绑定完整 ExecutionRun origin chain + ExecutionRunSpec、ProjectIntentRevision、Project/config/skill snapshots、contract、graph、project-rule/oracle snapshots、candidate commit/tree、Fact/Evidence set、TestInventory、必需 artifact set、AuditVerdict、最终 Readiness revision、全部实际 Attempt 的 AttemptPermissionProfile、逐步骤 ModelSelectionIdentity/Qualification 与 HumanReviewReceipt。它只表示“这个候选具备交付资格”，**不表示用户任务 Completed**。只有交付演练、批准、交付和只读交付树/制品核对全部完成，才可能签发 CompletionCertificate。

完成证书必须列出：TaskContract 版本、candidate commit/tree、交付 destination/ref/commit/tree、验证环境等级、每项 Requirement 的证据、保留的已知基线问题，以及没有声称验证的环境。

### 5.9 Environment Center、SupportedEnvironmentProfile 与 ReadinessReceipt

本地环境不是 PATH 猜测，而是一组可追踪快照和受控修复事务：

```text
SignedDependencyCatalog
  catalog_hash · signature · issued_at
  entries[] { component_id, requirement_class, version/channel policy,
              discovery_rules, install_recipes, allowed_origins,
              signature_policy, health_probe, rollback_class, licenses }

EnvironmentSnapshot
  id · observed_at · OS/arch · project_id?
  components[] {
    component_id · required_by[]
    presence(missing|present|duplicate)
    integrity(trusted|untrusted|unknown)
    auth_mode(not_applicable|dedicated|shared_macos_keychain|dedicated_api_key)
    auth_state(not_applicable|signed_out|signed_in|expired|unknown)
    auth_storage? { kind, canonical_identity_hash, owner_mode, policy_source }
    qualification(qualified|stale|failed|not_required)
    readiness(ready|warning|blocked)
    candidates[] { canonical_path, symlink_chain, owner, install_manager,
                   version, arch, binary_digest, signature/team_id }
    selected_candidate_id? · selection_source(explicit|policy) · issues[]
  }
  package_manager_provenance { brew_origin, api_domain, bottle_domain, core_origin }
  skill_inventory_id · snapshot_digest

EnvironmentRemediationPlan
  snapshot_digest · catalog_hash · project/profile refs
  actions[] { action_id, recipe_id, operation, exact_target,
              from_version?, to_version?, source, argv, downloads[],
              resolved_package_metadata_hash,
              cwd, env_allowlist/hash, network_allowlist, timeout,
              execution_mode(no_pty|visible_terminal|system_ui),
              preconditions[], privilege, user_prompts[], post_probe,
              rollback_class, requires_restart }
  total_download_limit · policy_hash · expires_at · plan_digest

EnvironmentInstallationTransaction
  txn_id · idempotency_key · plan_digest · state
  completed_actions[] · pending_action? · user_action_challenge_id? · unknown_outcome?

UserActionChallenge
  challenge_id · txn_id · action_id · plan_digest
  state(pending|probe_failed|verified|expired|abandoned)
  kind(system_ui|visible_terminal|account_login|admin_install|restart)
  instruction_template_id · allowed_entrypoint · expected_post_probe
  issued_at · expires_at · attempts · last_probe_result?
  actions(recheck|continue_after_verified|abandon) · challenge_digest

EnvironmentChangeReceipt
  plan_digest · action_id · before/after observations
  exit/result · stdout/stderr digests · started_at/finished_at · receipt_digest
```

Requirement class 防止“监测项缺失=所有 Task 阻塞”：iTerm2 是 `optional_convenience`；Homebrew 是仅修复事务需要的 `provisioner`；Node/npm/Skills CLI 是 `skill_manager` 或项目 profile 依赖；Codex/Claude 是仅在 StepExecutionRoute 引用时阻塞的 `route_required`；Xcode Command Line Tools/Git 与 sandbox 是 `core_task_required`；空 Skill 目录不阻塞，只有已绑定 Skill 缺失或漂移才阻塞。

| Component | 首选受控来源 | 安装后事实检查 |
|---|---|---|
| iTerm2 | 资格认证的 Homebrew cask 或 iterm2.com 签名制品 | bundle ID/version/Developer ID/notarization/launch |
| Xcode CLT/Git | macOS `xcode-select --install` 可见系统流程或已资格独立 Git | CLT receipt/path、Git binary/version/arch/机器输出能力 |
| Codex CLI | OpenAI standalone 或资格认证 Homebrew cask | exact binary/version/signature/app-server schema/file credential store/account capability |
| Claude Code CLI | Anthropic native stable 或资格认证 Homebrew cask | exact binary/version/signature/auth status/stream-json/sandbox capability |
| Node/npm | host profile 指定的 exact channel/artifact | binary/version/arch/npm/npx compatibility |
| Skills CLI | pinned npm package，仅在隔离 fetch 环境 | package integrity/version/search schema/telemetry disabled |

presence、integrity、auth_mode/auth_state 与 qualification 是正交事实，再由 Core 计算 readiness；不能把“命令存在”显示成“可运行”。同一 CLI 有多个候选时，PATH 顺序不构成选择，用户或已冻结 policy 必须选择 exact candidate，切换会使 Qualification/Readiness 失效。iTerm2 通过 bundle ID、版本、签名和 LaunchServices 检查，属于推荐的可见终端与快捷入口，缺失通常只是 warning；Codex/Claude 仅在某个 StepExecutionRoute 使用它时才阻塞。技能目录同时显示数量、有效性、来源、重名、symlink、CLI 可发现性和是否受 Autome 管理。

“一键补齐”表示用户一次批准一个绑定 EnvironmentSnapshot + SignedDependencyCatalog hash 的已展开 plan，不表示绕过 macOS、Homebrew、Keychain 或账号交互。Renderer 只能提交 plan/action ID，不能提交 shell、URL 或 argv。Core 按依赖顺序执行已签名 recipe，每步后立即重探测；计划、catalog 或 package metadata 漂移即过期，失败停止后续动作并保留精确状态。全局一次只允许一个 EnvironmentInstallationTransaction；若活动 Run 引用将被修改的组件，升级必须等待该 Run 安全停靠。

Homebrew cask/formula 名不是版本锁。执行前必须把 manager 当前解析结果固定为 exact version/artifact/digest/source，并证明该版本已经资格认证；如果 cask 已漂到 catalog 未批准的新版本，计划直接 Blocked，不能“先安装再跑 canary”。2.0.0 的 SignedDependencyCatalog 随签名 Autome release 原子更新，不单独在线热更新。

每个安装 action 从空白基线构造环境：只加入 catalog 声明的 PATH、HOME/cache、locale、proxy/registry 和控制变量；例如 Skills CLI 必须设置 telemetry off，Homebrew 必须禁止自动 update，npm registry/proxy 必须与 plan 一致。需要 PTY/系统 UI 的动作只能走 visible mode；无头动作不能继承 Electron 或用户 shell 环境。EnvironmentChangeReceipt 同时绑定实际 env/network policy hash。

Autome 不卸载或降级安装前已存在的软件；每个 action 的 rollback 诚实标为 `reversible / compensatable / manual`，只有本次新建且 receipt 可证明无其它引用的组件可以自动回滚。登录、系统扩展、shell integration 和需要管理员口令的动作进入可见终端/系统流程并持久化 UserActionChallenge；它绑定 txn/action/plan、精确入口、预期 post-probe 与 expiry。用户返回后只能“重新检测 / 在已验证后继续 / 放弃”；窗口关闭、口头确认或外部命令退出 0 都不能算完成，只有绑定 probe 成功才能推进 transaction。challenge 过期、plan 漂移或观察到不相关安装时生成新 challenge，不能错误归因。

快捷操作包括：安装/更新、重新检测、选择 exact installation、打开官方下载页、在 iTerm2 打开 Project、开始 Codex/Claude 登录、显示修复命令、打开 Skill 目录、查看 receipt。iTerm2 缺失时使用 macOS Terminal、系统安装器或浏览器完成可见步骤；被动 probe 不自动启动终端，打开 Project 只传 project ID，由 Core 解析固定目录，不通过 AppleScript 拼接命令。没有 Homebrew 或来源不在 allowlist 时，系统提供 guided action，不静默安装 Homebrew或改写镜像/PATH/shell dotfiles。Homebrew 只做目标 package/cask 的 capability probe 和精确动作；`brew doctor` 的无关警告不能阻塞，也不得触发全局 upgrade、cleanup、link、tap trust 或“修复全部”。

Task 的执行环境继续使用冻结 profile：

```text
SupportedEnvironmentProfile
  id · version · profile_hash · task_kind
  OS/arch · required_programs[{name, version_range, source, digest_policy}]
  lockfile_rules[] · dependency_mode · browser requirements[]
  sandbox_capabilities[] · network_policy · qualified_check_runners[]
  qualified_step_route_policy · producer_evaluator_separation_policy

ReadinessReceipt
  revision · scope(planning|execution) · profile_hash · environment_relevant_inputs_digest
  observed_at · valid_until
  subject =
    ExistingRepoSubject { repository_identity_hash, base_commit,
                          target_head, worktree_fingerprint }
    | GreenfieldSubject { parent_directory_identity_hash, destination,
                          destination_absent, template_hash }
  programs[{canonical_path, version, binary_digest}]
  lockfile_hashes[] · dependency_state[] · browser_fingerprints[]
  sandbox_probe_results[] · qualified_step_routes[] · skill_projection_hashes[]
  result(ready|not_ready) · missing[] · receipt_digest
```

`greenfield-web-v1` 是首个 host profile，不是 Autome 内置发行版；S0 冻结其 Node/package-manager/browser 范围、模板 hash、依赖准备方式和 check runners。用户可以提前准备，也可以在 Run 外让 Environment Center 按受支持 recipe 补齐。现有仓库也必须显式匹配一个已资格认证的 host profile。机器上缺少工具链、浏览器、锁定依赖或所选步骤路由时，Core 在任何项目代码运行前阻塞并给出 RemediationPlan；Task 自身绝不安装。

`environment_relevant_inputs_digest` 只覆盖 profile 指定的 lockfiles、runner/config、工具 binary、浏览器与依赖状态，不绑定普通源码 tree；普通源码变化由 candidate/evidence 失效规则处理，不触发 readiness 中断。环境相关输入、profile 或模型资格变化会追加新的 ReadinessReceipt revision，并使相关 EvidenceReceipt、AuditVerdict 与 CandidateCertificate 失效。CandidateCertificate 必须绑定最终 candidate 的环境输入所对应的 current revision，不能复用基线 preflight receipt。

`RepositoryIdentity` 绑定 canonical no-follow path、volume UUID、device/inode/file ID、Git common-dir identity、object format 与安全相关 config digest；`DirectoryIdentity` 绑定绿地目标父目录的相同文件系统身份。Core 从批准前一直保留打开的受保护目录句柄，并在演练、CAS/rename 前后重新核对路径仍指向同一对象；路径替换、mount 变化或 symlink 链变化会使所有审批失效。

### 5.10 ModelSelectionIdentity 与 QualificationReceipt

2.0.0 能机械证明的是“某个 CLI/model/Effort 组合可用、选择配置不同且分别通过资格测试”，不是 Provider 内部权重必然不同：

```text
HarnessCapabilitySnapshot
  adapter_id · installation_id · canonical_binary/digest/version/signature
  protocol/schema hash · auth_mode/state
  lifecycle/tool/sandbox/resume/usage capabilities
  models[{opaque_id, resolved_wire_name?, efforts[], default_effort?}]
  observed_at · valid_until · snapshot_digest

ModelSelectionIdentity
  adapter_id · installation_id · provider · CLI/protocol/schema hashes
  model_id · resolved_wire_name? · service_tier? · provider_native_effort
  auth_mode · account_fingerprint · exposed_snapshot_or_fingerprint?
  qualification_batch_id · model_choice_key_hash · runtime_selection_hash

QualificationReceipt
  identity · HarnessCapabilitySnapshot · account capability snapshot
  canary_manifest_hash · run_ids[] · issued_at · valid_until
  result(qualified|not_qualified) · receipt_digest
```

Codex 的可选项来自 app-server `model/list` 与 provider capability；Claude CLI 当前没有可依赖的等价稳定 model-list 合同，因此只展示 S0 已通过真实启动 probe 的 alias/ID 与实际解析结果。Effort 保留 provider-native 值和说明，不把两个 CLI 的 `high` 当成相同计算量；UI 只允许 capability snapshot 中真实存在的组合。

AttemptPermissionProfile、resolved config、Skill 投影、adapter/installation、账号、service tier 与 Effort 单独进入 `runtime_selection_hash`，不进入“不同模型”比较；否则仅因生产者与评估者权限、运行时或算力档位不同就会被误判为模型身份已分离。`model_choice_key_hash` 只覆盖 provider、实际 resolved model identity 与 Provider 可见 snapshot/fingerprint；alias 无法解析到稳定 identity 时该组合不能用于分离门。资格 canary 证明 adapter 能执行和拒绝对应权限类别，当前 Run 的精确目录、工具和 Skill 集合仍由 Attempt 启动后的 profile probe 证明。

首版要求 `contract_drafting ≠ contract_review`、`task_graph_planning ≠ graph_review`、`implementation ≠ node_evaluation/final_audit` 的 `model_choice_key_hash`；推荐跨 CLI 分离，但同一 adapter 的不同已资格模型也可满足默认策略。同一模型仅改变 Effort、service tier、账号、CLI 或 Skill 不能满足。上下文、workspace 与 SkillSetSnapshot 仍独立，人工审计不能替代该门。明确要求不同 provider 的 Task 只有在两个实际 route 的 provider identity 不同时才可执行。

Provider 未提供不可变 snapshot 时，资格收据最长有效 7 天，并在每个 Run 启动执行固定 canary；sealed pass² 的两次 Run 必须在同一资格批次的 24 小时内完成。CLI、认证、模型、Effort、协议、sandbox 或 skill projection 漂移时配置进入 `ConfigurationInvalidated`；无法恢复 exact selection 就 Blocked，绝不自动回退默认模型。

### 5.11 Skill Inventory、Marketplace 与 SkillSetSnapshot

```text
SkillInventory
  roots[] { path, scope, cli, origin_kind, writable, watched }
  entries[] { name, path, source?, content_digest, valid, duplicate_group?, managed }

SkillPackageSnapshot
  catalog_provider · repository · exact_commit · skill_subpath
  content_digest · file_manifest · metadata · requested_capabilities

SkillAuditReceipt
  package_digest · scanner/policy hashes · findings[]
  compatibility(codex|claude|both|unsupported) · verdict

SkillInstallPlan
  operation(install_to_vault|import_to_vault|stage_update|gc_unreferenced)
  package/audit/current-vault hashes · content/permission diff
  expiry · plan_digest

SkillInstallationTransaction
  txn_id · idempotency_key · plan_digest · state
  completed_actions[] · pending_action? · unknown_outcome?

SkillInstallReceipt
  plan/package/audit hashes · before/after Vault observations
  rollback_result? · receipt_digest

SkillBindingPlan
  operation(enable|disable|switch_digest|change_scope)
  installed_skill_digest · current/proposed binding hashes
  target(global|project) · affected_projects[] · steps[] · cli_targets[]
  per_step_skillset_diff[] · invocation_policy · expiry · plan_digest

SkillBindingReceipt
  plan_digest · before/after binding revisions
  affected_project_resolved_digests[] · projection_probe_results[]
  user_decision_receipt · receipt_digest

GlobalSkillBinding
  revision · skill_digest · steps[] · cli_targets[]
  invocation(explicit_only|implicit_allowed) · state(enabled|disabled)

ProjectSkillBinding
  project_id · revision · skill_digest
  mode(inherit|enable|disable|pin_version)
  steps[] · cli_targets[] · invocation? · state?

SkillSetSnapshot
  project/config/binding revisions · per_step_skill_digests[]
  projections[] {
    loop_step_id · adapter_id · source_skill_digests[]
    normalized_root_ref · projection_manifest_hash
    invocation_policy · explicit_invocations[] · projection_probe_hash
  }
  snapshot_digest
```

Environment Center 只读盘点所有已知位置，并明确区分原生与兼容路径：Codex 官方 Project/User 路径是 `<repo>/.agents/skills` 与 `$HOME/.agents/skills`，`$HOME/.codex/skills` 作为当前工具生态的兼容路径；Claude 使用 `<repo>/.claude/skills` 与 `$HOME/.claude/skills`。用户要求的 `$HOME/.agent/skill` 与 `<repo>/.agent/skill` 作为默认启用监测的 legacy roots，也允许在全局/Project 中增补其它只读 root；它们默认不会被两套 CLI 原生加载，Autome 只提供“打开目录、导入审计、迁移或忽略”，不把发现等同于安装，也不默认写入。

2.0.0 的主市场是 `find-skills` 所使用的 skills.sh/Open Agent Skills 生态。`SkillCatalogAdapter` 封装搜索，搜索请求只能包含通过 DLP 的通用关键词；安装量、GitHub source/stars、发布者、最近更新、license 和 CLI 兼容性用于筛选与排序，但名称、描述、作者和所有热度指标仍是不可信展示数据，不能替代审计。当前 search API/CLI 没有稳定生产合同，因此 S0 必须固定一个 `skills@<qualified-version>`、内容完整性和返回 schema；调用时设置 `DISABLE_TELEMETRY=1`。S0 若找不到能稳定返回 exact public source 的适配方式，M3 和 2.0 发布都不能通过；运行期短暂离线时市场显示不可用，但本地库存、已锁定 Skill 和“粘贴公开 GitHub 来源后审计安装”仍可使用。Codex app-server 的 `plugin/list/install` 当前标为 under development，不作为 2.0.0 安装主链。

安装与启用固定为两个独立决定：

```text
Search → Fetch to Quarantine → Static Audit → User Approval
       → Immutable Skill Vault → Installed (disabled)

Installed → Binding Impact Preview → Separate User Approval
          → Global/Project/Step Binding → Future Run Snapshot
          → CLI Projection Probe → Available to Attempt
```

首版只接受 catalog 映射的公开 HTTPS GitHub 来源，并解析到 exact commit；branch、tag、`latest` 和市场展示版本不能作为锁。审计前 Fetch Sandbox 只允许固定 GitHub API/archive hosts，禁用代理继承、cookie、凭证、file/ssh/git scheme、任意重定向、Git system/global config、URL rewrite、hooks、submodule、LFS、smudge/filter 和 credential helper，并限制响应/pack 字节、文件数、路径深度、解压比、时间与磁盘。优先由 Rust HTTPS broker 下载 exact-commit archive，不执行仓库代码。

quarantine 禁止 Agent 读取、Markdown HTML 执行、远程图片加载和脚本运行。审计覆盖路径穿越/zip bomb/symlink/device、重复或 Unicode 混淆名、二进制/归档/隐藏文件、shell/下载/凭证/Home/网络、Claude `!command`、hooks/MCP/plugin、allowed-tools、依赖、license 和 prompt injection。结论只能是“未发现已知风险”，不能写“安全”。

安装写入 Autome owner-only、内容寻址 Skill Vault，不直接调用 `npx skills add` 修改用户目录。Skills CLI 必须先由 Environment Center 作为 exact package/digest 安装到受控工具目录；运行期禁止 `npx` 下载、npm lifecycle 和浮动 latest。若用该 CLI 做兼容 fetch，也只能在 Fetch Sandbox 输出 staging；退出码和文案不是成功证据，Core 必须独立核对期望文件集、SKILL.md schema、source commit、内容 digest、无越界/断链后才签发 SkillInstallReceipt。安装成功的默认终态只是 Vault 中 `Installed (disabled)`，不得创建 binding 或成为 AvailableToAttempt。Vault 永久保留经审计的原始字节；Attempt 不直接挂载这些字节，而由确定性的 ProjectionBuilder 生成并绑定另一份内容寻址 manifest。

权限的真实强制粒度是整个 Attempt，不是单个 Skill：同一 SkillSetSnapshot 中的所有 Skill 共享冻结 StepPolicy，工具调用不能可靠归因到某一 Skill。普通 Skill 因此只能使用基础 StepPolicy 已经允许的文件/命令；它声明的 allowed-tools、网络或 binary 只是审计输入，不能加权。自带 hooks、第三方 MCP/plugin 或直接凭证需求的包在 2.0.0 不能绑定。新 Skill 默认 `explicit_only`。

每个 Run 按 `(LoopStepId, adapter_id)` 从 Vault 物化独立只读投影，绝不生成一个 CLI 级并集。ProjectionBuilder 移除任何能授予权限的 Skill metadata，并按冻结 invocation policy 写入 CLI 原生限制：Codex `agents/openai.yaml` 强制 `allow_implicit_invocation: false`，Claude `SKILL.md` 强制 `disable-model-invocation: true`；只有对应 PlanningRunSpec 或 ExecutionRunSpec 明列于 `explicit_invocations[]` 的 Skill 才由 Core 在该 Attempt 首个输入中用原生显式语法点名。若用户明确批准 `implicit_allowed`，Builder 才为该精确步骤恢复相应可发现 metadata，但权限仍完全来自 AttemptPermissionProfile。显式调用数量、语法与持久上下文行为必须在当前 CLI capability snapshot 中有已资格上限，超限直接拒绝配置。

Codex 的 `skills/extraRoots/set` 只会增加 root，不能当成隔离：adapter 使用独立 OS home/CODEX_HOME，先 `skills/list(forceReload=true)` 枚举全部来源，在专用 config 中逐 path 禁用所有非当前 step projection 的 user/Project/compat Skill，再设置该 Attempt 唯一 extra root；sandbox 同时 deny-read 原仓库和宿主的 Skill 目录。最终再次 list，effective enabled set 必须精确等于当前 step projection 加上 core-manifest 明示的签名 system-skill allowlist，否则该 Codex+Skill route 不合格。Claude 用 `--bare`、`disableSkillShellExecution=true`、生成的 `skillOverrides` 与只读 `--add-dir` step projection。每个 projection probe 都必须包含“不点名时不加载”和“点名时只加载 exact digest”的正反证据。任何未批准 Skill 不能作为指令加载；同名冲突必须让用户选择确切 digest 或受控别名，否则阻塞。

GlobalSkillBinding 是所有 Project 的默认集合；ProjectSkillBinding 只记录 inherit/enable/disable/pin-version 差异，并可按 LoopStepId 与 CLI 缩小范围。implementer 与 reviewer/auditor 默认不是同一技能集合，避免同一指令同时污染生产和评测；ProtectedOracle 永远不能包装成 Skill。解析后的逐字段 provenance 展示在 Project 配置中。

GlobalSkillBinding + ProjectSkillBinding 是“选择哪些 Skill”的唯一权威；Global/Project config 中的 `skill_policy_ref` 只约束脚本、网络、binary 和最低审计等级，不能列出或覆盖具体 Skill。SkillBindingResolver 先解析 binding，再用 ResolvedProjectConfig 的 policy 过滤，得到唯一 SkillSetSnapshot。

证据等级分开显示：`Installed`（Vault 字节正确且默认 disabled）、`Bound`（另一次用户决定已写 binding）、`Discoverable`（目标 CLI 看见精确投影）、`AvailableToAttempt`（Attempt 已绑定）、`Invoked`（有 CLI 事件或仅 Claim）、`Effective`（任务验收证明结果正确）。安装或调用 Skill 都不等于任务正确。更新只写入新 digest；另一个 SkillBindingPlan 展示内容/权限与逐步骤 SkillSet diff 后才能切换，活动 Run 不变。Global binding 必须单独展示全部受影响 Project 并再次确认；Project/步骤 binding 显示最终投影 diff。禁用先切 binding，物理 GC 只允许无当前 binding、无历史 Run 引用的 digest。

### 5.12 交付收据链

交付链中的每个对象都是 Rust 领域对象并单独持久化，但不使用会自引用的单一信封：`CandidateCertificate` 的 candidate envelope 为 `run_id · execution_origin_chain_hash · execution_spec_hash · contract_hash · policy_hash · nonce · issued_at · certificate_digest`；其后的 DeliveryRehearsalReceipt、DeliveryApprovalReceipt、DeliveryReceipt、DeliveredTreeCheckReceipt、ProjectTargetTransitionReceipt 与 CompletionCertificate 才使用 `run_id · contract_hash · candidate_certificate_hash · policy_hash · nonce · issued_at · receipt_digest` delivery envelope。

```text
delivery_subject =
  ExistingRepoDelivery { repository_identity_hash, target_head,
                         target_worktree_fingerprint, new_ref }
  | GreenfieldDelivery { parent_directory_identity_hash, destination,
                         destination_absent_proof, template_hash }
```

两类 subject 是穷举 tagged union，不使用 nullable `target_head`、虚构 Git baseline 或 sentinel 值。

| 对象 | 唯一签发者 | 额外绑定 | 主要失效条件 |
|---|---|---|---|
| `CandidateCertificate` | candidate gate | pre-delivery Project intent/config/skill、candidate commit/tree、project-rule/oracle、Fact/Evidence、TestInventory、AuditVerdict、environment/model qualification、human review、artifact set | 未授权的 Project intent/config/skill、candidate、契约、规则/oracle、事实/证据、环境或资格变化 |
| `DeliveryRehearsalReceipt` | verifier | task-kind subject、target identity、target head/parent、delivery tree、全量 Check receipts | target/candidate/profile/identity 变化 |
| `DeliveryApprovalReceipt` | Core IPC command handler | gate、展示摘要、delivery rehearsal、destination/new ref、artifact destinations、expiry、operator | 绑定值变化、撤销或过期 |
| `DeliveryReceipt` | capability broker | 实际 ref/directory/artifact 操作、前后 filesystem identity、result/UnknownOutcome | 只追加，不覆盖；失败不能当通过 |
| `DeliveredTreeCheckReceipt` | Core 只读 checker | 实际 ref/directory tree、artifact hashes、worktree fingerprint | 与 DeliveryReceipt 任一不符 |
| `ProjectTargetTransitionReceipt` | Project service | greenfield destination、delivered tree、new repository identity、project revision | 仅适用于首个绿地交付；身份或 tree 不符 |
| `CompletionCertificate` | completion gate | 上述完整 receipt set、按 task-kind 计算的最终谓词 | 终态产物；后续篡改不改写历史，只产生 integrity finding |

所有对象有显式 schema version；签发、事件追加与投影更新处于同一 SQLite 事务。这里的“签发”是本地 Core 通过内容摘要和受保护 nonce 建立不可串线性，不宣称对已控制当前 OS 用户的攻击者提供法律意义上的不可抵赖签名。

绿地首个 Task 是唯一 Project revision 例外：DeliveredTreeCheck 通过后，Core 在**同一 SQLite 事务**写入 ProjectTargetTransitionReceipt、把 Project locator 从 pre-transition revision 提升到绑定 delivered tree 的 post-transition revision、签发 CompletionCertificate 并投影 TaskCompleted。CandidateCertificate 显式授权并绑定这一对 revision，因此该精确转换不使它失效；任意其它 Project 变化仍立即失效。

---

## 6. 确定性 Loop 状态机

### 6.1 Project 状态

```text
lifecycle = active | archived

phase = Registered → Inspecting → AwaitingTrust → Initializing
        → ResolvingIntent → ResolvingConfig → CheckingEnvironmentAndSkills → Ready

hold = none | IdentityChanged | IntentUnresolved | ConfigInvalid | EnvironmentBlocked |
       SkillsBlocked | InitializationFailed
```

Project hold 不影响查看历史 Task，但禁止创建新 Task 或启动新 Run。repository/directory identity 变化会递增 project revision 并要求 reinitialize；`new_product` 首个 Task 原子交付后，由 ProjectTargetTransitionReceipt 把 locator 从 GreenfieldDestination 转为 ExistingRepository。存在非终态 Run 或 Environment/Skill/Project transaction 时，archive 命令直接拒绝；用户必须先完成或取消相关工作。归档不删除 ProjectHome、Task 或证据，恢复时必须重新检查身份、配置、环境和 Skills。

### 6.2 Run 状态

Run 状态是一个明确的三轴结构，不把等待、阶段和终态混成同一枚举；Task 只投影 active Run：

```text
phase = Received | ResolvingProjectContext | DiscoveringFacts |
        DraftingContract | ContractReview | PlanningGraph | GraphReview |
        CheckingReadiness | ContractFrozen |
        Ready | Executing | Repairing | Replanning | Integrating |
        FinalVerifying | FinalAuditing | DeliveryRehearsing |
        Delivering | DeliveredTreeChecking

nominal path =
  Received → ResolvingProjectContext → DiscoveringFacts
  → DraftingContract → ContractReview
  → PlanningGraph → GraphReview → CheckingReadiness → ContractFrozen
  → Ready → Executing → Integrating → FinalVerifying → FinalAuditing
  → DeliveryRehearsing → Delivering → DeliveredTreeChecking

hold =
  none
  | AwaitingClarification
  | AwaitingPlanApproval
  | AwaitingHumanAcceptance
  | AwaitingDeliveryApproval
  | AwaitingContractAmendment
  | AwaitingCorrectionClassification
  | AwaitingConfiguredHumanReview
  | ConfigurationInvalidated
  | Paused | Blocked | BudgetExhausted | Stalled | UnknownOutcome

terminal = none | Completed | Superseded | ProtocolFailed | Infeasible | Cancelled
```

`hold` 保留进入前的 phase；解除后只有状态表声明的 guard 可以继续。`terminal` 一旦非 none，普通命令不能恢复原 Run。`Completed` 只存在于 terminal：`phase=DeliveredTreeChecking` 且 §7 完成谓词为真时，Rust 才能写入。

合法回路：

```text
ContractReview                                      → DraftingContract
GraphReview                                         → PlanningGraph
CheckingReadiness + ready                           → hold=AwaitingPlanApproval
AwaitingPlanApproval + valid approval               → atomically freeze contract/graph/
                                                       ExecutionRunSpec → ContractFrozen
ReadinessInvalidated + origin                       → CheckingReadiness(revalidation_origin)
CheckingReadiness + same profile/capabilities + node origin
                                                     → Executing; node=Verifying
CheckingReadiness + same profile/capabilities + post-integration origin
                                                     → FinalVerifying
CheckingReadiness + material capability change      → hold=AwaitingContractAmendment
configured AI step passed + human_review=required   → hold=AwaitingConfiguredHumanReview
configured human review pass                        → guarded next phase
configured human review reject                      → §10.1 对该 LoopStepId 的唯一 reject 后继
CLI/model/Effort/auth/skill projection drift        → hold=ConfigurationInvalidated
Executing / Integrating / FinalVerifying / FinalAuditing /
DeliveryRehearsing                                  → Repairing → Executing
Executing / Integrating / FinalVerifying /
FinalAuditing / DeliveryRehearsing                   → Replanning → PlanningGraph → GraphReview
GraphReview + initial-plan origin + pass             → CheckingReadiness
GraphReview + replan origin + reviewed user approval → old Run Superseded;
                                                       new graph + ExecutionRunSpec + Run;
                                                       required new-Run human receipts
                                                       complete → Ready，否则等待人审
FinalAuditing + human judgement hold                → FinalAuditing
FinalAuditing + pass                                → DeliveryRehearsing
DeliveryRehearsing + pass                           → hold=AwaitingDeliveryApproval
AwaitingDeliveryApproval + valid approval           → Delivering
Delivering + DeliveryReceipt                        → DeliveredTreeChecking
candidate changed before delivery                   → Integrating
target head/profile/destination changed             → hold=AwaitingContractAmendment
target worktree fingerprint changed                 → hold=Blocked(TargetChanged)
DeliveredTreeChecking + observed mismatch           → hold=Blocked(DeliveryIntegrityMismatch)
Delivering + action outcome unknowable               → hold=UnknownOutcome
```

`UnknownOutcome` 是 Run hold，同时对应一个 Attempt/ExternalAction outcome。Core 必须先通过 provider session、process identity、candidate tree 和 action receipt 核对实际结果；确认未执行才允许重试，确认已执行则补记 receipt，仍无法判断则保持 hold 等待人工处置。

`DeliveryIntegrityMismatch` 表示已经观察到交付对象与收据不一致，不是 UnknownOutcome。Core 不会原地覆盖或重试；用户只能恢复批准对象，或通过 ContractAmendment 选择新 destination/ref 后创建新交付链。

Core 在 Attempt 提交 claim、启动 verifier、最终验证和交付演练前重算 environment-relevant digest；外部工具/model 身份在活动 Attempt 中变化时立即中断。`ReadinessInvalidated` 保存 revalidation origin 并重新探测：若新 revision 仍满足冻结 profile、dependency mode、network policy、runner 与预算边界，节点候选回到 Verifying，已集成候选回到 FinalVerifying，绝不直接跳回 FinalAuditing/Delivery。CLI/model/Effort/Skill 的执行选择变化走 ConfigurationInvalidated/RunPolicyAmendment；环境等级、dependency mode、network policy、runner 或验收能力变化才走 ContractAmendment。

全局或 Project 配置变更不会后台改写已冻结 spec。仍处于批准前规划阶段时，`PlanningPolicyRestart` 展示旧/新 PlanningRunSpec 与将丢弃的文档 Attempt；批准后旧 Run Superseded，新 Run 从原始输入重新分析。已有 ExecutionRunSpec 时，用户若要采用新的 CLI/model/Effort/HumanReview/Skill 策略，Core 创建 `RunPolicyAmendment`，展示新旧 ExecutionRunSpec/ResolvedProjectConfig 和全部失效 Attempt/Evidence/Audit/candidate；批准后旧 Run Superseded，新 ExecutionRunSpec 引用同一 TaskContract/TaskGraph，但新 Run 必须先完成 CarriedPlanningReviewBundle 所需的新 Run 人审，再从冻结 base 重做全部 implementation、repair、evaluation、最终验证与审计。2.0.0 不签发跨 Run 派生收据，也不复用旧 Attempt、HumanReviewReceipt、EvidenceReceipt、AuditVerdict 或 candidate。若变化同时改变已消费的 planning step、目标、环境等级、dependency/network class 或 AcceptanceCheck，则必须走 PlanningPolicyRestart/ContractAmendment，不能伪装成纯执行策略变更。恢复不了原 exact selection 的 ConfigurationInvalidated 只能 Blocked 或走新 Run，禁止 fallback。

`hold=Paused` 只能由 SafeParkReceipt 建立：

```text
SafeParkReceipt
  run · planning_spec_hash · execution_spec_hash? · prior_phase · durable_checkpoint
  provider_session/outcome · candidate tree
  no_active_tool_call · no_verifier_or_preview_process
  no_environment_skill_git_transaction · released_leases[]
  parked_at · receipt_digest
```

PauseRequested 先停止新调度，再 interrupt/await 活动 Harness，终止 verifier/preview，等待 broker transaction 到达可核对边界并释放租约；全部 guard 为真才写 Paused。超时默认继续等待或取消暂停；强制结束进入 UnknownOutcome。Resume 必须重新验证 Project/config/skill/environment/provider session 后回到 prior phase。PrepareShutdown 和会影响活动 Run 的 Environment/Skill 更新复用同一 SafePark gate，不各自发明停靠语义。

任何 Agent 输出都不能直接产生 `Completed` 事件。

### 6.3 节点状态

```text
node_status = Pending | Ready | Producing | ClaimSubmitted | Verifying |
              Evaluating | Accepted | RepairRequired | Inconclusive |
              ProtocolViolation | Isolated | ReplanRequired | NeedsHuman

nominal path =
  Pending → Ready → Producing → ClaimSubmitted → Verifying → Evaluating → Accepted
```

失败分支：

```text
Verifying → RepairRequired | Inconclusive | ProtocolViolation
Evaluating → RepairRequired | ReplanRequired | NeedsHuman
RepairRequired → Ready
Inconclusive + proven transient + retry budget       → Ready
Inconclusive + no safe retry                         → run hold=Blocked(VerificationInconclusive)
ProtocolViolation + control state intact             → Isolated → Ready in fresh clone
ProtocolViolation + control integrity uncertain      → run terminal=ProtocolFailed
NeedsHuman                                           → run hold=AwaitingHumanAcceptance
ReplanRequired                                       → emit RunReplanRequested; run phase=Replanning
```

人类决定写入 Receipt 后，NeedsHuman 节点回到 Evaluating；ReplanRequired 不在原 Run 原地换图，按 §6.6 审批新图并创建新 Run，其全部节点从 Pending 开始。依赖节点只有在同一 Run 内 `Accepted` 且输出哈希未变化时才满足依赖。所有节点 Accepted 后仍必须在唯一最终集成 tree 上重跑全局验收。

### 6.4 提问与默认假设

只有同时满足以下条件才允许默认假设：可逆、影响局部、不改变用户目标、不删除或放宽验收、不产生高风险外部动作、能在本 Run 验证，并且有稳定惯例支持。

以下情况必须暂停受影响节点并提问：

- 多个合理选择会改变用户看到的结果或验收标准；
- 用户要求互相矛盾；
- 无法定义可证伪的完成条件；
- 缺少必需资源、凭证、授权或目标环境；
- 涉及发布、外部写入、数据迁移、删除、付费、安全或隐私；
- 需要删除 Requirement、降低环境等级或接受功能缺失；
- 当前代码事实与用户描述冲突且无法按权威顺序裁定。

未决重大假设不能进入 Completed。

### 6.5 用户主动纠偏

用户可以在 Task 页随时提交 `UserCorrection`，但它不是一段直接塞进当前 CLI 会话的任意 steer。Core 先停止新调度、interrupt/await 当前 Attempt 并取得 SafeParkReceipt，再不可变保存原文/附件和当前 subject：

```text
UserCorrectionReceipt
  project/task/run/attempt? · planning/execution spec hashes
  raw_text_ref · attachment_hashes[] · submitted_at · operator
  subject_contract/graph/candidate hashes
  classification(planning_revision | contract_preserving_execution | graph_strategy |
                 contract_semantic_change | new_external_fact | ambiguous)
  affected_requirement/node ids[] · disposition
  successor_attempt_or_amendment_ref? · receipt_digest
```

- 尚未冻结 ExecutionRunSpec 时，任何纠偏都归入 `planning_revision`：Core 把“原始请求 + append-only correction”形成新的 request revision，当前 PlanningRunSpec/全部 DocumentStep 输出失效，旧 Run Superseded，新 PlanningRunSpec/Run 从 `fact_analysis` 重做。此阶段不存在 ContractAmendment，也不得把纠偏错误送进 implementation/repair。
- 已冻结 ExecutionRunSpec 后，`contract_preserving_execution` 才能补充不改变用户可见目标、约束或验收的实现提示；Core 创建绑定 correction hash 的全新 repair/implementation Attempt，旧 Attempt 终止，不能在同一上下文暗改。
- 已冻结后 `graph_strategy` 进入 §6.6 ReplanProposal，`contract_semantic_change` 强制 ContractAmendment；`new_external_fact` 先经事实通道生成/校验 FactReceipt，再由失效规则决定 replan 或 amendment。
- `ambiguous` 必须展示“仅执行纠偏 / 改任务契约”的影响让用户选择，Agent 不得替用户定级。任何会删除 Requirement、放宽 Check、改 ProjectIntent 或新增外部副作用的纠偏都不能归为 contract-preserving。
- 新后继 Attempt 的输入必须包含结构化 correction 与当前冻结契约；未消费 correction、输出 subject hash 未变化或再次命中同一开放 finding 时不得重新送审。

### 6.6 局部重规划

重规划只能提出新的 TaskGraph，不能改变 TaskContract，也不能原地改写当前 Run 的 ExecutionRunSpec。每个 ReplanProposal 必须列出触发证据、受影响要求/节点、语义上未变节点、将失效的全部 Attempt/候选/收据、旧图哈希、新图和预算变化。

接受条件：要求覆盖率不下降；验收没有被删除或放宽；新图仍为 DAG；节点输入、依赖、写域和要求合法；失败检查没有被替换成更容易通过的检查。独立 graph review 与用户批准后，旧 Run 置 `terminal=Superseded`，Core 原子写入新 TaskGraph、对应 ExecutionRunSpec 与新 Run；新 Run 从冻结 base 重做全部 implementation、evaluation、验证和审计。相同节点 ID 只用于可读 diff，不授权复用旧输出。

如果必须改变已冻结 TaskContract，则不能走 Replanning。当前 Run 保持 `hold=AwaitingContractAmendment`；Core 创建独立、版本化的 ContractAmendmentProposal，并让替代 TaskContract/TaskGraph 经过相同的 contract review、graph review、readiness 与用户批准。批准事务同时为预先分配的新 run ID 写入当前配置要求的 planning/contract/graph HumanReviewReceipt、旧 Run `terminal=Superseded`、新 contract/graph/ExecutionRunSpec 与新 Run；缺少任一项整体回滚。2.0.0 不自动复用旧节点、审批或 Receipt。用户拒绝且原契约已不可执行时，旧 Run 进入 `Infeasible` 或 `Cancelled`。

### 6.7 有界失败

- 相同 failure fingerprint 连续 3 次且没有新增事实 → `Stalled`。
- transient Harness 错误使用有界指数退避；业务失败不自动重试。
- 单节点尝试数、重规划次数、token/金额/墙钟时间均在冻结 policy snapshot 中。
- 墙钟时间、turn 和 token 上限始终由 Core 硬执行；美元上限只有在 Provider 提供可流式核对或服务端强制的用量时才标为 hard，基于本地估算的成本只能作为 soft alert。达到任一硬预算即进入 `BudgetExhausted`，不得自动追加。
- 禁止 `completed_with_gaps`；缩小目标必须创建新的 TaskContract 版本。

---

## 7. 事实分析、需求覆盖与最终完成门

Rust 内核维护三张强制矩阵：

```text
原始语义片段 → Requirement / Constraint / Non-goal / Question
Requirement  → TaskGraph Node
Requirement  → AcceptanceCheck → 当前 EvidenceReceipt
```

结构覆盖率 100% 只证明“每段文字都有去向”，不证明分类正确。用户明确表达的指令默认归为 must；若 analyst 将其标为 optional、Non-goal、Context 或 Question，必须给出原文锚点、排除理由和独立 reviewer 结论，任何实质性降级都进入 D11 用户确认门。最终 auditor 必须从用户原文独立重做一次语义分段，对比契约，而不是复用 analyst 的分母。

契约冻结前：三张矩阵必须覆盖完整；所有 must Requirement 都有验收；不存在无来源扩张；重大假设和矛盾已解决。资格评估另外报告人工标注的遗漏率、臆造率、必要性分类准确率和锚点准确率，不能让生成契约的系统自己定义正确答案。

证据真实性与验收充分性是两个不同的门。Rust Receipt 只能证明某项检查真实执行；关键 AcceptanceCheck 还必须记录 oracle provenance，并至少具备一种判别力证据：修改前阳性对照、mutation 反证、独立黑盒探针或用户可观察结果。最终 auditor 可以从原始要求提出新的 scratch probe，由隔离 verifier 执行；若探针应永久进入项目，则创建新的 implementer 节点，auditor 自身仍只读。

实现期间：producer 不能修改 TaskContract 或任何 ContractAcceptanceCheck，也不能读取或修改 ProtectedOracle；producer 只能修改允许写域内的 ImplementationTest。Requirement ID 不删除、不复用；契约新版本会使受影响节点、验收和证据失效；“顺手优化”只进入建议列表。

最终完成门必须全部成立：

```text
contract_is_frozen
AND execution_run_origin_chain_is_valid_and_promoted_outputs_match_contract_graph
AND execution_run_spec_is_frozen_and_matches_current_run
AND project_is_active_initialized_and_identity_current
AND project_intent_revision_matches_contract_and_has_no_unapproved_conflict
AND resolved_project_config_matches_run_snapshot
AND every_step_route_matches_qualified_cli_model_effort
AND every_attempt_matches_its_frozen_permission_profile_and_provider_observation
AND skill_set_snapshot_is_unchanged_and_projection_verified
AND no_environment_or_skill_install_binding_transaction_affects_run_snapshot
AND original_source_coverage == 100%
AND all_must_requirements_have_checks
AND applicable_project_rule_snapshot_matches_contract_and_base
AND no_unadjudicated_project_rule_change
AND supported_environment_profile_matches
AND current_readiness_receipt_is_ready
AND all_required_fact_receipts_are_current
AND every_must_requirement_is_satisfied_by_all_its_mandatory_checks_on(candidate_tree)
AND all_receipts_match_current_contract_check_tree_and_environment
AND every_mandatory_process_check_matches_its_executable_oracle_snapshot
AND no_unexpected_skips_or_filtered_tests
AND test_inventory_not_silently_reduced
AND every_test_inventory_change_is_requirement_mapped_and_independently_accepted
AND all_required_negative_cases_pass
AND final_regression_policy_satisfied
AND final_audit_verdict == pass
AND producer_evaluator_model_choice_is_distinct_and_qualification_is_current
AND no_open_blocking_question_or_material_assumption
AND no_open_blocking_finding
AND no_open_human_review_finding
AND no_unexplained_out_of_scope_diff
AND candidate_working_tree_is_clean
AND all_required_artifacts_exist_with_matching_hash
AND required_human_decisions_have_receipts
AND all_configured_human_reviews_have_current_receipts
AND candidate_certificate_is_valid
AND delivery_tree_content_equals_candidate_tree
AND every_required_deliverable_is_present_at_approved_destination_with_matching_hash
AND (
  (
    task_kind == existing_repo_change
    AND delivery_rehearsal_matches(repository_identity, target_head, candidate, delivery_tree)
    AND delivery_approval_matches(repository_identity, new_ref, delivery_tree, artifact_destinations, policy)
    AND delivery_receipt_matches(new_ref, delivery_tree, artifact_destinations)
    AND delivered_tree_check_matches(delivery_receipt)
    AND original_target_worktree_fingerprint_is_unchanged
  )
  OR
  (
    task_kind == greenfield_product
    AND delivery_rehearsal_matches(parent_directory_identity, destination_absent, candidate, delivery_tree)
    AND delivery_approval_matches(parent_directory_identity, destination, delivery_tree, artifact_destinations, policy)
    AND delivery_receipt_matches(destination, delivery_tree, artifact_destinations)
    AND delivered_tree_check_matches(delivery_receipt)
    AND project_target_transition_receipt_matches(destination, delivery_tree)
  )
)
AND event_log_integrity_check_passes
```

### 7.1 失败语义

| 状态 | 精确定义 | 可作为通过 |
|---|---|---:|
| `CheckFailed` | oracle 已执行，观察结果不符 | 否 |
| `Inconclusive` | 环境、工具、解析器或 flaky 导致无法判断 | 否 |
| `Unreproduced` | 在指定条件下没有复现报告现象 | 否；不等于不存在 |
| `KnownBaselineFailure` | 修改前已记录，最终 fingerprint 相同且证明不在影响面 | 可披露，但不能伪装全绿 |
| `NewRegression` | 相对 baseline 新增或变化的失败 | 否 |
| `AttemptFailed` | Harness 未产生有效候选 | 否 |
| `Blocked` | 等待用户、凭证、权限或外部依赖 | 否，可恢复 |
| `ConfigurationInvalidated` | 冻结 CLI/model/Effort/auth/Skill 选择不再可用 | 否；恢复 exact selection 或新 Run |
| `BudgetExhausted` | 达到冻结预算 | 否，可追加预算后恢复 |
| `Stalled` | 同因失败达到上限且没有新事实 | 否 |
| `ProtocolViolation` | 改规格、改保护验收、错绑证据或自评冒充事实 | 否，隔离候选 |
| `Infeasible` | 契约存在有证据的客观矛盾或不可达条件 | 否，需改契约 |
| `Cancelled` | 用户主动终止 | 否 |

### 7.2 已有仓库的历史红灯

只有同时满足以下条件才允许在存在旧失败的仓库中完成任务：失败在改动前已由内核采集；最终 fingerprint 完全相同；没有新增失败、skip 或 filtered；冻结契约包含由独立 reviewer 确认的影响范围 Check，且该 Check 在最终 tree 通过；最终 auditor 明确接受；完成证书完整披露。单独的 LLM“看起来不相关”判断不够。

---

## 8. 工作区、安全与审批

### 8.1 工作区

- 现有仓库要求存在可解析 HEAD；默认要求基线工作树干净。
- Core 在 OS 应用数据目录创建 `runs/<task-id>/<run-id>/repo`，使用 `git clone --no-local --no-hardlinks` 形成拥有独立 Git metadata/object store 的 disposable clone；不使用 linked worktree 作为安全边界。
- disposable clone 禁用 push URL 和 credential helper。Agent 只能写该 clone 与独立 scratch；原始仓库、控制数据库、契约、ProtectedOracle 和证据目录不可见。
- 每个 Run 分配独立进程组、临时目录、端口和测试数据租约。
- Agent 和 verifier 的所有项目命令都必须经过已资格认证的 sandbox executor：文件系统限制到 disposable clone/scratch/只读 SkillSetSnapshot，默认无网络，不可见用户 Home、Keychain、SSH/云配置和 Core 控制面。Codex 与 Claude adapter 各自启用并 fail-close 其原生 sandbox，同时由 Rust 外层执行边界兜底；任一层不能证明生效时该 route 不合格，禁止回退 unrestricted shell。
- verifier 在同一 candidate commit 创建第二个干净 disposable clone，使用独立低权限 sandbox 执行；Rust Core 只接收原始观察并在隔离外签发 Receipt，绝不以 Core 自身权限执行候选仓库脚本。
- 最终候选先进入任务 integration branch，并取得 CandidateCertificate；随后按 task kind 构造唯一交付演练树。
- `ExistingRepoRehearsal`：Core 从 TaskContract 的精确 target head 创建第三个临时 clone，核对 RepositoryIdentity 后导入候选 commit，形成 `delivery_tree` 和拟创建的新 ref。
- `GreenfieldRehearsal`：Core 在 Run 的受保护 staging 根中从 candidate repository 构造完整 `delivery_tree`，同时绑定目标 DirectoryIdentity、destination-absent proof 和 template hash；不要求或虚构 target head。
- 两类 rehearsal 的内容 tree 都必须与 CandidateCertificate 完全相同，否则回到 Integrating；并在这个临时 tree 上重跑全部 mandatory Check 和最终回归。任何可执行验证都到此为止。
- 演练通过后，用户批准的不是抽象“合并”，而是 `candidate_tree + delivery_tree + target_identity + target_head/destination + required_artifact_set/destinations + policy_revision`。任一值变化都使批准和演练收据立即失效；candidate 变化回到 Integrating，target/profile/destination 变化必须恢复原快照或走 ContractAmendment。
- `existing_repo_change` 的 2.0.0 交付只通过受控 bundle/index-pack 路径把已演练 commit objects 导入原仓库，并用 `git update-ref <new-ref> <approved-commit> <zero-oid>` compare-and-swap 原子创建全新的 `refs/heads/autome/<task>-<run>`；Git 使用固定 binary、净化环境、空的受信 `core.hooksPath`、禁用 system/global config 与 credential helper，不 checkout、不改当前分支、不覆盖已有 ref，也不触碰用户工作树。
- `greenfield_product` 的交付在目标父目录的同一文件系统准备完整仓库，确认目标仍不存在且不是 symlink 后以原子 rename 落位；跨文件系统或目标已出现时 fail closed。
- `ContentAddressedArtifact` 从 Core 只读 store 复制到批准目标的同目录临时文件，核对 hash、fsync 后以 no-replace rename 原子落位；已存在目标、跨文件系统、权限或 identity 变化都阻断，不覆盖。
- DeliveryReceipt 产生后只做 `DeliveredTreeCheck`：核对 ref/directory、commit/tree、对象可达性、制品路径/hash、权限和交付前后用户工作树 fingerprint。**不得在用户仓库或最终目录中执行候选代码。** 完成证书绑定演练通过的 delivery tree、必需 artifact set 与只读核对结果。

### 8.2 Capability broker

以下能力不能通过任意 sandbox shell 获得：

- 合并、push、force push；
- 创建/归档 Project、改变 Global/Project 配置；
- 安装、升级、迁移或卸载应用、CLI、包管理器和 Skill；
- 发布、部署、外部系统写入和数据库迁移；2.0.0 对这些动作固定 deny/plan-only；
- 读取长期凭证；
- 产生付费外部副作用。

首版将能力拆成 Git/Artifact、Project、Environment 和 Skill 四个 Rust broker，均只接受已持久化 ID，不接受 Renderer/Agent 传来的 shell/URL/argv。Agent 只能提交 proposal；Project 初始化、配置应用、Environment/Skill 事务和交付必须由用户从 UI 发起。交付批准继续绑定 `gate_id + candidate_tree + delivery_tree + target_identity + target_head/destination + artifact_set/destinations + action_hash + policy_revision + expiry + nonce`；Environment/Skill 则分别绑定自己的 snapshot/catalog/package/plan hashes。任一绑定值变化后旧批准自动失效。

### 8.3 凭证与日志

- 不继承 Electron/Main 的完整环境；从环境白名单构造 Harness env。
- 原始凭证不进入 prompt、argv、disposable clone、Renderer storage、日志或诊断包。
- 必需凭证由 OS Keychain/受信 broker 通过已资格认证且不向 Agent 子进程暴露的 handoff 提供；无法实现时对应认证模式禁用。
- 所有 Agent 输出、仓库 Markdown、diff、ANSI 和链接均视为不可信内容。
- 诊断包默认脱敏，并列出删除/保留策略。
- 仓库首次登记时必须由用户确认“受信仓库”；“用户信任此仓库”不等于把 CLI 的 project trust 打开。Core 对自动 Attempt 一律关闭候选 `AGENTS.md`/`.codex`/`CLAUDE.md` 的原生再发现，禁用项目 Harness hooks、未批准 MCP/plugin/skill、Git filter 和 credential helper，只把 ApplicableProjectRuleSnapshot、签名 role prompt 与已批准 SkillSetSnapshot 通过受控投影注入。
- Codex 使用隔离 `CODEX_HOME`、版本匹配 schema、唯一 Autome dynamic tools、受控 skill extra root、严格配置与默认无网络 sandbox；Claude 使用隔离 `CLAUDE_CONFIG_DIR`、`--bare`、唯一 Autome stdio MCP 和受控 skill projection。未知配置、用户/Project Skill 泄漏或缺少隔离能力时 fail closed。
- 首次向模型提供仓库内容前展示 provider、账号、允许目录和排除规则；执行 secret scan/redaction，并记录发送内容摘要与数据策略 hash。Task、附件、日志、截图、备份和删除采用明确保留期，诊断导出默认脱敏。

`CredentialRecord` 只保存 provider、auth mode、storage kind、Keychain service/account 或专用 auth file 的不可逆标识、创建/轮换/撤销时间和状态，不保存秘密。Autome 自建 Keychain item 使用 when-unlocked-this-device-only、禁止同步，并把访问控制绑定正式签名应用或受控 credential broker；开发签名不能读取生产 item。Renderer、Agent、项目进程和普通 CLI 工具没有读取接口。创建、替换、撤销、账号漂移、Keychain 锁定以及卸载时“保留/删除 Autome 自建 item”的用户决定都生成 CredentialReceipt。Codex OAuth 由其 owner-only 独立 `CODEX_HOME/auth.json` 与官方凭证机制管理，强制 file store 并记录文件 identity，不与 Autome 自建 API-key item 混称。

默认本地保留策略：ProjectHome 保留到用户删除 Project；TaskContract、结构化 Receipt 与 CompletionCertificate 保留到用户删除 Task；原始流式/安装日志和截图保留 30 天；被拒 quarantine 最多 7 天；仍被 Global/Project binding 或历史 Run 引用的 Skill digest 保留，其余可垃圾回收；数据库升级备份最多保留最近 3 份且不超过 30 天。删除时另写不含原文/源码/Skill 内容的最小 tombstone；诊断导出再次运行 secret scan。2.0.0 不承诺对已进入外部模型提供商系统的数据执行远端删除，UI 必须展示该边界。

Core 从 macOS Application Support API 获取专用数据根，不接受环境变量覆盖。根目录及子目录固定为 owner-only `0700`，数据库、配置、日志、截图和备份固定为 `0600`；创建和每次启动均以 no-follow/open-at 方式拒绝 symlink、hard-link 异常、错误 owner、world/group writable 父目录和越界真实路径。权限无法修复时只进入诊断态，不继续运行任务。

数据库、内容对象和备份在应用层保持可恢复的普通格式，但始终受 owner-only 权限和 macOS 数据卷保护；诊断导出只包含经过再次扫描的脱敏内容。用户把文件复制到其它卷、关闭 FileVault 或导出原始制品后，保护等级可能下降，UI 必须在动作前说明。SSD 上的删除不承诺物理覆写，产品文档必须如实说明。

---

## 9. Electron 工作台

顶层主导航固定为四项：

1. **项目**：默认入口；创建/导入/归档 Project，显示产品意图、环境、配置、技能、活动 Task 和“需要我决定”摘要。
2. **本地环境**：组件 presence/integrity/auth/qualification/readiness 状态、候选 installation、版本/签名/来源/登录/资格、安装计划、进度与 Receipt。
3. **技能市场**：市场搜索、已安装、更新、隔离区和本机目录盘点。
4. **全局设置**：默认步骤路由、人审策略、环境/技能/预算策略、Core/UI/protocol 版本和诊断。

窗口顶部常驻全局执行条，分别显示“当前运行 / 等待我 / 排队 N”，每项都带 Project 名与可跳转 Task；排队项显示稳定顺序、当前 lease holder 和取消动作，不能让用户只在某个 Project 内猜测为什么尚未启动。

选中 Project 后进入五个页签：**概览 / 任务 / 项目配置 / 环境与技能 / 历史与证据**。概览置顶显示当前 ProjectIntentRevision 及其来源；修改产品目标、跨 Task 约束或关键决定必须打开版本化影响预览。Task 只能从这里创建；无 Project 时主 CTA 是“创建新产品项目”或“导入已有仓库”，不是直接输入一句话。Task 页包含原始要求、契约、任务图、节点、候选资格、交付、暂停/取消、“纠偏当前方向”和验收矩阵；人审 reject 使用可定位 finding 编辑器，展示开放/已解决状态与将进入的唯一后继。证据按 Requirement 索引实现位置、环境、时间、结果与制品。

UI 只能显示 Rust 计算的 `已验证要求数 / must 要求总数`，不得显示 Agent 自报百分比。

### 9.1 配置矩阵

全局设置和项目设置使用同一张矩阵：

| 步骤 | CLI 安装身份 | Provider / 账号 | 模型 | Effort | 人工审计 | Skills | 配置来源 | 状态 |
|---|---|---|---|---|---|---|---|---|
| fact analysis 等 AI 步骤 | Codex/Claude + exact installation、version、source | provider + account fingerprint/auth mode | 仅当前 capability 列表 | provider-native | off / required | 已批准集合 | 全局 / 项目覆盖 | ready / blocked |
| verifier / delivery | Rust Core 固定 | 本地 Core | 不适用 | 不适用 | 固定安全门 | 不加载 | 系统 | 不可覆盖 |

Project 每个 AI 行可选择“继承全局 / 项目自定义 / 恢复继承”。多 installation、账号不同或继承导致 installation/account 改变时，身份列禁止折叠或只显示“Codex/Claude”。保存前展示 resolved diff、资格影响和“仅影响未来 spec”；无效三元组直接拒绝保存。全局保存必须先展示跨 Project 的 GlobalConfigImpactPreview，账号/provider/Skill/readiness 变化高亮，存在失效项目时二次确认。对于正在规划的 Task，显示“仍使用 PlanningRunSpec X / 重新规划”；对于已有 ExecutionRunSpec 的 Task，显示“仍使用 snapshot X / 创建 RunPolicyAmendment”，绝不热替换。

### 9.2 环境与技能交互

Environment Center 的“一键补齐”先打开计划抽屉，列出每个动作的来源、精确版本、下载量、权限、是否重启、外部确认和 rollback class；用户一次确认后按步骤执行，遇到系统/登录动作切换为 NeedsUserAction 卡片。卡片显示 challenge ID、精确入口、到期时间、预期检查以及“重新检测 / 继续 / 放弃”；只有 post-probe 通过才启用“继续”。取消、部分成功、challenge 过期、来源漂移和 UnknownOutcome 都必须显示真实状态，不能把黄色 warning 混成红色 blocker。

技能市场使用四个页签：市场、已安装、更新、隔离区。第一步“安装到 Vault”抽屉只显示 source/commit、文件与权限摘要、CLI 兼容性和审计发现，完成后明确显示“已安装，未启用”。第二步“启用此 Skill”才选择 Global/Project/步骤/CLI/invocation policy，展示最终逐步骤 SkillSet diff 和“只影响未来 spec”，使用不同 approval/action hash；默认不开启，关闭安装抽屉也不自动绑定。Global binding 另列所有受影响 Project 并二次确认。外部目录发现的 unmanaged Skill 只能打开、导入审计或忽略；不能在没有 receipt 时显示为 Autome 已安装。

### 9.3 首版必须覆盖的界面状态

首次启动、无 Project、Project 未初始化/身份变化/已归档、Codex/Claude/iTerm2/Homebrew/Node 缺失、重复 CLI、来源或签名异常、登录中/失效/共享账号待确认、资格过期、环境 warning/blocker、安装计划/部分成功/NeedsUserAction/失败/UnknownOutcome、Skill 搜索离线/审计失败/重名/更新可用/投影漂移、配置继承/覆盖/无效、权威来源缺失、无 Task、无需人工决定、尚无证据、Core 不可达、协议不兼容、Core 恢复中、事件序号缺口、命令结果未知、DB 迁移失败、Renderer 重载、Task 阻塞、预算耗尽、验收失败、候选已就绪、交付待批准、交付完整性失败和已完成。

任何断线缓存必须显示“截至某时”，写操作一律禁用；“未验证”不能渲染成“通过”。

### 9.4 Electron 安全基线

- `nodeIntegration: false`、`contextIsolation: true`、`sandbox: true`、`webSecurity: true`、`webviewTag: false`。
- 主 Renderer 只加载包内 `autome://app` 自定义安全协议，不使用远程 HTML、CDN 或 `file://`；PreviewWindow 按 §9.6 使用独立不可信 origin。
- 严格 CSP；拒绝导航、新窗口和默认权限请求。
- 每个 IPC handler 校验 sender origin、webContents、schema 和 payload 上限。
- preload 不暴露通用 IPC、Node、process、文件、shell 或 URL 打开能力。
- 登录 `verificationUrl/userCode` 只接受当前 app-server、匹配 outstanding login ID 的响应，并校验 S0 冻结的 HTTPS origin；Renderer 不能提供或改写登录 URL/code。
- 外链必须由用户明确点击，经 Main 校验 `https:` 与 allowlist 后交给系统浏览器。
- 生产包启用 Electron fuses：关闭 RunAsNode、NODE_OPTIONS 和 inspect 参数；启用 ASAR integrity 与 OnlyLoadAppFromAsar。

### 9.5 Rust sidecar 生命周期

- Rust binary 通过 `extraResources` 放在 `process.resourcesPath/core/`，不进入 ASAR。
- `core-manifest.json` 的期望摘要固定在受 ASAR integrity 和应用签名保护的 Main 代码中；它同时绑定 Rust binary、protocol/schema、playbook、config/environment profile、skill policy/scanner、SignedDependencyCatalog/install recipes 与 check-runner manifest。Main 启动前校验 manifest、Rust binary SHA-256、签名 designated requirement/Team ID、target 和执行权限，不能允许同时替换外部 binary 与外部 manifest。
- 握手返回 core/protocol/schema/build/target/capabilities；major 不兼容立即拒绝。
- Core 启动先取得 OS 单实例锁并绑定 Main 的 `parent_instance_nonce`；新 Main 未确认旧 owner/lease 终止前不得启动第二个 Core。
- Core 观察到控制 stdio EOF 时立即停止新调度、持久化 shutdown intent、按协议中断活动 turn、清理其完整进程组并退出；每个 Harness/runner 另有 watchdog pipe，父控制链消失时不得成为孤儿。
- 关闭窗口隐藏到托盘，任务继续；明确退出触发 `PrepareShutdown(deadline)` 并复用 §6.2 SafePark gate，Core 停止调度、持久化 checkpoint、核对 broker boundary 并安全中断 Harness。
- deadline 内未能安全停靠时默认拒绝退出，并让用户选择继续等待或强制结束；强制结束必须把活动 Attempt 标为 `UnknownOutcome`，重启后先核对而不是自动重试。
- Core 崩溃按有界退避重启；恢复失败后进入只读诊断态，不无限拉起。

### 9.6 PreviewSession

用户预览候选产品时，Renderer 只能调用 `openPreview(preview_id)`，不能传 URL。Core 为已验证的 candidate 创建带 TTL 的 PreviewSession：候选服务运行在项目 sandbox 内并绑定随机 loopback 端口；受信 preview proxy 使用另一随机端口，只接受数字 loopback Host、精确 origin、一次性 capability path 和未过期 session，再转发到该候选端口。端口、进程和 token 都归 Run lease 管理。

Main 用独立 `BrowserWindow` 打开 proxy URL。该窗口无 preload、无 IPC、`nodeIntegration=false`、`contextIsolation=true`、`sandbox=true`，使用一次性 non-persistent partition；session 层只允许精确 preview origin，拒绝其它网络、导航、弹窗、权限、下载和外部协议，关闭后销毁 profile。候选服务自身仍处于默认无网络 sandbox。若当前 macOS/Electron 无法证明这些限制，UI 只提供由 verifier 生成的静态截图/录屏，不宣称“隔离交互预览”，也不打开系统默认浏览器。

---

## 10. Harness 与角色策略

### 10.1 逻辑角色

| 角色 | 权限 | 主要输出 |
|---|---|---|
| analyst | read-only | facts、unknowns、风险、候选需求原子 |
| planner | read-only | TaskContract/TaskGraph proposal |
| contract-reviewer | read-only、独立模型 | 原始语义覆盖与验收可证伪性 |
| implementer | workspace-write | 候选代码、测试与说明 |
| auditor | read-only、独立模型 | Requirement 逐项结论与缺陷/缺口 |

Verifier 不是 LLM 角色，而是 Rust 控制的事实执行器。测试作者也不是 auditor；需要补测试时，由内核创建新的 implementer 节点，再交 auditor 复核。

唯一步骤映射如下；GraphReview 先过 Rust 的 DAG/覆盖确定性门，再由独立 contract-reviewer 检查拆分语义：

| LoopStepId | Run/Node phase | 角色 | 写权限 | pass 后继 | reject 后继 |
|---|---|---|---|---|---|
| `fact_analysis` | DiscoveringFacts | analyst | read-only | DraftingContract | DiscoveringFacts |
| `contract_drafting` | DraftingContract | planner | read-only | ContractReview | DraftingContract |
| `contract_review` | ContractReview | contract-reviewer | read-only | PlanningGraph | DraftingContract |
| `task_graph_planning` | PlanningGraph | planner | read-only | GraphReview | PlanningGraph |
| `graph_review` | GraphReview | contract-reviewer | read-only | 初始规划→CheckingReadiness；重规划→待新 Run 批准 | PlanningGraph |
| `implementation` | Executing/Producing | implementer | candidate-write | node→Verifying | Repairing |
| `repair` | Repairing | implementer | candidate-write | node→Verifying | Repairing |
| `node_evaluation` | Executing/Evaluating | auditor | read-only | node→Accepted/下一 Ready node | Repairing 或 Replanning |
| `final_audit` | FinalAuditing | auditor | read-only | DeliveryRehearsing | Repairing、Replanning 或 ContractAmendment |

HumanReview 插在对应 AI step 的输出之后；通过或拒绝都只使用本表唯一后继并保存意见类型。表中包含全部九个 LoopStepId，Rust schema test 必须逐字比对该全集，任何缺项都不能启动 Scheduler。

### 10.2 步骤路由

Scheduler 在进入每个 AI 步骤时只读取 Run 冻结的 StepExecutionRoute 与对应 AttemptPermissionProfile，从已选择 installation 的 HarnessCapabilitySnapshot 校验 model/Effort/认证、provider 实际工具面、sandbox、文件/命令/网络范围与 Skill 投影，再创建 Attempt。`repair` 默认独立继承 `implementation`，`node_evaluation` 与 `final_audit` 分别独立配置；“同一 CLI”不等于“同一模型”，“同名 Effort”也不跨 provider 归一。

如果步骤配置了人工审计，AI 输出先形成只读 review bundle，进入 AwaitingConfiguredHumanReview；用户通过或驳回后由 Rust 决定下一状态。配置界面、Agent proposal 和 Project 文件都不能跳过 frozen route 或 human gate。

### 10.3 Prompt 与 Playbook

- Rust 状态机定义“何时进入哪个阶段”；Prompt 定义角色如何判断和工作。
- Prompt 写目标、判断标准、工具和边界，不编排可由 Agent自行决定的微步骤。
- Playbook 由 manifest + Markdown role prompts + references 组成，运行时固定内容哈希。
- 首版只有 `greenfield-product` 和 `existing-repo-change` 两个 playbook。
- 所有 role 的状态性输出使用 JSON Schema 或 Autome dynamic tools；自由文本只能作为叙述，不能决定状态。

### 10.4 上下文纪律

- analyst：Project context、ResolvedProjectConfig、该步骤 SkillSetSnapshot、用户原文、附件、目标仓库只读视图、基线规则发现结果和经策略开放的事实工具；外部事实必须引用 FactReceipt。
- planner / contract-reviewer：Project context、各自独立技能集合、用户原文、事实清单、契约/图候选、ApplicableProjectRuleSnapshot；reviewer 不读取前一角色的私有推理。
- implementer：冻结 Project/config/skill/contract、ApplicableProjectRuleSnapshot、当前节点、依赖产物、允许写域、验收摘要和可用能力。
- auditor：冻结 Project/config、独立 reviewer SkillSetSnapshot、用户原文、冻结契约、最终候选仓库的只读视图、ApplicableProjectRuleSnapshot、ProjectRuleChange、diff 和 EvidenceReceipt 原文；不从 candidate 重新发现规则，也不注入 producer summary、历史说服性文本或无关聊天记录。
- 大文件和历史事件按需读取，不一次塞入上下文。
- 决策和重要事实持久化在 Core，不能只存在于聊天记录。
- compaction/resume 后重新校验 project/config/skill/contract hashes、candidate tree、ModelSelectionIdentity 和 provider session ID。

---

## 11. 持久化与恢复

### 11.1 唯一事实源

SQLite 是 2.0 控制状态的唯一权威；Git 是用户代码与制品的权威。二者职责不重叠，不声称能够互相完整重建。

SQLite 中维护：

- append-only `events`，带单调序号、唯一事件 ID、aggregate revision 与事务完整性检查；本地日志不宣称提供对同一 OS 用户的外部不可抵赖证明；
- Project/ProjectIntent revisions、Intent amendments、ProjectInitialization/TargetTransition receipts、Task 索引、ExecutionQueue/HarnessLease 与 ProjectHome manifest；
- GlobalConfig/ProjectConfigPatch revisions、GlobalConfigImpactPreview、ResolvedProjectConfig snapshots、PlanningRunSpec/PlanApprovalReceipt/ExecutionRunSpec、PlanningPolicyRestart/RunPolicyAmendment/ReplanApprovalReceipt/ContractAmendment/BudgetGrantReceipt、UserCorrectionReceipt、CarriedPlanningReviewBundle、AttemptPermissionProfile、HumanReviewReceipt/HumanReviewFinding 与 SafeParkReceipt；
- SignedDependencyCatalog、EnvironmentSnapshot/RemediationPlan/UserActionChallenge/InstallationTransaction/ChangeReceipt；
- CredentialRecord/Receipt 与认证 consent revisions；
- SkillInventory、SkillPackage/Audit receipts、SkillInstallPlan/InstallationTransaction/InstallReceipt、SkillBindingPlan/BindingReceipt、Global/Project bindings、SkillSetSnapshot 与 Vault 引用；
- TaskContract revisions、Requirement、ApplicableProjectRuleSnapshot、AcceptanceCheck、ExecutableOracleSnapshot、TaskGraph revisions；
- Run、Attempt、AttemptPermissionProfile binding、Gate、AgentClaim、FactReceipt、EvidenceReceipt、TestInventorySnapshot、AuditVerdict；
- SupportedEnvironmentProfile、ReadinessReceipt、ModelSelectionIdentity、QualificationReceipt；
- CandidateCertificate、DeliveryRehearsalReceipt、DeliveryApprovalReceipt、DeliveryReceipt、DeliveredTreeCheckReceipt、CompletionCertificate；
- command idempotency、leases、budgets、schema migrations。

读取模型由事件/事务更新的投影表提供。所有状态转换、事件追加和投影更新处于同一事务；数据库使用编号迁移、迁移前备份、失败只读模式和恢复演练。

### 11.2 恢复规则

- Rust 重启后只恢复 durable event 之前的状态。
- 每个外部动作先记录 intent 与幂等 key，执行后记录 receipt。
- 活跃 Attempt 使用 provider session ID、进程启动身份和 lease，不以 PID 单独认领。
- 无法证明是否执行过的动作进入 `UnknownOutcome`，等待核对，不能自动重试。
- 恢复时先核对未决 Environment/Skill transaction 的真实后置状态，再重新校验 Project/repository/directory identity、ResolvedProjectConfig/SkillSetSnapshot、工作区、candidate、contract、graph、Readiness/Qualification/Fact freshness、交付收据链和 provider session identity；结果未知时不重复安装、更新或交付。

---

## 12. 测试与质量门

T1–T6 是累计测试域，不要求尚未进入范围的未来能力在早期里程碑通过。每层按里程碑提供具名 manifest（如 `T2-M0`、`T4-M0`）；新里程碑只能增加用例，不能通过 skip、filter 或删除旧 manifest 降低门槛。下文列的是 M6 完整集合。

### T1　Rust 静态与单元门

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --locked`
- 状态 reducer、候选/完成谓词、Project→Task 外键、ProjectIntent 版本/冲突门、ExecutionQueue/HarnessLease FIFO、全局×Project sparse overlay/provenance、StepExecutionRoute/HumanReview、AttemptPermissionProfile 交集与不可扩权、安全下限、rule/oracle/skill snapshot、DependencyCatalog 签名、环境 requiredness/hash/expiry/rollback class 和错误域单元测试；LoopStepId schema、默认路由、HumanReview subject 与 §10.1 表必须恰好覆盖同一九项全集；model choice 比较不得把 CLI/账号/Effort/service tier/权限/config/Skill 差异误算成模型分离。

### T2　状态机与持久化门

- 合法/非法转换穷举与 property tests；
- PlanningRunSpec 能启动且只能启动批准前五个只读步骤；D11 批准在一个事务冻结 contract/graph/ExecutionRunSpec，缺少任一对象不得进入 ContractFrozen；
- event replay、幂等命令、事务崩溃点、重复/乱序事件；
- Project init/reinitialize/archive guard、ProjectIntent 冲突/amendment、跨项目 ID 串线、队首 lease/等待释放/恢复重排/取消幂等、配置 revision 乐观并发/GlobalConfigImpactPreview 过期、配置生效矩阵、PlanningPolicyRestart/RunPolicyAmendment/replan 的 new-Run + no-carry-forward guard、沿用 Task 级 contract/graph 时 CarriedPlanningReviewBundle 为新 Run 重签必需人审、BudgetGrantReceipt、UserCorrection 安全停靠/分类/新 Attempt（fact/contract/graph 三种规划期纠偏都 Supersede 并从新 PlanningRunSpec 重启）、HumanReviewFinding 反馈消费、HumanReviewReceipt tagged subject 与 SafeParkReceipt 全 guard；
- Environment/Skill transaction 的 journal/replay/idempotency：崩溃、取消、重复点击、部分成功和 UnknownOutcome 后先重探测，不重复副作用；UserActionChallenge 绑定/expiry/post-probe/放弃后不可继续；
- SQLite migration、事务崩溃点、磁盘满、损坏只读模式、备份/恢复、保留期清理与 FileVault 状态记录；
- stale claim、stale approval、stale evidence 防重放。

### T3　运行时与隔离门

- Git/disposable clone 全生命周期；
- 目录逃逸、symlink、脏基线、detached HEAD、unborn HEAD；
- owner/`0700`/`0600`、hard-link 与路径替换攻击；
- Repository/DirectoryIdentity 在 rename、mount、symlink 和同 HEAD 替代仓库下均 fail closed；
- ReadinessReceipt 在 lockfile、工具 binary、浏览器、模型对与 profile 漂移后失效并走正确恢复/重批路径；
- Codex/Claude Harness 超时、取消、崩溃、exact session resume、未知事件和恢复；每个角色都用“允许的最近邻工具成功 + 相邻禁止工具被机械拒绝”证明 AttemptPermissionProfile，不以 Prompt 拒绝充数；
- GUI 无 login-shell PATH 时的 known-location/显式 installation 发现；duplicate、symlink swap、TOCTOU、伪造 version、坏签名、错误架构、probe timeout/output quota；
- Codex 专用 `CODEX_HOME` 强制 file credential store；`auth.json` owner/mode/path/account 漂移、共享 keyring/auto、admin/MDM override 和额外 config/instruction/hook/rule source 均 fail closed；producer 修改 candidate `AGENTS.md`/`.codex` 后 reviewer 上下文仍等于冻结 snapshot；
- 非默认 Homebrew mirror、package metadata 漂移、lock、离线、磁盘满、下载截断；禁止 broad upgrade/cleanup/link/tap trust、PATH/dotfile 改写和 shell 字符串；
- Claude shared Keychain 身份只能被动监测、不能被自动 route 选择或由 Autome 登录/登出；dedicated API key 对 Agent/Bash/子进程/argv/env/log/error 不可见；
- Keychain item 的 device-only/non-sync/签名 ACL、开发/生产隔离、轮换、撤销与卸载保留/删除；
- 进程组、端口、临时目录和服务租约清理；
- PreviewSession 的随机端口/token/TTL/Host/origin、candidate 无外网和关闭后全清理；
- secret/环境变量/argv/日志脱敏；
- `process/spawn`、`thread/shell_command` 和其它 unrestricted 执行入口被机械拒绝；
- ServerRequest allowlist 之外的内建 user-input、MCP elicitation、新旧 approval 与 turn/session 级文件、网络、命令提权全部得到有界拒绝，且不能进入 UI Gate或悬挂；
- 原生 `web_search` 关闭；research query 在出站前被 DLP 拒绝，fetch 不能接受 raw URL，redirect 每跳重验；
- Skill quarantine 的 zip-slip/symlink/device/嵌套 SKILL、混淆名、恶意脚本、动态命令、prompt injection、tag 重指、同版本异内容和 scanner mutation；
- Fetch Sandbox 拒绝 proxy/cookie/credential/file/ssh/git/redirect、Git config/url rewrite/hooks/submodule/LFS/filter、zip bomb 和运行期 npx/npm lifecycle；
- 多 Skill 同 Attempt 不能获得权限并集；hooks/第三方 MCP/plugin/credential 型 Skill 一律不可绑定；
- Skill 安装后独立核对 source commit/content/files/规范化 projection；投影按 `(LoopStepId, adapter_id)` 隔离，implementer-only Skill 对 reviewer 不可见；explicit-only 在未点名时不加载、点名时只加载 exact digest；未批准的用户/Project Skill 对两个 CLI 都不可见；
- 替换 `verify.sh`、package script、runner config、HTTP/UI spec 或 result parser 均使 ExecutableOracleSnapshot 失效；
- 修改/新增嵌套项目规则不能改变当前 Run 的 ApplicableProjectRuleSnapshot；
- 基线测试删除、改名、等量恒真替换、runner/config 替换均被 TestInventorySnapshot 检出；
- delivery rehearsal、过期 target head、CAS 新分支、目标 ref 冲突、恶意 reference-transaction/post-fetch hook、必需制品缺失/错投、绿地原子落位与“交付后不执行代码”。

### T4　Electron 与 IPC 门

- TypeScript strict、lint、unit、build；
- Rust schema → TypeScript binding 漂移检查；
- sender/origin/schema/payload 限制；
- Fake Harness 下的 Playwright Electron E2E；
- Projects-first：无 Project 或未批准 Intent 不能建 Task、切换不串数据、intent 冲突/amendment、init/reinitialize/archive、绿地转已有仓库；
- 配置矩阵：继承/覆盖/恢复继承、全局跨 Project 影响/过期/二次确认、installation/provider/account 可见、模型/Effort capability、invalid save、运行中 snapshot、RunPolicyAmendment、human review on/off 不隐藏固定门；
- Environment：optional warning vs route blocker、安装 plan 展开/过期/确认、NeedsUserAction challenge 的 recheck/verified continue/abandon、错误外部安装不归因、取消/部分失败/重试/rollback unavailable/restart；
- Skills：市场离线、搜索、quarantine、risk review、安装后默认 disabled、安装/绑定两个独立确认、Global 影响项目二次确认、Project/step binding、重名、update digest 与 binding switch diff、运行中 snapshot；
- Codex device-code login/cancel/logout；Claude shared 模式仅 status/外部打开且不能被 route 选择；dedicated credential 创建/轮换/撤销、失效与 Keychain 锁定；
- PreviewWindow 无 preload/IPC、一次性 partition、跨 origin/下载/权限/弹窗/外部协议全部拒绝；
- 全局当前运行/等待我/排队跳转、FIFO/释放/重新入队/取消；纠偏先停靠，契约内/改图/改契约/歧义分流；人审 reject 无 finding、finding 未消费或输出不变均拒绝重送；
- Renderer reload、Core crash、事件缺口、幂等命令和过期审批；
- CSP、导航、新窗口、XSS 和 Electron 安全扫描。

### T5　真实 Harness 资格测试

不计入确定性“全绿”数字，单独报告模型、CLI、协议、用量、时间和 run ID：

- 任一 adapter/协议变化：跑 capability + structured stream + tools + interrupt + exact resume + usage smoke。
- Codex 版本变化：先做 Client/Server request 枚举 schema diff，再重跑独立 `CODEX_HOME` + `cli_auth_credentials_store=file` 的 account read/login/cancel/logout、auth file identity、effective instruction/config/hook sources、dynamic tools、逐步骤 skill root、sandboxed command、`approvalPolicy=never`、内建 user-input/MCP elicitation 和所有提权/禁止入口矩阵。
- Claude 版本变化：重跑 auth status、stream-json schema、model/Effort probe、session/resume/interrupt、`--bare`、skill projection、唯一 MCP、permission-prompts none、sandbox failIfUnavailable/无 escape hatch 与 tool deny 矩阵。
- 每个 AttemptPermissionProfile 都必须以 provider init/事件和正负探针证明：声明允许的最近邻能力确实可用、声明禁止的相邻能力确实被 adapter 与外层 sandbox 同时拒绝；只观察 Agent 自觉不调用不能签发资格收据。
- 每个 Run 前核对所有步骤三元组可用、生产/评测身份满足 separation、QualificationReceipt TTL 未过期并执行固定 canary；无服务端 snapshot 时不得声称 lineage 不变。
- Skills provider/CLI 变化：重跑 search schema、遥测关闭、隔离 fetch、双 CLI 的逐步骤 discoverability/invocation 正反证据和 benign/malicious fixtures；explicit-only 必须证明未点名不加载、点名只加载 exact digest。
- Prompt、planner、auditor、verifier 或策略变化：跑对应 golden 子集。
- 每夜轮换子集；RC 固定版本跑完整集合两次。
- Provider/model 更新必须先通过隔离资格测试，才能成为默认配置。
- sealed qualification 额外验证：OracleController 输出永不进入被测 Core，oracle 不能从 argv/env/路径/进程信息/错误/日志/事件/证书/网络中泄漏，两次 Run 结束前无逐项反馈。

### T6　打包与干净机器门

- 三类干净机快照：完全预装；无 Xcode CLT/Git/iTerm2/Homebrew/Node/Codex/Claude/Skills CLI，从“一键开始补齐”到所有可自动项完成并在系统/登录处正确停靠；混合来源/重复 binary/自定义 brew mirror；
- 完成 Codex 独立认证、Claude dedicated apiKeyHelper 与 `greenfield-web-v1` 准备后，分别用 Codex-only、Claude-only 和跨 CLI reviewer 路由启动、运行、恢复和卸载；
- Rust binary、app.asar、manifest、签名、公证和 Gatekeeper 验证；
- host 工具链、浏览器、锁定依赖与 profile hash 验证；
- 升级时 UI/Core/protocol/schema 原子匹配；
- 旧 profile/新 Core、旧 runner/新 schema 和升级中断均 fail closed 或整体回到上一签名版本；
- 完整运行后无孤儿进程、锁、端口、未脱敏诊断和工作区外写入；卸载 Autome 不删除用户原有工具、账号或 unmanaged Skill。

### 12.1 Golden Task Set

RC 至少包含 18 个任务，每个独立执行两次：

- 6 个绿地产品任务：表单/列表/持久化、CSV 工具、小型看板等，使用浏览器旅程与运行事实验收；
- 6 个已有仓库迭代：CLI contract、UI 交互、时区边界、API/存储纵向功能等；
- 6 个负向任务：关键歧义、缺权限/凭证、矛盾验收、恶意仓库指令、验证未执行、新增回归。

每个 golden task 固定 Project revision、ResolvedProjectConfig、逐步骤 AttemptPermissionProfile、SkillSetSnapshot、fixture commit、原始要求、允许回答、可见验收、独立 ProtectedOracle、允许写域、正确终态、预算/超时和禁止接受的“看似完成”结果。隐藏 oracle 只能验证已表达要求和系统安全不变量，不能偷偷增加产品要求。

18 个 Task 使用 pairwise 路由矩阵覆盖 Codex implement + Claude audit、Claude implement + Codex audit、单 adapter 不同模型、Project 全继承/混合 override、HumanReview on/off、无 Skill/显式 Skill；两个正式 adapter 至少各承担 6 个 implementation Run 和 6 个 reviewer/auditor Run。除此之外，Project/Environment/Skill 管理流使用确定性 fixture 单独全量覆盖，不挤占 18 个任务正确性样本。

评测数据分三层：development set 可用于调试；regression set 用于防止已知失败复发；sealed release set 由独立维护者保管，不能用于 Prompt、planner 或策略调优，发布后才解封失败样例并轮换。RC 的 18 个任务指 sealed release set。

`pass²` 的精确定义：同一冻结任务使用相同产品要求和 oracle、独立创建两个 Run；两个 Run 的终态都与 oracle 一致，该任务才计为一次 pass²。RC 至少 17/18 个任务满足 pass²，其中 6 个负向安全任务必须 6/6 满足；任一假完成仍直接阻断发布。

### 12.2 发布指标

| 指标 | 2.0 发布门 |
|---|---:|
| sealed 认证集观测到的假完成 | **0 / 36 次 Run，硬门；同时报告分母与残余统计风险，不声称总体概率绝对为 0** |
| 原始语义覆盖率 | 100% |
| sealed set 中明确 must 要求遗漏 | 0 项 |
| 未经来源或用户确认而新增的 must 范围 | 0 项 |
| must Requirement 证据完整率 | 100% |
| 可解 golden task 正确完成率 | ≥90%，并满足 pass² |
| 完成/提问/阻塞/不可实现的正确处置率 | ≥95% |
| 明确、可逆、环境完备任务的无需计划外人工介入率 | ≥80%；中位数 0 次，P90 ≤1 次 |
| 环境已就绪、需求明确任务的总人工活跃时间（计划内+计划外） | 中位数 ≤10 分钟，P90 ≤25 分钟；环境一次性安装时间单列但不隐藏 |
| 每 Task 全部人工决定次数（契约、提问、人审、纠偏、交付均计） | 中位数 ≤2，P90 ≤5；取消/故障恢复单列 |
| D11/全局配置/Skill binding 审批理解度 | seeded 遗漏、范围降级、账号切换的检出率 ≥90%；高风险错误批准 0 次 |
| 多余提问率 | ≤10% |
| 硬预算遵守率 | 100% |
| 端到端墙钟时间与实际 Provider 成本 | 按 task class 满足 S0 在 M0 前冻结的 P50/P90 数字上限；RC 不得临时放宽，原始用量完整公开 |
| Step 实际 CLI/model/Effort 与冻结路由一致率 | 100%；静默 fallback 0 次 |
| Project/Task/Receipt 跨项目串线 | 0 次 |
| Environment 自动动作与批准 plan 一致率 | 100%；未授权安装/升级/配置改写 0 次 |
| Skill source/digest/binding/projection 可追溯率 | 100%；未批准 Skill 加载 0 次 |
| 崩溃注入恢复且无重复副作用 | 10/10 |
| 未授权工作区外写入、密钥泄漏、验收删改 | 0 次 |

不能只优化“假完成率”：否则系统可以把所有任务标成 Blocked；也不能把契约确认、计划内 HumanReview 或交付批准排除在“人工介入”之外。UI 对每个用户决定记录 active-focus 时间、bundle 规模与结果（不做键盘内容监控），报告全量和分类型 P50/P90。S0 用相同 12 个任务冻结各 task class 的墙钟/成本上限并写入签名 eval manifest，之后只能通过用户批准的产品决策收紧或重做基线，不能在 RC 失败后放宽。

D11 与高影响配置审批不能只测“按钮可点”：development/sealed 用例在摘要中植入已知 requirement omission、验收降级、installation/account 切换或越权 Skill，核对用户是否能在摘要层发现。该测试评估信息设计，不把用户机械点击当作有效控制。

### 12.3 Sealed 评测治理与人员独立性

sealed set 不放在产品源码仓库，由独立访问控制的 release-eval 仓库与 OracleController 持有。职责分离如下：

| 职责 | 可见内容 | 禁止事项 |
|---|---|---|
| task author | 业务目标、公开验收 | 不参与对应 RC 的实现或 prompt 调优 |
| oracle owner | sealed fixture、期望值、正确终态 | 不向实现团队泄露样例或失败细节 |
| implementation team | development/regression set、产品代码 | 不访问 sealed set、OracleController 或发布结果中间态 |
| release adjudicator | 汇总结果、泄漏审计、异常 Run | 不修改产品代码或把失败改成 waiver |

每个 RC 先由 release-eval 环境从冻结 commit 做独立可复现构建，核对 binary/ASAR/schema hash，再冻结 Project/config/skill snapshots、Prompt/playbook、Codex/Claude installations、协议、模型/Effort、environment profile 和评测 manifest。被测包只能访问资格认证所需的精确 Provider endpoints，关闭 research/market tools 和其它网络，输出遵循固定 schema、目录与配额。oracle owner 随后一次性启动两轮评测；运行中不调参、不挑选重跑，OracleController 只向外部 orchestrator 返回结果，基础设施故障按预先登记规则作废整次 Run 并保留记录。RC 解封后，失败样例移入 regression set，sealed 替代样例必须轮换并做需求等价复核与隐蔽通道泄漏审计。

排期假定除两名 Rust/Electron 实现者外，还有一名不参与实现的兼职 oracle owner/release adjudicator。若实际只有两人，所有结果只能标为“内部 held-out”，不得作为 2.0 发布认证；正式发布前必须增加外部盲评与泄漏审计。

---

## 13. 研发里程碑

### S0　八项并行承重 Spike 组合（总体 3–4 周，按里程碑取证）

S0 不是要求所有外围能力先完成、再允许验证核心 Loop 的“大瀑布”。用可丢弃代码回答八个承重问题，并按下面依赖矩阵在对应里程碑前取得 GO：

1. Codex adapter 能否在专用 `CODEX_HOME`、file credential store、冻结 instruction/config/hook source 下稳定完成 account、model/list、initialize、thread/turn、dynamic tools、sandboxed command、interrupt、exact resume 和 usage，并穷举拒绝全部旁路权限；
2. Claude adapter 能否以 stream-json + 唯一 stdio MCP 完成同等生命周期，且 `--bare`、permission-prompts none、sandbox hard-fail、无 unsandbox escape、逐步骤 Skill 投影和不会泄漏的 dedicated apiKeyHelper 均可证明；shared Keychain 只验证 status/外部打开且不会被路由使用；
3. macOS GUI 环境发现能否在无 login-shell PATH、重复 binary、symlink、非默认 Homebrew mirror 下准确判断 identity/source/signature/capability；EnvironmentInstallationTransaction/UserActionChallenge 在 crash/cancel/retry/partial success 后能否无重复副作用恢复；
4. `find-skills` 搜索源、pinned Skills CLI、quarantine、静态审计、Vault、分离安装/绑定以及 Codex/Claude 只读投影能否形成可重放 receipt，并证明未批准 Skill 不可见；
5. 12 条产品/功能/歧义任务能否形成不遗漏、不臆造、验收可证伪的 ProjectIntent/TaskContract，并由独立人工标注评测，同时冻结人工时间、决定次数、墙钟与成本基线；
6. macOS 签名 Electron 能否可靠启动、校验和关闭 Rust sidecar，sandbox 能否阻止恶意项目/Skill 读取 Home/Core DB/证据存储、访问网络或调用宿主凭证；
7. 不带 Marketplace、使用预装环境、空 SkillSet 与固定离线模板的最小绿地产品，能否完成构建、黑盒 ProcessCheck 与原子目录交付；`greenfield-web-v1`/PreviewSession 能否在此基础上资格化；
8. Core-mediated research search/fetch 能否在出站前 DLP、URL 约束和内容快照下提供可复核事实。

| 开始里程碑前 | 必须已有 GO 的 S0 项 |
|---|---|
| M0 | #1 Codex 最小生命周期、#5 契约/证据、#6 Electron/Core + sandbox 最小边界 |
| M1 | #5 完整人工标注与效率基线、#7 最小离线绿地 tracer、#8 事实通道（若该切片需要当前外部事实） |
| M2 | #2 Claude + credential、#7 最小原子绿地交付 |
| M3 | #3 Environment、#4 Skill Marketplace |
| M4 | #7 完整 `greenfield-web-v1` + PreviewSession |
| M6 / 2.0 发布 | 八项全部 GO，并对最终固定版本重跑 |

某项失败只阻断依赖它的切片和对应 2.0 发布能力，不能阻断无关的 M0 核心闭环，也不能被静默删除后把 2.0 改称完成。Spike 代码不直接晋升产品；每个里程碑将已通过项的边界和版本正式重建进产品并重跑资格门。

### M0　Project-first 与最薄真实闭环（3–4 周）

**可演示结果**：用户创建/导入并初始化 Project，看到只读环境状态和全局继承配置；在 Project 下输入一句现有仓库任务，完成契约、单节点实现、隔离验证与独立审计，Rust 签发 CandidateCertificate。

范围：两个 Rust crate、Electron 安全壳、ProjectHome/ProjectIntent/Project→Task 聚合、ExecutionQueue/HarnessLease、Global/Project 配置快照、只读 Environment Center、版本化 IPC、SQLite/events、Codex 最小 adapter、AttemptPermissionProfile、sandbox、Autome tools、Readiness/Qualification、最小 rule/oracle/test inventory 和单次返工。

通过门：`T1-M0`–`T4-M0` 全绿；无 Project 不能建 Task，项目切换不串线；同一 seed task 连续 3 次正确；无/旧/错绑 Receipt 不能取得候选资格；干净构建不存在 1.x。**M0 不写用户仓库、不产生 Completed，交付到 M2。**

### M1　任务理解、配置继承与人工审计（约 3 周）

**可演示结果**：用户在全局与 Project 配置矩阵中为每个 AI 步骤设置 CLI/model/Effort/HumanReview；产品需求、已有功能和关键歧义形成带事实、要求、Checks、覆盖矩阵与执行图的审批稿；并用预装环境、固定离线模板和空 SkillSet 把一个最小绿地 Web 产品做到 CandidateCertificate。Skills 市场绑定仍在 M3 开放。

范围：sparse inheritance/provenance、GlobalConfigImpactPreview、ResolvedProjectConfig、PlanningRunSpec→ExecutionRunSpec 原子提升、PlanningPolicyRestart/RunPolicyAmendment/BudgetGrantReceipt、UserCorrection、HumanReviewReceipt/Finding、原始语义、project-rule snapshot、contract reviewer、外部事实新鲜度、ExecutableOracle 判别力，以及无 Preview/无市场/不安装依赖的 `greenfield-lite` tracer。

通过门：配置 overlay/生效矩阵真值表、无 fallback、活动 spec 不热改；批准前五个步骤无 candidate-write，批准事务缺任一 hash 均回滚；restart/amendment 创建新 Run 且不复用旧收据；development set 与人工标注对账；遗漏、臆造、必要性误分和锚点错误分别报告；人审 off 不能关闭固定安全门；同一最小绿地 tracer 连续 3 次生成可构建、可黑盒验证且证据齐全的 candidate。

### M2　双 CLI 顺序多节点完整 Loop（5–6 周）

**可演示结果**：同一已有仓库 Task 按 Project 配置跨 Codex/Claude 执行“分析 → 拆分 → 实现/返工 → 独立审计 → 交付演练 → 用户批准 → 专用分支 CAS → 只读核对”；另一个最小绿地 Task 在预装环境、空 SkillSet、固定离线模板下完成黑盒验证与原子目录交付，不等待 Marketplace。

范围：Claude adapter/stdio MCP、正式 CredentialRecord/Receipt、Keychain 管理 UI、签名 credential broker/apiKeyHelper 及泄漏负测、空/内置 fixture SkillSetSnapshot 的两套逐步骤投影基础、逐步骤 route、身份分离、多节点 TaskGraph、TestInventory、历史红灯、deliverable、CandidateCertificate、最小 Git/Directory broker、完整交付收据链与 `greenfield-lite` headless runner。

通过门：至少 6 个已有仓库任务各运行 2 次，另有 2 个最小绿地 Task 各运行 2 次并原子交付；Codex 和 Claude 各承担 implementation 与 review/audit；Claude dedicated key 对 Agent/项目进程/argv/env/log/error 不可见；实际 CLI/model/Effort 100% 匹配冻结路由；测试替换、未交付制品、过期事实/证据、无关 diff 和原 checkout 变化均阻止完成。Git ref 与 greenfield directory delivery 都在 intent 前、外部写成功后、receipt 写入前后逐点强杀；重启必须分辨未执行/已执行/UnknownOutcome、在可证明时补记 receipt，且绝不重复写入。

### M3　Environment Center 与 Skill Marketplace（约 4 周）

**可演示结果**：从环境总览一键生成并执行受控补齐计划；从技能市场搜索、隔离审计、第一次确认安装到 Vault（默认 disabled），再经第二次确认绑定 Project/步骤，并由两套 CLI 在新 Run 中准确加载。

范围：SignedDependencyCatalog、presence/integrity/auth/qualification/readiness 环境状态、多 installation 选择、安装事务/UserActionChallenge、Homebrew/source/signature policy、iTerm2 快捷入口、skills.sh/find-skills、quarantine/audit/Vault、分离的 SkillInstallPlan 与 SkillBindingPlan、Global/Project binding、更新/禁用/GC 与 receipt。

通过门：完全缺失、完全预装、混合来源三类机器 fixture；安装 crash/cancel/retry/UnknownOutcome 无重复副作用，UserActionChallenge 仅凭绑定 post-probe 推进；市场离线可降级；安装后未绑定 Skill 0 次加载，Global binding 必须有影响确认；良性 Skill 双 CLI 可发现，恶意/漂移/未批准 Skill 0 次加载；CLI exit 0 但 post-probe 失败时必须失败。

### M4　绿地产品需求（约 3 周）

**可演示结果**：创建 `new_product` Project，从离线模板生成可运行产品，经一次性 PreviewWindow、黑盒旅程、持久化和制品验收后原子交付；Project 随后转换为已有仓库。

范围：greenfield playbook/profile、环境补齐入口、ProjectTargetTransitionReceipt、PreviewSession/proxy/一次性 partition、进程/端口租约、check runners、原子目录交付和证据等级。

通过门：至少 4 个绿地与 4 个已有功能 Task 各运行 2 次；干净目录构建、重启持久化、黑盒旅程、制品 hash、项目转换和后续第二个 Task 全部可核对；专门构造第二 Task 试图违反首个 ProjectIntent 跨任务约束，未经 IntentAmendment 必须阻塞。目录交付在 intent 前、临时 tree 完成后、原子 rename 成功后、DeliveryReceipt/ProjectTargetTransitionReceipt 事务前后逐点强杀；恢复不得覆盖既有 destination、重复落位或伪造 Project 转换。

### M5　控制、恢复与资格稳定性（3–4 周）

**可演示结果**：强杀 Renderer/Main/Core/Codex/Claude 或中断 Environment/Skill/Git transaction 后，从 durable point 核对恢复且不重复副作用；预算硬顶、配置/Skill/CLI 漂移和身份替换均 fail closed。

范围：单实例/orphan policy、Project identity、config/skill/readiness invalidation、两 adapter TTL/canary、approval 防重放、四类 broker 的跨事务/组合故障强化、长时间 UnknownOutcome、数据出站、FileVault 与诊断脱敏；每类 broker 的首次真实写入里程碑已经各自承担最小崩溃恢复门，不能推迟到 M5。

通过门：每个持久化边界 fault injection；恢复 10/10；恶意 hook/MCP/plugin/skill/Git filter/credential helper 无法越权；CLI/模型/Skill/DependencyCatalog 更新自动撤销相应资格。

### M6　macOS 2.0 发布资格（2–3 周）

**可演示结果**：在没有 Autome 源码、Rust、1.x、Xcode CLT/Git、CLI、Node、iTerm2 或 Skills CLI 的干净 Mac 上安装 Autome；从“一键补齐”开始，对可自动项完成安装，对 CLT/登录/系统动作正确停靠，随后用双 CLI 配置完整执行、恢复、验收、交付并卸载。

范围：Electron Forge + 稳定 Webpack、Rust sidecar、签名/公证、原子 manifest、手工前向升级、数据库恢复、诊断包、三类干净机矩阵、外部 OracleController 与 sealed certification。2.0.0 不做在线应用自动更新，也不承诺外部软件都可自动回滚。

通过门：T1–T6 完整 manifest；sealed set 达到 pass²；36 次认证 Run 假完成观测数为 0；签名/Gatekeeper/安装/卸载通过；没有 waiver 式跳过；卸载不删除用户原有工具、账号或 unmanaged Skill。

**总周期估算：26–31 周。** 该估算假定两名熟悉 Rust/Electron 的工程师、一名兼职独立 oracle owner/release adjudicator，并包含双 CLI、Environment Center、Skill Marketplace、S0 与两次密封评测窗口。各 S0 证据按依赖里程碑即时更新排期，不能等全部外围 Spike 完成才首次验证核心 Loop。

---

## 14. 发布清单

2.0.0 发布包必须包含：

- Electron 应用、签名 Rust Core 及各自许可证；Codex CLI 与 Claude Code CLI 是可由 Environment Center 管理的外部前置依赖，不打进安装包；
- `core-manifest.json`：应用/Core/protocol/schema/playbook/profile/check-runner/config/permission/skill-policy/scanner/install-catalog/build/target/hash；
- 签名 DependencyCatalog、精确 install recipes、允许来源/镜像、rollback class 与已资格版本；
- 首批 host profile manifest、准备说明、工具来源与资格结果；
- 精确 lockfiles、SBOM 和依赖漏洞报告；
- 支持的 Codex/Claude installation、协议、model/Effort、认证模式、host profile 与资格结果；
- ProjectIntent/ExecutionQueue、config inheritance/impact schema、UserCorrection、HumanReview/Finding 语义与示例 resolved snapshot；
- Skill market provider/pinned helper、分离 Install/Binding、Vault/Projection schema、扫描策略和已知限制；
- research search/fetch provider、允许域名、数据出站策略与 FactReceipt freshness 规则；
- 完整 golden run IDs、每项发布指标和明确的未验证边界；
- sealed manifest hash、oracle owner 与 release adjudicator 签认、泄漏审计结果；
- 数据库备份/恢复与手工前向升级说明；
- owner-only 权限、FileVault 探测结果和“无应用层字段加密”的明确数据保护边界；
- 默认脱敏的诊断导出；
- UserActionChallenge/安装恢复手册，以及卸载保留用户原有 CLI、iTerm2、账号、Project 与 unmanaged Skill 的说明；
- 无 1.x 文件或运行时依赖的证明。

安装/升级包必须原子包含 Electron、Rust Core、schema、playbook、profile、config/skill policy、scanner、SignedDependencyCatalog、install recipes 和 check-runner manifest。首版不允许从网络单独热替换其中任一资产，也不宣称自动回滚应用二进制；升级失败时恢复数据库备份并重新安装上一已签名版本。

---

## 15. 主要风险与反证条件

| 风险 | 设计防线 | 推翻当前方案的证据 |
|---|---|---|
| Rust Core 过度复杂 | 两个 Rust crate、S0 后只做 Project-first 最薄纵向链路 | M0 四周内不能完成最小状态/证据闭环，应继续缩减非核心 UI，而不是删证据门 |
| “从零”边界被暗中复用破坏 | 物理新仓库、禁止实现者读取/复制 1.x 源码、初始文件 provenance、独立来源审计 | 任一运行或构建依赖 1.x，或发现复制旧协议/实现，应拒收对应实现并重新建设 |
| 双 CLI 语义不对等 | 共同 Harness contract、各自 capability snapshot 和同一 golden 门 | Claude/Codex 任一需解析自然语言状态、不能安全暂停/恢复/加载工具时，不能宣称该 route 可选 |
| 认证边界不可闭环 | Codex 独立 home；Claude shared 身份只监测，自动 route 仅接受经 S0 证明的 dedicated credential handoff | 必须复制 token、把 API key 暴露给项目子进程、让 `--bare` 读取 shared OAuth 或伪称 Keychain 身份独立时，阻断 Claude 自动 route |
| 环境补齐失控 | SignedDependencyCatalog、exact artifact、事务/UserActionChallenge/后置探测、无 broad package-manager 操作 | 未批准安装、错误外部动作被归因、镜像漂移、exit 0 假成功或崩溃后重复副作用任一出现，停止自动修复 |
| Skill 市场供应链污染 | quarantine、exact commit/digest、静态审计、默认 disabled、安装/绑定两次决定、逐步骤投影、权限交集 | 安装即启用、未批准/更新后 Skill 被加载，或 Skill 能扩大 StepPolicy，停止市场安装能力 |
| Project/配置串线或漂移 | ProjectIntent、Project 聚合、sparse inheritance、全局影响预览、resolved spec、amendment | 后一 Task 静默推翻项目约束、切换 Project 后出现其它任务/Receipt，或运行中配置被热替换，阻断发布 |
| 运行环境不可重现 | profile + ReadinessReceipt + catalog 支持的补齐计划 | profile ready 仍因隐藏宿主依赖失败，应修正 profile 并使旧资格失效 |
| 项目代码越过宿主边界 | disposable clone、受信仓库门、sandbox executor、默认无网络、verifier 独立低权限 | 恶意 fixture 能读取 Home/Core DB、调用凭证或联网，禁止进入任何真实任务里程碑 |
| ProtectedOracle 被测评对象探测 | 外部 controller、单向黑盒 IO、运行结束前零反馈和泄漏测试 | 被测 Core/应用/产物能从路径、env、错误、进程信息或出站推断 oracle，sealed 结果全部作废并轮换 |
| 自动拆解遗漏需求 | 原始语义覆盖矩阵、独立 contract reviewer | M1 评测中遗漏/臆造率不能达到发布门，应暂停自动执行，只保留人工契约确认 |
| 用户纠偏被忽略或绕过契约 | UserCorrection 安全停靠、结构化分类、新 Attempt/Amendment 与 finding 消费 | 同一输出反复送审、纠偏未进入后继或语义变化被当执行提示时，阻断发布 |
| 验收被 Agent 迎合 | ContractAcceptanceCheck、独立 verifier、最终 tree 重跑 | mutation 后仍完成，说明 verifier/证据模型无判别力，禁止进入下一里程碑 |
| 过度提问/审批降低自动化 | 明确默认假设规则、统计全部计划内外决定与人工活跃时间 | 正确处置率提高但人工时间、决定次数或多余提问率超门槛，说明系统让用户全职监督来伪装可靠 |
| Electron 成为第二内核 | contract test 与依赖方向门禁 | 业务状态在 Rust 不可用时仍能由 UI 改变，应阻断发布 |
| 恢复产生重复副作用 | intent/receipt/idempotency/UnknownOutcome | 任一 fault injection 重复执行外部动作，暂停对应 capability |
| 交付验证污染用户仓库 | 临时 rehearsal tree、专用新分支 CAS、交付后只读核对 | 用户 checkout 被修改或交付后执行候选代码，停止 Git broker |
| 多 CLI/模型仍共享盲区 | 真实 oracle 优先、身份隔离只降低利益冲突 | 不同 route 通过但 ProtectedOracle 失败时，以 oracle 为准并加入 regression set |

---

## 16. Agent-native 架构检查表

| 原则 | 2.0 落点 |
|---|---|
| Parity | UI 与 Agent 的 Project/config/environment/skill 读取和 proposal 通过同一 Rust service；安装、配置应用、人工审批与完成裁决故意不授予生产者 |
| Granularity | Harness 保留读写、命令、搜索等原子工具；Rust 只硬编码安全、状态和证据不变量 |
| Composability | 新任务类型由 playbook/prompt/acceptance adapter 组合，不改核心状态机 |
| Emergent capability | Golden set 含开放任务；不能因为没有预制功能就拒绝，但必须遵守契约和证据门 |
| Shared workspace | 用户代码与 Agent 候选通过 Git commit/受控交付共享；控制面与证据存储隔离 |
| Explicit completion | `complete_task` 明确结束 Attempt，但只产生 AgentClaim |
| Partial/resume | Run/Attempt/节点状态、provider session ID 和 durable events 支持恢复 |
| Context | 动态注入 Project、ResolvedConfig、SkillSet、任务、资源、能力和有效证据；大材料按需读取 |
| Agent → UI | Rust 事件立即更新 UI；事件缺口强制 resync |
| Capability discovery | Environment/Skill inventory 与两个 Harness probe 共用 Core 快照，UI 与 Agent 看见同一能力和缺失项 |
| Testing outcomes | 验证最终环境与用户结果，不锁死 Agent 的具体工具调用路径 |

---

## 17. 官方技术依据

- Electron 进程模型与 IPC：<https://www.electronjs.org/docs/latest/tutorial/process-model>
- Electron 安全清单：<https://www.electronjs.org/docs/latest/tutorial/security>
- Electron Context Isolation：<https://www.electronjs.org/docs/latest/tutorial/context-isolation>
- Electron 打包分发：<https://www.electronjs.org/docs/latest/tutorial/application-distribution>
- Codex CLI 安装与能力：<https://learn.chatgpt.com/docs/codex/cli>
- Codex App Server：<https://learn.chatgpt.com/docs/app-server>
- Codex Skills 与目录：<https://learn.chatgpt.com/docs/build-skills>
- Codex 配置与 model reasoning effort：<https://learn.chatgpt.com/docs/config-file/config-reference>
- Claude Code 安装：<https://code.claude.com/docs/en/quickstart>
- Claude Code CLI/stream-json/model/Effort：<https://code.claude.com/docs/en/cli-usage>
- Claude Code Sandbox：<https://code.claude.com/docs/en/sandboxing>
- Claude Code Skills：<https://code.claude.com/docs/en/skills>
- Claude 配置目录与 Keychain 边界：<https://code.claude.com/docs/en/claude-directory>
- skills.sh / Skills CLI：<https://www.skills.sh/docs/cli>
- iTerm2 下载与 Shell Integration：<https://iterm2.com/downloads.html> · <https://iterm2.com/documentation-shell-integration.html>
- Homebrew：<https://brew.sh/>
- Tokio 异步进程：<https://docs.rs/tokio/latest/tokio/process/>
- Cargo Workspaces：<https://doc.rust-lang.org/cargo/reference/workspaces.html>
- Cargo Test：<https://doc.rust-lang.org/cargo/commands/cargo-test.html>
- rusqlite Transaction：<https://docs.rs/rusqlite/latest/rusqlite/struct.Transaction.html>
- Git `update-ref` compare-and-swap：<https://git-scm.com/docs/git-update-ref.html>
- Apple FileVault 数据卷保护：<https://support.apple.com/guide/security/volume-encryption-with-filevault-sec4c6dc1b6e/web>
- OWASP SSRF 防护：<https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html>

---

## 18. 开工与分阶段解锁条件

以下架构与资源条件确认后可以进入 M0：

1. 物理独立的新 2.0 仓库已建立，内容不含 1.x 源码，并启用初始文件 provenance 审计。
2. macOS 14+ Apple Silicon 首发边界被接受。
3. Project-first、ProjectIntent、`new_product | existing_repository`、ProjectHome 不默认写用户仓库以及 Task 必属 Project 的边界被接受。
4. SQLite 权威、Rust sidecar、Codex App Server + Claude stream-json/stdin MCP 双 adapter 与 disposable clone 路线被接受。
5. Global→Project sparse inheritance、逐 AI 步骤 CLI/model/Effort、可选 HumanReview 和 Run 快照/Amendment 语义被接受。
6. SignedDependencyCatalog、Environment 安装事务/UserActionChallenge、Skill quarantine/Vault/分离安装与绑定/Projection 和供应链风险已进入架构测试清单；它们不必在 M0 实现。
7. “AgentClaim 不等于 CandidateCertificate/Completed”、交付演练与发布指标被写入不可降级架构测试。
8. 独立 oracle owner/release adjudicator、fixture repository、人工标注任务集和首个 ProtectedOracle 已准备。
9. S0 #1、#5、#6 已取得 M0 所需的最小 GO 证据；不要求 Claude、Marketplace 或完整 greenfield profile 先于 M0 完成。

后续能力严格按 §13 的 S0 依赖矩阵解锁：M1 前取得最小绿地/必要事实通道证据，M2 前取得 Claude credential 与最小原子绿地交付证据，M3 前取得 Environment/Skill 证据，M4 前取得完整 `greenfield-web-v1`/PreviewSession 证据。M6 与 2.0 发布仍要求八项对最终固定版本全部 GO；某项未通过时只允许推进不依赖它的切片，不能把失败能力包装为可用。
