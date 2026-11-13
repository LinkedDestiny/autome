# M-06 · 实现 #8

结论：代码与非监听验收已完成；M-06 保留 **开放**。完整验收的 9 个 HTTP 用例全部在服务监听阶段遇到 `listen EPERM: operation not permitted 127.0.0.1`，本轮权限禁止提权，无法取得完整通过证据。不把环境阻塞写成协议失败，也不标待审或已完成。

## 改动与依据

- 设计 §7：新增 `/env`、env-model、组件状态/recipe/登录展示、强制检测、命令复制和终端不可用提示；首页提示沿用根 loader 的 environment。显示内核全部组件，包括 iTerm2。
- `admin` 注册 install/login，仅接受 write-gate 放行的 component，当场重新读取 recipe；拒绝额外键、缺前置条件及没有登录命令的组件。
- Terminal 启动器可注入；仅 runFromEnv 使用真实 macOS 启动器。默认不可用，测试不执行 osascript；macOS 引号/反斜线转义及启动失败有替身测试。
- tick 首份有效报告与 generation/probing 变化发布 env.changed；重复报告不发布；前端订阅刷新。保留原 Onboarding tick 行为。
- 已有测试只增设计指定的 write-gate 1 例、HTTP 2 例、布局 1 例；HTTP setup 增加可注入终端以记录调用。无依赖变化，无内核改动。
- `node apps/web/test/fixtures/w9-env-capture.ts`：在临时 HOME/AUTOME_HOME 采集真实环境和四组件 recipe 至 `test/fixtures/w9-env-replies.json`。场景 fixture 的 recipe 原文取自采集结果，仅将 prerequisite_ok 固定为 true 构造成功路径；失败路径显式置 false。真实内核测试另校验字段集合和实际命令。
- 上一轮审计为 M-05 通过，没有退回检查需要搬入和红绿复现。

## 命令、结果与用例基线

1. 里程碑验收命令：
   `cd apps/web && npx vitest run test/env-ops.test.ts test/env-page.test.ts test/env-real.test.ts test/write-gate.test.ts test/w9-http.test.ts test/w9-layout.test.ts test/overview.test.ts test/shell.test.ts test/labels.test.ts test/styles.test.ts`
   - 最终 10 文件、**106 例：97 通过、9 失败、0 跳过**。9 失败全部是 HTTP 文件监听 EPERM，不是断言失败；不得视作通过。原始输出 `.autome/output/m06-r8-tests-final.txt`。
   - 相对上一轮 M-05 基线 73 例，**+33**：两个验收集共有的 5 文件由 34→37（HTTP +2、布局 +1）；退出本轮范围的 6 文件共 39 例；进入范围的 env 三文件 17、write-gate 47、overview 5 共 69 例。73 − 39 + 69 + 3 = 106。相对设计 M-06 预期 106，差为 0。
   - 首次执行 96 通过、10 失败，其中 env-page 第 7 例测试调用 shouldRevalidate 缺 URL 参数；补齐 currentUrl/nextUrl/currentParams 后通过。其余 9 例一直为同一监听限制，未跳过或削弱断言。
2. 直接相关回归：`cd apps/web && npx vitest run test/onboarding-watch.test.ts test/settings-config.test.ts`：2 文件、12 例全部通过，输出 `.autome/output/m06-r8-related.txt`。
3. 最后一次仅修改 env-ops 的 mock 类型标注后：`cd apps/web && npx vitest run test/env-ops.test.ts`：7 例通过，输出 `.autome/output/m06-r8-ops-final.txt`。这是重复验证，不增加用例总数。
4. 本轮去重总数 **118**（106 + 12），109 通过、9 环境阻断失败、0 跳过；对上一轮执行范围 73 例为 **+45**，其中额外 12 例用于验证 admin 共享接线未破坏 Onboarding/配置。
5. `git diff --check` 通过。未运行项目全量门禁、lint/typecheck/build 或 Rust 快检，按本轮指令由宿主承担。

## 标待审之前的自审清单（已执行，阻塞项明确保留）

### 1. 情形表逐行

