# W9 项目管理与环境：网页上添加项目（服务端路径校验与目录补全）、Onboarding、移除项目、项目设置页、环境检查页

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6「W9 项目管理与环境」。W5（T-35，外壳与项目列表）、W6（T-58，项目页与任务卡）已合并。现在网页面板只能看已注册的项目：添加、Onboarding、移除、环境检查都还得回命令行或旧的 Electron 面板，而 W11 会把旧渲染进程删掉。

网关已经为这些操作留好了位置但没有实现：

- `apps/web/server/gateway.ts` 的 `localOps`（`project.pick`、`project.confirm`、`open.path`、`open.terminal`、`env.install`、`env.login`）没有注册处理函数，现在统一回 501「网页端还不支持」。
- 读白名单已放行 `project.probe`（路径限制在 `$HOME` 内）、`project.onboarding.artefacts`、`config.get`、`config.validate`、`env.get`、`env.install_recipe`；写白名单已放行 `project.onboarding.{advance,skip,run,save}`、`project.alert.dismiss`、`env.detect`。
- 写白名单里**没有** `project.remove`，内核的 `project.add` 也只能经 `project.confirm` 这个本地操作间接调用。

另外两处与需求的差距（2026-11-13 对照 main 上的面板与内核）：

- 内核的项目报警（`project.get` 返回的 `alerts`，例如「合并后 main 门禁失败」）在网页上只会变成一条铃铛通知，项目页上看不到当前有哪些未消除的报警，也没有地方消除。
- 配置按 D6 只在文件里改，但面板上看不到「内核实际按什么配置在跑」，配置写错（`config.validate` 的警告）也只能在命令行里发现。

## 要做什么

1. **添加项目** `apps/web/app/routes/projects.new*`
   - 输入路径，服务端目录补全：新增 `apps/web/server/paths*`，只列 `$HOME` 之内的**目录**（不列文件），对输入与结果都做 realpath 后再判断是否仍在 `$HOME` 内（符号链接指到外面的不列）；拒绝含 `..`、NUL 的输入；单次最多返回 50 项；默认不列以 `.` 开头的目录，输入本身以 `.` 开头时才列。补全是只读接口，同样过 §5.5 的 Host 检查。
   - 确认之前先调 `project.probe`，把结果显示出来：是否 Git 仓库根、是否像工作区（`suggest_workspace`）、成员仓库与建议的文档仓库（`suggested_docs_repo`）、内核给的拒绝原因。
   - 表单项：路径、是否工作区、文档仓库（下拉自 probe 的成员，只传名称）、是否跳过 Onboarding。提交走写入口的 `project.confirm`：在 `server/` 里为它注册处理函数，重新校验路径在 `$HOME` 内后调内核 `project.add`。成功后跳到项目设置页（没跳过 Onboarding 时）或项目页。
   - `project.pick`（原生目录选择）在网页里保持 501；页面上预留一个「选择目录…」按钮位，只有 `window.autome?.pickDirectory` 存在时才显示（接口由 W11 提供）。
   - 左侧项目列表底部与 ⌘K 增加「添加项目」入口。
2. **项目设置页** `apps/web/app/routes/projects.$id.settings*`
   - Onboarding：显示当前步骤；「运行」（`project.onboarding.run`，运行中显示会话状态，结束后实时刷新）；查看并编辑两份产物——项目画像 `docs/agent-project-profile.md` 与 `AGENTS.md`（`project.onboarding.artefacts` 读、`project.onboarding.save` 写，长度上限与 write-gate 一致）；「完成」（`advance`）与「跳过」（`skip`）。完成或跳过后显示 init commit 的结果。
   - 配置（只读）：显示 `config.get` 解析后的值——并行数、门禁命令与运行时机、快检、重试、会话时长上限等内核返回的字段，以及 `config.validate` 的警告，并写明两份配置文件的路径（`~/.autome/config.toml`、`<项目>/.autome/config.toml`）。不提供编辑。字段清单以内核返回为准，不在前端另写一份默认值。
   - 移除项目：把 `project.remove` 加进写白名单（只收 `project_id`，按现有 id 校验口径）；按钮需要二次确认（输入项目名）；内核拒绝时（还有未结束的任务）原样显示内核那句话。移除只是取消注册，不删任何文件——确认框里写明这一点。
   - 项目页右上角加「设置」入口。
