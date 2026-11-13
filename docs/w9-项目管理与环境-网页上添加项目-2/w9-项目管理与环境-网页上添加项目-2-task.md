# 任务：W9 项目管理与环境——网页上添加项目（服务端目录补全与路径校验）、Onboarding 与项目设置页、移除项目、项目报警、环境检查页

本任务的规则见 `docs/w9-项目管理与环境-网页上添加项目-2/protocol/loop-protocol.md` 与
`docs/w9-项目管理与环境-网页上添加项目-2/protocol/session-protocol.md`。

- 设计文档：`docs/w9-项目管理与环境-网页上添加项目-2/w9-项目管理与环境-网页上添加项目-2.md`
- 任务卡（第一输入，用户所写）：`docs/w9-项目管理与环境-网页上添加项目-2/w9-项目管理与环境-网页上添加项目-2-card.md`
- 用户指定的参考文档：`docs/plans/2026-11-10-web-dashboard-via-loop.md`（下称「计划」）
- 任务文件生成日期：2026-11-13；基线提交 `2dff3485`（分支 `autome/w9-项目管理与环境-网页上添加项目-2`，
  与 main 的分叉点即生成时的 main `02a05496`）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支）
- 证据、简报、运行记录一律写在 `docs/w9-项目管理与环境-网页上添加项目-2/` 下

## 1. 需求原文

> W9 项目管理与环境：网页上添加项目（服务端路径校验与目录补全）、Onboarding、移除项目、项目设置页、环境检查页

这句话对应计划 §6 **W9 项目管理与环境**。任务卡正文把它展开成了具体行为、排除项与验收标准，并比计划多出两块
（项目报警、配置只读区）。**两者不一致时以任务卡正文为准**；任务卡与计划 §6 W9 的「目录」「验收」不一致时
同样以任务卡为准。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-05 取自任务卡「要做什么」，A-01..A-11 取自任务卡「验收标准」，原意不增不减。编号供评审、审计追溯。

- **R-01 添加项目** `apps/web/app/routes/projects.new*`
  - R-01a 目录补全：新增 `apps/web/server/paths*`。输入路径，服务端只列 `$HOME` 之内的**目录**（不列文件）；
    对输入与结果都做 realpath 后再判断是否仍在 `$HOME` 内（符号链接指到外面的不列）；拒绝含 `..`、NUL 的输入；
    单次最多返回 50 项；默认不列以 `.` 开头的目录，输入本身以 `.` 开头时才列。补全是只读接口，同样过计划 §5.5 的
    Host 检查。
  - R-01b 确认之前先调 `project.probe`，把结果显示出来：是否 Git 仓库根、是否像工作区（`suggest_workspace`）、
    成员仓库与建议的文档仓库（`suggested_docs_repo`）、内核给的拒绝原因。
  - R-01c 表单项：路径、是否工作区、文档仓库（下拉自 probe 的成员，只传名称）、是否跳过 Onboarding。提交走写入口的
    `project.confirm`：在 `server/` 里为它注册处理函数，**重新校验路径在 `$HOME` 内**后调内核 `project.add`。
    成功后跳到项目设置页（没跳过 Onboarding 时）或项目页。
  - R-01d `project.pick`（原生目录选择）在网页里保持 501；页面上预留一个「选择目录…」按钮位，只有
    `window.autome?.pickDirectory` 存在时才显示（接口由 W11 提供）。
  - R-01e 左侧项目列表底部与 ⌘K 增加「添加项目」入口。
- **R-02 项目设置页** `apps/web/app/routes/projects.$id.settings*`
  - R-02a Onboarding：显示当前步骤；「运行」（`project.onboarding.run`，运行中显示会话状态，结束后实时刷新）；
    查看并编辑两份产物——项目画像 `docs/agent-project-profile.md` 与 `AGENTS.md`（`project.onboarding.artefacts` 读、
    `project.onboarding.save` 写，长度上限与 write-gate 一致）；「完成」（`advance`）与「跳过」（`skip`）。
    完成或跳过后显示 init commit 的结果。
  - R-02b 配置（只读）：显示 `config.get` 解析后的值——并行数、门禁命令与运行时机、快检、重试、会话时长上限等
    内核返回的字段，以及 `config.validate` 的警告，并写明两份配置文件的路径（`~/.autome/config.toml`、
    `<项目>/.autome/config.toml`）。不提供编辑。**字段清单以内核返回为准，不在前端另写一份默认值。**
  - R-02c 移除项目：把 `project.remove` 加进写白名单（只收 `project_id`，按现有 id 校验口径）；按钮需要二次确认
    （输入项目名）；内核拒绝时（还有未结束的任务）原样显示内核那句话。移除只是取消注册，不删任何文件——
    确认框里写明这一点。
  - R-02d 项目页右上角加「设置」入口。
- **R-03 项目报警。** 项目页（`projects.$id.tsx`）顶部显示未消除的报警：内容、时间、相关任务链接、
  「知道了」（`project.alert.dismiss`）。左侧项目列表里有报警的项目带一个标记。报警出现与消除都实时更新。
  （任务卡写的数据来源与事件名与内核现状有出入，见 §2.1 的 F-01、F-03。）