- 四种 login、已安装/未安装、版本/路径、problems 标记：env-page #1 SSR 检查通过；env-real #1 loader 与真实 env.get 整包及组件数一致。
- 未检测→正在检测且不显示缺项；已检测时重新检测中→显示进行状态、禁用重复检测：env-page #1/#3 通过。
- recipe 原文、note、前置满足/不满足、登录命令与离线三类写按钮禁用：env-page #2 通过。
- 安装/登录即时读取 recipe、执行原文、成功提示/无法启动时原文和复制：env-ops #1/#3/#5、env-page #4、env-real #2 通过。
- 总览提示只在 probed 且存在问题时显示，侧栏环境链接：env-page #5/#6 通过。
- env.probing 与 env.changed 刷新、首份报告推送与重复报告抑制：env-page #7、env-ops #7、env-real #3 通过。
- Host/Origin 与普通 action/single-fetch 安全边界：测试已加入，两条新增 HTTP 例及七条原有例均因监听 EPERM 未取证，待允许监听环境复验。
- 限高、内部滚动、标签和样式守卫通过；真实 1512×944 测量仍属 H-01，未取证；H-03 真实终端、H-04 剪贴板未执行。

### 2. 失败分支

- 任意 command/额外键、非法 component：env-ops #2，400 且内核/终端零调用。
- prerequisite 不满足：env-ops #4，409 原文且不启动终端；git 无 login_command：#3，400 且不启动终端。
- 终端不可用、非 macOS、execFile 拒绝：env-ops #5/#6，降级到 TerminalUnavailable，返回命令用于复制。
- 内核拒绝授权、超时、服务不可达：env-ops #4，拒绝且终端零调用；env-page #1/#2 的读取错误、读盘失败、recipe 超时保留原文，页面照常显示可用信息。
- 空 intent、无网关：env-page #3 拒绝；空 component 由网关测试拒绝。
- 浏览器授权弹窗/剪贴板拒绝是人工域 H-04，代码捕获异常并显示手动复制提示，本轮未做浏览器取证。网络服务不可达无需真实外网：本页依赖本地内核，由错误替身验证，安装命令不在本轮实际执行。

### 3. 全部迁移

- 检测：未检测→探测中（page #1）；已检测→force 检测中（page #3/real #3）；检测中→新 generation 完成（real #3）；首份即完成→发布一次、probing false→true→false、generation 独立改变→发布、重复报告→不发布（ops #7）。无效/缺报告→不发布。
- 执行：点击安装/登录→action 写入口（page #3）；服务端 recipe→成功启动（ops #1/#3）；前置拒绝/无登录命令/内核错误→零启动（ops #3/#4）；启动不可用→命令回退（ops #5、page #4）。
- 通道：账本 env.probing→环境页 revalidate；扩展 env.changed→全局 revalidate（page #7）；非相关消息不重载。
- 在线→离线写按钮禁用由 SSR gate false 覆盖；不适用持久化状态写回，本页业务状态只来自 loader。

### 4. 输入域边界

- 四个允许组件 git/claude/codex/iterm2 均通过 write-gate；空串、大小写错误、尾空格、未知名、null、数字、513 字符拒绝（ops #2）。额外键即使值为 null/false 也拒绝。
- 命令双引号、反斜线的 AppleScript 转义（ops #6）；命令只能来自内核，不存在浏览器任意命令输入域。
- 无组件/未探测环境不输出缺项；零个 problems 不输出首页提示；首个有效 tick 与重复 tick 均已覆盖。
- 不适用：日期、闰日、跨年、前导零及数值最大值；本页没有这些可编辑输入域。

## 负向对照

逐次暂改一处、执行相应项目测试，再用 finally 恢复源码；最终验收在全部还原后执行。

- 移除 env.install/login 的额外键拒绝：`npx vitest run test/env-ops.test.ts`，第 2 例变红。
- detect params 从 `{ force: true }` 改 `{}`：`npx vitest run test/env-page.test.ts`，第 3 例变红。
- 首份 tick 只记基线不发布：`npx vitest run test/env-ops.test.ts`，第 7 例变红。
- 每次 tick 都发布：同命令，第 7 例变红。
- 输出分别为 `.autome/output/m06-r8-negative-{command-keys,force,first-report,repeat-report}.txt`。还原后这些用例均绿。

## 下一步

在可监听 127.0.0.1 的执行环境重跑本里程碑原验收命令。106 例全过后才能标待审；人工项继续沿用设计 H-01/H-03/H-04，不宣称已完成。