3. **项目报警** 项目页（`projects.$id.tsx`）顶部显示未消除的报警（`project.get` 的 `alerts`）：内容、时间、相关任务链接、「知道了」（`project.alert.dismiss`）。左侧项目列表里有报警的项目带一个标记。报警出现与消除都实时更新（`project.updated` / `project.alert_dismissed` 事件）。
4. **环境检查页** `apps/web/app/routes/env*`
   - 显示 `env.get`：Git、Claude Code、Codex 的版本、路径与登录状态；「重新检测」走 `env.detect`（检测中显示进行状态，`env.probing` 事件）。
   - 安装与登录：页面显示 `env.install_recipe` 返回的命令原文与前置条件是否满足；点「在终端里执行」走写入口 `env.install` / `env.login`，由服务端打开 Terminal.app 执行。**浏览器只传组件名（`git` / `claude` / `codex`），要执行的命令由服务端当场再向内核取一次**，不接受浏览器传来的任何命令文本。`open.terminal`、`open.path` 同理：只接受现有 write-gate 允许的参数形状，路径必须解析到已注册项目目录或 `AUTOME_HOME` 之内。
   - 非 macOS 或打不开终端时，给出命令原文和「复制」按钮，不报内部错误。
   - 左侧栏底部加「环境」入口；首页总览在环境有缺项（未安装 / 未登录）时给一条提示并链接到环境页。
5. 中文文案进 `app/lib/labels.ts` 或就近的文案表，沿用 W5/W6 的口径；内核离线时这些页面的写按钮置灰（沿用外壳已有的状态）。

## 不做什么

- 角色、路由图、技能、协议页（D6：角色配置改文件）。
- 配置的网页编辑。
- 任务详情页（W7）、会话日志（W8）。
- 不改 `crates/`，不新增 JSON-RPC 方法；不改 `apps/desktop/`（原生目录选择归 W11）。
- 不做远程访问与登录体系；鉴权仍只有 §5.5 的 Host / Origin 检查。

## 验收标准

1. 目录补全：只返回 `$HOME` 内的目录；`..`、NUL、绝对路径逃逸、指向 `$HOME` 之外的符号链接各有一个测试；结果上限有测试。
2. 添加项目：确认前一定调过 `project.probe`（测试）；`project.confirm` 的路径在服务端再次校验（测试：直接向写入口发 `$HOME` 之外的路径被拒）；单仓库与工作区各走通一次（对真实 automed 的集成测试，临时目录里建仓库）；内核拒绝时显示原文。
3. Onboarding：对真实 automed 与假 CLI 走完「运行 → 编辑两份产物并保存 → 完成」，仓库里出现 init commit；「跳过」同样生成 init commit；保存超长内容被网关拒绝并有中文提示。
4. 工作区项目：probe 提示、成员与文档仓库选择、添加后项目页正常显示（集成测试）。
5. 移除：有未结束任务时被内核拒绝并原样显示；没有时移除成功、左侧列表实时少一项；仓库文件不变（测试）。
6. 报警：造一条项目报警 → 项目页顶部出现、左侧列表有标记 → 点「知道了」后消失，全程不手动刷新（集成测试）。
7. 环境页：三个组件的状态与内核一致；`env.install` / `env.login` 的请求体带任意 `command` 字段时被忽略或拒绝，实际执行的命令等于内核 recipe（测试用假的终端启动器记录收到的命令）；打不开终端时页面给出命令原文。
8. 配置只读区：显示的值与 `config.get` 一致；配置里写一个非法值时页面显示 `config.validate` 的警告（测试）。
9. §5.5：本任务新增的每个写操作与补全接口都有 Host / Origin 检查的测试。
10. 1512×944 下新增的三类页面 `scrollHeight ≤ clientHeight`（内部滚动区域除外）。
11. `npm --prefix apps/web` 的 lint、typecheck、test、build 全绿；`sh scripts/ci-local.sh` 全绿。

## 参考

- 计划 §5.5（本机安全）、§6 W9、§2 的 D6。
- 网关：`apps/web/server/gateway.ts`（`localOps`、`registerLocalOp`）、`apps/web/server/gates/{read-gate,write-gate}.ts`、`apps/web/server/security.ts`、`apps/web/server/home.ts`。
- 内核：`crates/automed/src/dispatch.rs` 的 `project_probe`、`project_add`、`project_remove`、`onboarding_step` / `onboarding_run` / `onboarding_artefacts` / `onboarding_save`、`env_get` / `env_detect` / `env_install_recipe`、`config.get` / `config.validate`、`project.get` 的 `alerts`。
- 旧面板的做法（只作参考，不迁移代码结构）：`apps/desktop/renderer/screens/{projects,project,env}.js`、`apps/desktop/src/main/` 里打开终端与目录选择的实现、`apps/desktop/src/login-path.js`。
- W5/W6 产物：`apps/web/app/components/{shell.tsx,project-list.tsx,palette-model.ts,board-data.ts}`、`apps/web/app/load-context.ts`、`apps/web/app/routes/projects.$id.tsx`、`apps/web/app/routes/projects.$id.tasks.new.tsx`（表单与 action 的写法照它）。
- 设计 token 与硬规则：`apps/web/app/styles/tokens.css`、`docs/development/` 下的设计规范。