- **R-04 环境检查页** `apps/web/app/routes/env*`
  - R-04a 显示 `env.get`：Git、Claude Code、Codex 的版本、路径与登录状态；「重新检测」走 `env.detect`
    （检测中显示进行状态，`env.probing` 事件）。
  - R-04b 安装与登录：页面显示 `env.install_recipe` 返回的命令原文与前置条件是否满足；点「在终端里执行」走写入口
    `env.install` / `env.login`，由服务端打开 Terminal.app 执行。**浏览器只传组件名（`git` / `claude` / `codex`），
    要执行的命令由服务端当场再向内核取一次**，不接受浏览器传来的任何命令文本。`open.terminal`、`open.path` 同理：
    只接受现有 write-gate 允许的参数形状，路径必须解析到已注册项目目录或 `AUTOME_HOME` 之内。
  - R-04c 非 macOS 或打不开终端时，给出命令原文和「复制」按钮，不报内部错误。
  - R-04d 左侧栏底部加「环境」入口；首页总览在环境有缺项（未安装 / 未登录）时给一条提示并链接到环境页。
- **R-05 文案与离线。** 中文文案进 `app/lib/labels.ts` 或就近的文案表，沿用 W5/W6 的口径；内核离线时这些页面的
  写按钮置灰（沿用外壳已有的状态）。

验收标准（任务卡原文）：

- **A-01** 目录补全：只返回 `$HOME` 内的目录；`..`、NUL、绝对路径逃逸、指向 `$HOME` 之外的符号链接各有一个测试；
  结果上限有测试。
- **A-02** 添加项目：确认前一定调过 `project.probe`（测试）；`project.confirm` 的路径在服务端再次校验（测试：直接向
  写入口发 `$HOME` 之外的路径被拒）；单仓库与工作区各走通一次（对真实 automed 的集成测试，临时目录里建仓库）；
  内核拒绝时显示原文。
- **A-03** Onboarding：对真实 automed 与假 CLI 走完「运行 → 编辑两份产物并保存 → 完成」，仓库里出现 init commit；
  「跳过」同样生成 init commit；保存超长内容被网关拒绝并有中文提示。
- **A-04** 工作区项目：probe 提示、成员与文档仓库选择、添加后项目页正常显示（集成测试）。
- **A-05** 移除：有未结束任务时被内核拒绝并原样显示；没有时移除成功、左侧列表实时少一项；仓库文件不变（测试）。
- **A-06** 报警：造一条项目报警 → 项目页顶部出现、左侧列表有标记 → 点「知道了」后消失，全程不手动刷新（集成测试）。
- **A-07** 环境页：三个组件的状态与内核一致；`env.install` / `env.login` 的请求体带任意 `command` 字段时被忽略或拒绝，
  实际执行的命令等于内核 recipe（测试用假的终端启动器记录收到的命令）；打不开终端时页面给出命令原文。
- **A-08** 配置只读区：显示的值与 `config.get` 一致；配置里写一个非法值时页面显示 `config.validate` 的警告（测试）。
- **A-09** §5.5：本任务新增的每个写操作与补全接口都有 Host / Origin 检查的测试。
- **A-10** 1512×944 下新增的三类页面 `scrollHeight ≤ clientHeight`（内部滚动区域除外）。
- **A-11** `npm --prefix apps/web` 的 lint、typecheck、test、build 全绿；`sh scripts/ci-local.sh` 全绿。

### 2.1 任务整理轮核对到的事实（基线 `2dff3485`，读源码所得；设计轮引用前自行复核）

任务卡里有几处写法与内核、仓库现状对不上。本任务不能改 `crates/`（§4 约束 1），所以这些出入只能在
`apps/web` 一侧解决；怎么解决由设计轮定，这里只列事实。

- **F-01 `project.get` 的回包里没有 `alerts`。** `dispatch.rs` `project_get` 返回
  `{ project, cost_usd_total, cost_sessions_without_cost, tasks, counts, config, violations, rules }`。未消除的报警在
  **`dashboard.get`** 的 `alerts` 里（全部项目的，`store.open_alerts()`），每条是
  `{ id, project_id, kind, task_id, detail, created_at }`。`dashboard.get` 已在读白名单里。源码里出现过的 `kind`：
  `main_gate_failed`、`setup_failed`（`scheduler.rs`），`server/notifier-rules.ts` 另外认 `budget_exceeded`。
- **F-02 `project.probe` 的回包里没有「拒绝原因」字段。** 它返回
  `{ path, exists, is_repo_root, members:[{name, default_branch}], suggest_workspace, suggested_docs_repo }`，只读、无副作用；
  `exists` 为假表示那不是一个已存在的目录。「该目录已经是一个项目」「… 与已有项目「…」嵌套，不能同时管理」
  「… 不是目录」「工作区项目要指定一个文档仓库」「… 不是这个目录下的仓库」「… 没有可用的默认分支」这些拒绝
  只在 `project.add` 时给出。
- **F-03 账本里的事件名与任务卡写的不完全一样。** 浏览器收到的事件来自 `events.since`（账本），不是 sidecar 回帧
  （计划 §5.1）。账本里实际追加的是：`project.added`、`project.removed`（subject 都是项目 id）；Onboarding 的
  advance / skip / save 记的是 **`project.onboarding`**（subject 项目 id）——`project.updated` 只出现在本进程的
  回帧里，账本里没有；报警产生记 `project.alert`（subject 项目 id，payload `{ alert_id, kind, task_id }`）；
  报警消除记 `project.alert_dismissed`，**subject 是报警 id，不是项目 id**；`env.detect` 只在真的启动了一次探测时
  记 `env.probing`（subject `environment`）。`app/root.tsx` 现在的 `shouldRevalidate` 是
  `byKind('task.', 'project.', 'bug.', 'merge.')`，没有 `env.`。
- **F-04 环境探测结束没有账本事件。** `env_detect` 的注释写明：探测落地由 `scheduler.tick` 回包里的
  `env_generation` / `env_probing` 报告。`apps/web` 目前没有任何代码读这两个字段。`env.get` 在还没有快照时会自己
  启动一次探测并立刻返回 `probed: false`——那是「还没检测」，不是「一切正常」，也不是「有缺项」。
- **F-05 `env.get` / `env.detect` 回包：**`{ probed, probing, generation, environment: { components: [{ component, present,
  version, path, login, checked_at }], checked_at }, severity, problems, can_run_anything, prerequisites: { brew, npm } }`；
  `login` 是 `NotApplicable | Ok{account_hint} | Expired | Unknown{detail}` 的序列化（具体线上形状设计轮实测）。
  `dashboard.get` 也带一份精简的 `environment: { probed, severity, problems:[{component, name, present, login}] }`。
- **F-06 组件有四个，不是三个。** `Component::ALL` = `git`、`claude`、`codex`、`iterm2`；web 的 write-gate
  `COMPONENTS` 同样放行 `iterm2`。任务卡 R-04a / R-04b / A-07 只说 Git、Claude Code、Codex 三个。
  `env.install_recipe` 回包 `{ recipe: { component, command, prerequisite, note }, prerequisite_ok, login_command }`；
  `login_command` 只有 `claude`（`claude /login`）、`codex`（`codex login`）有，其余为 null。
- **F-07 `project.add` 会动磁盘。** 路径不存在时 `create_dir_all` 创建；非工作区且不是仓库根时 `git init`（默认分支
  main）；随后写 `.autome/` 脚手架。这是需求 P-01 定下的行为（`docs/development/requirements.md`）。参数只有
  `path`、`workspace`、`docs_repo`，**没有「跳过 Onboarding」**；命令行 `autome project add --skip-onboarding` 的做法是
  add 成功后再调一次 `project.onboarding.skip`。回包 `{ project, protocol_ref, init, pending_commit }`，新项目的
  `onboarding` 从 `{ onboarding: "in_progress", step: 3 }` 开始。
- **F-08 Onboarding 的步骤。** `Onboarding::InProgress{step}` 取 3..=5；`advance` 是 3→4→5→`completed`，**要调三次才结束**；
  `skip` 直接到 `skipped`；结束之后再 advance / run 被拒「Onboarding 已经结束」。advance / skip 回包
  `{ onboarding, init_commit }`，只有从未结束变为结束的那一次才做 init commit（提交 `.autome`、`.gitignore` 与两份产物中
  存在的那些，信息 `chore(autome): initialise project`）。`project.onboarding.run` 回包 `{ session_id, project_id }`，
  账本记 `session.started`（subject 项目 id，payload `{ session_id, kind: "onboarding", terminal }`），会话文件归在合成任务键
  `onboarding` 下；会话结束时账本记什么、`session.log` 对它是否可用，任务整理轮没有核实。
  `artefacts` 回包 `{ files: [{ path, exists, content }] }`；`save` 只收这两个 `path`。
  工作区项目的 `project.path` 本身不是 Git 仓库，它的 advance / skip 会得到什么 `init_commit`，同样没有核实。
- **F-09 `project.remove` 的拒绝语是英文：**`still-running tasks must finish or be cancelled first: T-1, T-2`。
  成功回包 `{ removed }`，账本记 `project.removed`；`project.list` 只列未移除的项目。移除后同一目录可以再次添加，
  沿用原来的项目 id。
- **F-10 `config.get` 回包 `{ global, resolved, violations, scope }`，`config.validate` 回包 `{ violations, ok }`。**
  内核自己的测试给了一个会产生 violation 的非法值：项目 `.autome/config.toml` 写 `[loop]` `session_minutes = 1` →
  `violations[0].kind == "session_minutes_out_of_range"`。配置文件 TOML 解析不了时，`config_io::load_project` 返回的是
  **错误**而不是 violation；`project.get` 也读同一份配置，会一起失败。
- **F-11 网关现状（`apps/web/server/gateway.ts`，60 行）。** `localOps` 六个操作没有注册处理函数时回 501
  「网页端还不支持「…」」；`registerLocalOp(op, handler)` 已有，`main.ts` 里没人调它。传给 `createGateway` 的 `home` 是
  **`AUTOME_HOME`**（`server/home.ts` 的 `resolveAutomeHome`），不是 `$HOME`。`execute` 把处理函数抛出的错误这样映射：
  带 `code` → 409，`timeout` → 504，其余 → 503 并在消息前加「内核未连接：」——本地处理函数自己的拒绝（例如路径不在
  `$HOME` 内）照现在的写法会被报成「内核未连接」。
- **F-12 两份白名单现状。** 写白名单（`gates/write-gate.ts`，293 行）：没有 `project.remove`，也没有 `project.add`
  （`test/write-gate.test.ts` 有断言守着后者不可达）；`project.confirm` 只检查 `docs_repo` 不能像路径，对 `path` 没有专门
  检查（普通字符串上限 512 字符）；`project.onboarding.save` 的 `path` 只能是两份产物之一，`content` 上限
  `MAX_DOCUMENT_LENGTH` = 256 × 1024 **字符**，整包上限再加 4096；`env.install` / `env.login` 的 `component` 须在四个组件里，
  多余字段目前不拒绝；`open.path` 的 `kind` ∈ `project / worktree / document / skill / log`，各 id 不能像路径；
  `open.terminal` 没有专门检查（旧面板传 `task_id`）。白名单里还留着从旧面板迁来的 `config.set_role` /
  `config.reset_role` / `config.set_loop` / `config.set_theme`。
  读白名单（`gates/read-gate.ts`）：`project.probe` 的 `path` 只能是 `~` 或 `~/…`，不含 `\`、NUL、`..` 段——只做字面检查，
  不做 realpath；其余参数不能像路径；`env.detect`、`events.since` 故意不放行。`test/read-gate.test.ts` 解析
  `dispatch.rs` 的 `READ_METHODS`（17 项）交叉核对。
- **F-13 `server/security.ts`：**所有请求查 Host；Origin 只查非 GET / HEAD 的请求。loader 与 action 在服务端直接拿
  `context.get(servicesContext).gateway` 调 `read` / `write`，不经 `/api/*`。
- **F-14 旧面板的参照实现在 `apps/desktop/main.js`**（`pickProject`、`confirmProject`、`openPath`、`runInTerminal`、
  `openTerminal`、`runInstall`、`runLogin`），任务卡写的 `apps/desktop/src/main/` 目录不存在。几个值得知道的做法：
  选中的目录存在主进程的 `pendingPick` 里，渲染进程的 `project.confirm` 不带路径；`runInTerminal` 用 `osascript`，
  装了 iTerm 就用 iTerm、否则用 Terminal；`runInstall` 在 `prerequisite_ok` 为假时拒绝「请先安装 …」；`runLogin` 在没有
  `login_command` 时拒绝。任务卡写的 `docs/development/` 下只有 `requirements.md`、`technical-design.md` 两份，没有单独的
  设计规范文件；设计 token 的硬规则由 `apps/web/test/styles.test.ts` 逐条守着。

### 2.2 任务卡没有定死、留给设计轮回答并写进设计的问题（只列问题，不预设答案）

1. 报警的数据来源（F-01）：项目页、左侧标记各从哪个读方法拿报警；`project.alert_dismissed` 的 subject 是报警 id（F-03），
   项目页与外壳靠什么重新加载。
2. R-01b「内核给的拒绝原因」在 probe 阶段显示什么（F-02）；`exists: false`、`is_repo_root: false` 时页面怎么告诉用户
   「确认后会创建目录 / 会 `git init`」（F-07）。
3. A-02「确认前一定调过 `project.probe`」由谁保证、测试断言什么（action 里先 probe 再 add，还是服务端像旧面板那样记一个
   待确认项）；网页的 `project.confirm` 参数形状（旧面板的不带路径，F-14）。
4. 路径的写法与 `$HOME` 的来源：补全接口、probe、confirm 各收绝对路径还是 `~/…`（读白名单只放行 `~/…`）；服务端的
   `$HOME` 取自哪里、是否与内核 `ctx.home` 展开 `~` 的结果一致；`$HOME` 自身是符号链接时（macOS 临时目录
   `/var` → `/private/var`）realpath 比较怎么做。
5. 补全接口的路径、方法、回包形状、排序、超过 50 项时怎么提示；它是 GET 时只有 Host 检查（F-13），A-09 的测试各断言什么。
6. 「跳过 Onboarding」怎么落到内核（F-07）；add 成功而 skip 失败时页面怎么收场。
7. Onboarding 页面的步骤与内核 step 3..5 的对应（F-08）：「完成」是一次还是三次 advance；「运行」之后会话状态从哪里读、
   页面刷新后还认不认得这次会话、结束靠什么事件刷新；A-03 的假 CLI 要写出什么才算对齐真实接口。
8. 工作区项目的 Onboarding 与 init commit 行为（F-08 未核实的部分），设置页对它显示什么。
9. 环境页在 `env.detect` 之后怎么得知探测结束（F-04）；需要动 `server/ticker.ts` / `server/main.ts` 接线时写明改什么。
10. `iterm2` 显不显示、能不能从网页安装（F-06）；终端用哪个（任务卡写 Terminal.app，旧面板优先 iTerm）。
11. 本地处理函数的拒绝怎么回给浏览器（F-11）：状态码与文案，不能被报成「内核未连接」。
12. `open.path` / `open.terminal` 在本任务里是否注册处理函数、哪个页面用到；注册的话 R-04b 的约束与 A-09 怎么覆盖，
    不注册的话它们保持 501 是否满足任务卡。
13. 「打不开终端」怎么判定、怎么注入假的终端启动器（A-07），以及怎样保证任何测试都不会真的拉起终端（§4 约束 9）。
14. 配置只读区显示哪些字段、字段名怎么配中文而不另写默认值；配置文件解析失败时（F-10）设置页还能不能打开、显示什么。
15. 移除成功后跳到哪里；确认框「输入项目名」比对的是什么；A-05「仓库文件不变」用什么断言。
16. 首页环境提示的数据来源（`env.get` 还是 `dashboard.get` 的 `environment`）与「还没检测」的显示（F-04）。
17. 「选择目录…」按钮位怎样做到服务端渲染时不出现、只在 `window.autome?.pickDirectory` 存在时出现，测试怎么断言。
18. A-10 用什么让会话自己能跑的办法验证（已有 `test/styles.test.ts` 用静态样式规则守「外壳约束页面高度、列表内部滚动」）；
    确实只能靠真浏览器肉眼确认的部分按协议写进 `## 人工验收清单`。
19. 256 × 1024 字符的产物经表单 action 提交是否有别的长度上限先拦住（F-12），A-03「保存超长内容被网关拒绝并有中文提示」
    的提示从哪一层来。

## 3. 范围

**可以改动的目录与文件：**

- `apps/web/app/routes/projects.new*`、`apps/web/app/routes/projects.$id.settings*`、`apps/web/app/routes/env*`（新建）；
  `apps/web/app/routes.ts`（路由表是显式列出的，新页面要加进去）；
- `apps/web/server/paths*`（新建）——目录补全与「路径在 `$HOME` 内」的校验；
- `apps/web/server/` 下为本地操作注册处理函数所需的新文件，以及 `gateway.ts`、`main.ts`、`http.ts` 的接线改动
  （注册 `project.confirm`、`env.install`、`env.login`，暴露补全接口；`open.path` / `open.terminal` 见 §2.2 第 12 问）。
  `server/ticker.ts` 只在 §2.2 第 9 问的答案需要时改，设计里写明缺什么；
- `apps/web/server/gates/write-gate.ts`——加 `project.remove`（只收 `project_id`）；为满足 R-04b / A-07 收紧
  `env.install` / `env.login` / `project.confirm` 的参数检查。`gates/read-gate.ts` 预期不需要改，确需改动时设计里写明理由；
- `apps/web/app/routes/projects.$id.tsx`（顶部报警、右上角「设置」入口）与它的数据模块 `app/components/board-data.ts`；
- `apps/web/app/components/`：`shell.tsx`、`project-list.tsx`、`shell-data.ts`、`palette-model.ts`、`nav-links.ts`
  （「添加项目」「环境」入口、报警标记），以及本任务页面用到的新组件与纯逻辑模块；
- `apps/web/app/routes/_index.tsx`（环境缺项提示）、`apps/web/app/root.tsx`（只为事件 → 重新加载的接线）、
  `apps/web/app/lib/labels.ts` 或就近文案表、`apps/web/app/lib/live-routes.ts`（只在现有匹配器不够用时）；
- `apps/web/app/styles/app.css`——只加本任务页面用到的样式，用 `tokens.css` 已有的 token；
- `apps/web/test/`——新测试与 `test/fixtures/` 下的替身；已有测试只在白名单、路由表、外壳入口变化引起的必要变化时改，
  逐条说明理由；
- `apps/web/package.json`、`package-lock.json`——预期**不需要**改。确需新增依赖时，设计里写明为什么不手写、
  `npm view <包> engines` 实测值、与 Node ≥ 22.22 的兼容性；
- `.github/workflows/ci.yml`（只 `web` job）、`scripts/ci-local.sh`、`apps/web/test/ci-config.test.ts`——**只在**新集成测试
  需要现有步骤给不了的东西时改，先实测并写明证据；不需要就不改。

**明确不做（出现即越界；前五条是任务卡「不做什么」原文）：**

- 角色、路由图、技能、协议页（D6：角色配置改文件）；
- 配置的网页编辑；
- 任务详情页（W7）、会话日志（W8）；
- 不改 `crates/`，不新增 JSON-RPC 方法；不改 `apps/desktop/`（原生目录选择归 W11）；
- 不做远程访问与登录体系；鉴权仍只有 §5.5 的 Host / Origin 检查；
- 不把 `project.add`、`events.since`、`env.detect`（读通道）放进白名单；不新开让浏览器按任意路径读文件、列文件
  （非目录）的接口；
- 不在网页上实现 `project.pick`（保持 501）；
- 内核给不了的信息，在设计里写成 Backlog 或争议项，不绕过去改内核；
- 启动与安装（W10）、Electron 薄壳（W11）、Playwright 端到端（W12）。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、`.autome/`。** 只能读或运行它们（构建并运行
   `target/debug/automed`、`target/debug/autome`）。不新增 JSON-RPC 方法。
2. **浏览器不能给出任意路径**（计划 §5.5）。补全的输入与每一条结果、`project.probe` 的路径、`project.confirm` 的路径，
   都要在服务端 realpath 之后确认仍在 `$HOME` 之内才放行；含 `..`、NUL 的输入直接拒绝。`project.confirm` 的校验
   必须在写入口这一层成立——绕过页面直接向 `/api/write` 发 `$HOME` 之外的路径同样被拒（A-02）。
3. **浏览器不能给出命令文本。** `env.install` / `env.login` 只收组件名；要执行的命令由服务端在执行当时向内核
   `env.install_recipe` 再取一次。请求体里的 `command`（或任何别的命令字段）被忽略或拒绝，不得影响实际执行的内容（A-07）。
4. **`open.path` / `open.terminal` 如果实现**，只接受现有 write-gate 允许的参数形状，解析出的路径必须落在已注册项目目录
   或 `AUTOME_HOME` 之内，路径一律来自内核回包，不来自浏览器。
5. **`project.add` 只能经 `project.confirm` 间接调用**，不进写白名单。`project.remove` 进写白名单，只收 `project_id`。
6. **§5.5 不放松也不加码。** 只绑 `127.0.0.1`；Host / Origin 检查沿用 `server/security.ts`，不加令牌、Cookie、登录会话，
   不加 CORS 头。本任务新增的每个写操作与补全接口都要有 Host / Origin 检查的测试（A-09）。
7. **配置只读。** 不做任何配置的网页编辑，不在白名单里现成的 `config.set_*` 上搭界面；显示的字段与值以内核回包为准，
   前端不另写默认值。
8. **移除只是取消注册。** 本任务的代码里不出现删除项目目录或仓库文件的操作；确认框写明这一点。
9. **测试不碰真实环境。** 启动内核、服务器或命令行的测试用 `mkdtemp` 的临时 `AUTOME_HOME` 与 `HOME`；会话用假 CLI
   （`AUTOMED_CLAUDE_BINARY` / `AUTOMED_CODEX_BINARY` 指向替身脚本，现成写法见
   `apps/web/test/gateway-task-lifecycle.test.ts`、`notifier-live.test.ts`、`test/fixtures/` 下的脚本，源头是
   `crates/automed/tests/end_to_end.rs` 的 `FAKE_CLI`）；**任何测试都不得真的执行 `osascript` 或打开终端窗口**，
   终端启动器必须可注入；不得向用户真实账本注册或移除项目。手工起服务验证时同样要指定临时 `AUTOME_HOME`
   （必要时再指定 `AUTOMED_BIN`）——不指定的话，服务托管的内核会去推进用户真实账本里正在跑的任务。
10. **替身对齐真实接口**，不是对齐调用方的想象：probe / add / env / config / dashboard 的回包形状取自真实内核的输出
    （或直接用真实内核），假 CLI 写出的文件与真实 Onboarding 会话一致。本项目四次「测试全绿、真机坏」都出在这里。
11. **服务端 TypeScript 走 Node 原生类型剥离**（ESM、`.ts` 后缀导入、`erasableSyntaxOnly`、`verbatimModuleSyntax`，无 tsx），
    与 W1–W8 一致；测试用 vitest（`fileParallelism: false`，超时 60 秒）；`npm run lint`（`--max-warnings 0`）、
    `npm run typecheck`、`npm run build` 必须无错误。
12. **界面硬规则**（2026-09-15 定下，计划 §2 重申）：用户可见文字全中文，专有名词、方法名、字段名保留原文；
    卡片优先、低密度；1512×944 下页面本身不出现纵向滚动（列表、编辑器、日志等可以内部滚动）；沿用现有外壳与 token
    （`app/root.tsx`、`components/shell.tsx`、`styles/tokens.css`），不引入新的 UI 库。内核拒绝时原样显示内核那句话
    （包括 F-09 那句英文）。内核离线时写按钮置灰，用外壳已有的 `useWriteGate()`。
13. **前端不维护业务状态副本。** 项目、报警、环境、配置只来自 loader，变化靠 `useLiveEvents()` 让受影响的路由重新加载
    （计划 §5.2），不整页刷新。表单与 action 的写法照 `app/routes/projects.$id.tasks.new.tsx` 与
    `app/components/task-new-action.ts`。
14. **里程碑不超过 6 个。** 计划 §7.4 的止损规则：设计文档的里程碑超过 6 个会被驳回并要求拆分。
15. **宿主门禁只在合并前跑一次**（`.autome/config.toml`：`[gate] command = "sh scripts/ci-local.sh"`、
    `run = "before_merge"`、超时 30 分钟）；每个实现轮结束后宿主另跑一次快检（`cargo fmt --check`、`cargo clippy`、
    Web lint 与 typecheck）。实现轮、审计轮只跑里程碑验收与直接相关的测试；A-11 的 `sh scripts/ci-local.sh` 全绿由
    合并前门禁与最后的审计给出，不要每轮都跑全量 ci-local。
16. **并行任务。** 生成时 W7（任务详情页，分支 `autome/w7-任务详情-…`）正在并行开发，它的目录是
    `apps/web/app/routes/tasks.$id*`。`app/routes.ts`、`app/styles/app.css`、`app/lib/labels.ts`、`server/gates/write-gate.ts`、
    `server/main.ts`、`components/shell.tsx` 是两边都可能碰的文件：对它们的改动越小、越集中，合并前 rebase 越省事。
17. 不切分支、不推送、不操作 `.worktree/` 下的其它目录；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`
    （已 gitignore），不进任务目录。

## 5. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（任务整理 → 设计 → 评审/裁决 → 实现/审计 → 合并）驱动 Agent 会话开发软件的
本地工具，仅支持 macOS，用自己的 Loop 开发自己。计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 把 Electron
渲染进程换成浏览器面板：`apps/web` 的 Node 服务托管 Rust 内核 `automed`（framed JSON-RPC over stdio），对浏览器提供
HTTP + WebSocket。W1（骨架）、W2（内核网关）、W3（事件总线）、W4（通知）、W5（外壳与导航）、W6（项目页与任务卡）、
W8（会话日志）已在 main 上；W7（任务详情）并行进行中；W10–W12 尚未进 main。现在面板只能看已注册的项目：添加、Onboarding、
移除、环境检查都得回命令行或旧的 Electron 面板，而 W11 会把旧渲染进程删掉。

计划里与本任务直接相关的决策：D1（Web 任务不碰 `crates/`）、D3（Electron 留作薄壳，原生目录选择由它提供）、
D6（面板不做角色，配置改文件）；§5.5 本机安全；§5.4（`project.alert` → 铃铛通知，已由 W4 实现）。

**技术栈与布局：**

- Rust workspace（`rust-toolchain.toml` 固定版本，edition 2024）：
  - `crates/autome-domain`——纯类型与状态转换。`project.rs`（`Onboarding`、`AddDisposition`、`Member`、
    `ProjectKind`）、`environment.rs`（`Component`、`ComponentStatus`、`Login`、`Severity`、`install_recipe`、
    `login_command`）、`config.rs`（`ResolvedConfig`：`bug`、`loop_defaults`、`roles`、`gate`、`gate_quick`、`setup`、
    `gate_run`、`merge_auto`、`merge_after`、`gate_same_failure_limit`、`gate_reuse_for_main_check`、`gate_concurrency`、
    `retry`；`validate`）；
  - `crates/automed`——SQLite 账本、Git、会话启动、调度、门禁、协议。`src/dispatch.rs`（5776 行）里本任务要调用的
    处理函数：`project_list`（566 行）、`project_get`（584）、`project_add`（668）、`project_probe`（863）、
    `project_remove`（903）、`onboarding_step` / `onboarding_run` / `onboarding_artefacts` / `onboarding_save`
    （930–1024）、`config_get` / `config_validate`（1070–1112）、`dashboard_get`（2456，`alerts` 与 `environment` 在
    约 2495–2528）、`alert_dismiss`（2558）、`env_get` / `env_detect` / `env_json` / `env_install_recipe`（2598–2680）、
    `READ_METHODS`（2793）。`src/scheduler.rs` `start_onboarding`（4814）、报警的产生（约 3585、3996、4181）；
    `src/env_probe.rs`、`src/config_io.rs`、`src/init.rs`；
  - `crates/autome-cli`——`autome` 命令行。`src/main.rs` `project`（约 301 行）是 add / ls / rm / onboard 的参照。
- `apps/web`——React Router 8 框架模式 + express 5 自定义服务器；TypeScript ~6.0、Vite 8、vitest 5、ESLint 10、
  React 19、`ws`；`engines.node >= 22.22.0`。现状：
  - `server/main.ts`：`startServer({ port, mode, home, binaryPath })` 依次建面板库、事件总线、logtail、`CoreSupervisor`、
    通知器、上传、`createGateway({ core, home, uploads })`、`createHttpServer`，监听后挂 `/ws`、启动内核与 ticker；
  - `server/gateway.ts`、`server/gates/{read-gate,write-gate}.ts`、`server/security.ts`、`server/home.ts`：见 §2.1 的
    F-11..F-13；`server/http.ts`：`/healthz`、`POST /api/read|write`、`POST /api/uploads`、`/api/notifications*`、
    `getLoadContext: createLoadContext({ gateway, unread, uploads })`；`server/core/supervisor.ts`：
    `CoreSupervisor.call(method, params)`（服务端直接调内核，不经白名单）、`tick()`、`restart()`；
    `server/ticker.ts`：每 3 秒一次 `scheduler.tick`；`server/uploads.ts`：一个现成的「服务端生成路径、浏览器只拿编号」的例子；
  - `app/load-context.ts`：`servicesContext`（`{ gateway, unread, uploads }`，键用 `Symbol.for`，Node / Vite SSR / 生产包
    三份模块实例共用——往 context 里加新服务时照这个写法，不要另 `createContext`）；
  - `app/routes.ts`：显式路由表，现有 `/`、`projects/:id`、`projects/:id/tasks/new`、`tasks/:id`；
  - `app/root.tsx`：loader 调 `loadShell`（`project.list` + 逐个 `project.get`），`shouldRevalidate` 见 F-03；
    `components/shell.tsx`（侧栏：品牌、总览、项目列表；顶栏：搜索、新建任务卡、铃铛）、`project-list.tsx`
    （空态文案现在让用户去命令行 `autome project add`）、`shell-data.ts`、`palette-model.ts`（`buildCandidates`）、
    `nav-links.ts`、`board-data.ts`（`loadBoard`）、`board-live.ts`；
  - `app/lib/labels.ts`（中文文案表）、`lib/live.tsx`（`useLiveEvents`、`writeGate`、`useWriteGate`、横幅）、
    `lib/live-routes.ts`（`liveShouldRevalidate`、`byKind`、`bySubjectParam`）、`lib/live-client.ts`；
  - `app/components/session-log*`（W8）：按 `session_id` 订阅并显示会话日志的组件；
  - `app/styles/{tokens.css,app.css}`（24 行 + 104 行）；
  - `test/`：65 个 `*.test.ts`，`fixtures/` 有 `fake-core.mjs`、`w4-ledger.sh`、`w5-states.sh`、`w6-project.sh`、
    `dashboard-double.ts`、`wait-for-transition.ts` 等。会被本任务碰到的守卫测试：
    `labels.test.ts`「7 views do not duplicate status, column or stage Chinese literals」——`app/routes/` 与
    `app/components/` 下**任何 `.tsx` 的源码文本**都不能包含 `STATE_LABELS`、`COLUMN_LABELS` 的值与两张阶段表的阶段名
    （按子串匹配），即「未开始 / 排队中 / 进行中 / 已暂停 / 已停止 / 失败 / 已完成 / 已取消 / 待开始 / 等你处理 /
    整理 / 设计 / 批准设计 / 实现 / 审计 / 合并 / 完成 / 定位 / 修复 / 复核」。本任务的文案天然会撞上它
    （Onboarding 的「完成」按钮、「检测失败」「合并后 main 门禁失败」等），这些字只能出现在 `labels.ts` 或 `.ts` 文案表里
    ——W6、W8 各在合并门禁上被它拦过一次；
    `styles.test.ts`（不散写颜色、圆角与阴影只用约定 token、无内联样式、无 emoji、页面高度受限）；
    `bell-view.test.ts`「目标可用性与真实路由表一致」；`write-gate.test.ts`（逐个操作的形状用例、`project.add` 不可达、
    两份门禁互不 import）；`read-gate.test.ts`（与 `READ_METHODS` 交叉核对）；`palette.test.ts`、`shell.test.ts`、
    `ci-config.test.ts`（ci.yml 与 ci-local 的 Web 步骤一致）。
- `apps/desktop`——Electron 外壳（CommonJS，`node --test`），本任务不改，只作参照：`main.js`（F-14）、
  `renderer/screens/{projects,project,env}.js`、`src/login-path.js`。
- `protocol/`——Loop 协议源（编进二进制）；`plugins/`——插件；`scripts/`——`ci-local.sh`、打包与安装脚本。

**本机（生成时）：** macOS 27.0.1，Node v26.5.0。本 worktree **没有** `apps/web/node_modules`，也没有 `target/`——
跑 Web 测试前先 `cd apps/web && npm ci`；需要真实内核的测试默认找 `<worktree>/target/debug/{automed,autome}`
（或环境变量 `AUTOMED_BIN` / `AUTOME_BIN`），先在 worktree 里 `cargo build -p automed -p autome-cli`。
Web 用例总数基线生成时未实测，设计轮装好依赖后自己取，并写明是 `npm test` 运行时报告的数。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm --prefix apps/web test`、
  `npm run build --prefix apps/web`；单文件：`cd apps/web && npx vitest run test/<文件>`
- 内核与命令行（只构建、不修改）：`cargo build -p automed -p autome-cli`
- 全部：`sh scripts/ci-local.sh`（rust：fmt、clippy、test；desktop；contract；web：lint、typecheck、test、build）
- CI：`.github/workflows/ci.yml` 的 `web` job（Install、Lint、Typecheck、`cargo build -p automed -p autome-cli`、Test、Build），
  由 `apps/web/test/ci-config.test.ts` 守卫，须与 ci-local 保持一致。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 `README.md`，无具体规则）；仓库内无
  `docs/agent-project-profile.md`；
- `docs/development/requirements.md`（P-01 添加项目、P-07 项目列表、P-08 移除项目、C-01..C-03 初始化 / Onboarding /
  init commit、E-01..E-05 本地环境）、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2（D1、D3、D6 与 UI 硬规则）、§5.2、§5.4、§5.5、
  §6 的 W9（以及 W7、W10、W11、W12 的边界）、§7.4；
- 前序任务的设计文档与教训：`docs/w5-外壳与项目导航-迁移-animal/`、`docs/w6-项目页与任务卡-按待开始-进行中/`、
  `docs/w8-实时会话日志/`（各有 `lessons.md`）、`docs/apps-web-的服务端托管-automed-2/`（W2 网关）。

**前车之鉴（此前 Web 任务的复盘教训，供参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴 watcher EMFILE），审计轮可以。此前的 Web 任务（W1–W6、W8）都撞上过。
  路径校验、补全筛选与截断、probe 结果到表单的映射、报警筛选、环境与配置的视图模型尽量写成纯函数或用假的网关测；
  必须监听或起真实内核的用例集中在少数文件。跑 Web 全量之前先跑一个不加载项目源码的 listen 探针；探针报 EPERM 时
  不要反复跑全量，验收契约各段分别给退出码，写明被拦住的用例数，交审计轮复验。
- `npm run typecheck` 与 `npm run lint` 是静态检查，不受 EPERM 限制：实现轮报待审之前当场跑，把退出码写进证据
  （W6 为此多花过一个实现轮）。报待审之前跑一次 `cd apps/web && npx vitest run` 全量，逐条标明失败是否为 EPERM
  （W6、W8 的文案守卫都是到合并门禁才被拦下）。
- vitest 的 `beforeAll` / `afterAll` 及其辅助函数里不要用 `expect.poll` 或 `expect(`，等待用 `vi.waitFor`；
  同一个仓库不要重复 `project.add`（W8 的集成测试因此 reopen 过一次）。
- 写进断言的耗时上限，要逐项列出计时起点到终点之间的固定等待（ticker 3 秒、轮询间隔等）并留出余量。
- 设计里的用例数预期写验收命令**运行时**报告的数；数字依赖环境时在同一句写明各环境下的值。
- 设计对任务要求或验收标准加大小、数量、场景上的限定时，必须引用任务卡里允许该限定的原文。
- 设计写「某已有测试不受改动影响」时，要附上临时施加改动后跑全量得到的失败清单，不能只凭读代码。
- 并行冲突风险要逐个列出本设计将修改的每个已有文件，并注明 W7 是否也会改它。
- 实现轮不改任何「M-xx 验收契约」小节的文字；负向对照（临时注入）还原后 `git status --porcelain` 必须为空并写进证据。
- 里程碑关闭之后又改了产品源码（含解决 rebase 冲突），要把受影响的里程碑重新标为 `待审`。
- 门禁或全量里出现任务范围外（`crates/`、`apps/desktop/`）的失败：不在本任务修，在设计文档 `## Backlog` 写一条，
  含用例全名、文件行号、断言原文与机制。
- `## 里程碑` 一节只放那一张表；逐轮证据写进 `evidence/`，不进设计文档。
