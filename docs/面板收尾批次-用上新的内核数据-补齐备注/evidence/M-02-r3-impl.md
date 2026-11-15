# M-02 实现轮 #3

结论：A-4～A-7 实现完成，契约验收 74/74，申请待审。implementation-round 为 3/65。上一轮审计只关闭 M-01，没有退回 M-02 的缺陷或判别检查，因此本轮不涉及审计检查搬入。

## 改动与依据

- 添加项目页按 probe.rejection 展示四种拒绝原文与项目链接，未知 kind 保留 message；有拒绝时不生成确认表单。提交仍经过原有网关与内核。
- 设置页读取 project.get.onboarding_session，使用同一个 SessionLog 组件按 session id 读日志并跟随。正常退出、非零退出、killed、timed_out、vanished 分别显示正常结束、崩溃、被停止、超时、进程消失。源码核对：内核超时是 lifecycle.state=timed_out，不能读取假设的 timed_out 布尔字段。
- 删除 onboarding watch、tick 轮询和扩展结束推送；保留 onboarding-disk 的单次读取。仅账本 running 时调用，且只把同一个 session id 的退出/中断标记作为兜底；读盘错误保留原文。账本结束后不读盘。只读内核下仍需刷新页面才能读取磁盘兜底，这是设计 A-5 明示的限制。
- configRows 接收 defaults、按顶层段标记默认；角色显示来源；补齐 gate_quick_concurrency、setup、merge_after 子键标签。m02-config-get.json 来自本检出真实 automed、临时 HOME/AUTOME_HOME、临时注册项目的 config.get 回包；配置命令子项另用已有真实配置夹具及显式设置检查。
- dashboard.config_error 进入侧栏与总览的独立警告，不计入可消除报警；项目页显示校验原文，task.get 失败只丢该卡详情。设置页 config.get 失败保留原文且不调用 config.validate。
- 原有 task-card 测试按新的逐卡容错契约调整：task.get 失败必须保留卡片并无详情，dashboard.get 失败仍保留原错误断言。新增配置错误滚动区登记进 testids 的封闭白名单（7 → 8），没有放宽任意滚动区。

## 命令与结果

原始输出均在 `.autome/output/`，不复制到任务文档。

1. 改前同范围基线（onboarding-disk 用旧名 onboarding-watch）：9 文件 **62/62**，日志 `r3-baseline.txt`。
2. 最终契约命令：

```sh
cargo build -p automed -p autome-cli && (cd apps/web && npx vitest run test/add-project.test.ts test/settings-config.test.ts test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/onboarding-disk.test.ts test/shell-data.test.ts test/board.test.ts test/overview.test.ts test/test-groups.test.ts) && test ! -e apps/web/server/onboarding-watch.ts && test -e apps/web/server/onboarding-disk.ts && ! grep -rqE "onboarding-watch|onboarding\.ended|createOnboardingWatch" apps/web/server apps/web/app apps/web/test
```

退出 0；9 文件 **74/74，失败 0、跳过 0**。日志 `r3-final-build.txt`、`r3-final-accept.txt`。比上一实现轮本范围的改前基线 **+12**：add-project 11→15（+4），settings-config 11→14（+3），settings-onboarding 7→8（删扩展推送 1、加账本状态/兜底 2，净 +1），onboarding-watch 5→onboarding-disk 4（−1），shell-data 6→8（+2），board 8→10（+2），overview 6→7（+1），real 4 与 test-groups 4 不变。真实用例将结束通知换为账本 session.ended 驱动持久 router 重验，并在原用例内覆盖 session.log 正文和 exit 4。

3. `npx vitest related --run <本轮 app/server 改动文件>`：46 文件、**285 用例**，226 通过、38 失败、21 因 suite setup 失败而未运行。1 个实现配套问题为新滚动区未登记，已修复；其余失败为 `listen EPERM 127.0.0.1` / `spawnSync ps EPERM`，另有开发 SSR watcher 的 EMFILE 输出。日志 `r3-related.txt`、`r3-related-summary.txt`。没有修改测试跳过条件或弱化断言。
4. 修复后同一相关依赖选择限制为可运行的 `--project unit --project kernel`：32 文件、**213/213**，日志 `r3-related-supported.txt`。285→213 的 −72 全部来自显式排除需要端口/进程扫描的 exclusive 项目，不能称全体相关用例通过；原 285 中这 213 项为 212 通过 + 1 个滚动区失败，修复后 213 通过。
5. `npx vitest run test/core-sidecar.test.ts test/testids.test.ts`：**30/30**，日志 `r3-guards.txt`。测试数不变，包含无条件跳过守卫与滚动区白名单。以上重叠子集不相加冒充唯一用例总数。
6. 修改文件定向 ESLint、`npx tsc --noEmit`、`git diff --check` 均退出 0；`npm run build --prefix apps/web` 成功。日志 `r3-lint.txt`、`r3-tsc.txt`、`r3-web-build.txt`。未运行 ci-local、ci-quick 或完整项目门禁。
7. 页面补充巡检：`npx playwright test e2e/layout.spec.ts -g '项目设置|添加项目补全|1512×944 项目$|1512×944 总览$'`，目标 4 用例，实际 **0 个执行**。globalSetup 在启动服务之前执行 ps 被拒（EPERM），teardown 同样受限。日志 `r3-layout.txt`。无截图与真实浏览器视觉通过证据，由宿主复验；本轮仅做 SSR 结构和现有样式核对。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计行 | 执行检查 | 结果 |
|---|---|---|
| A-4 already_project、nested_in_project、contains_project、not_a_directory | add-project 12～14：原文、中文标题、编码链接、无确认表单 | 通过 |
| A-4 未知 kind（含 toString/__proto__/空串）、合法 probe | add-project 15：message 回退、null 可确认；原有确认动作仍走网关 | 通过 |
| A-5 none/running/正常/崩溃/停止/超时/消失 | settings-onboarding 7 使用真实 lifecycle 形状逐项渲染，session id 对应日志入口 | 通过 |
| A-5 日志与事件 | real 首例实际 session.log 有假 CLI 输出，project subject 的 session.ended 触发持久 router 得到正常结束；第三例 exit 4 | 通过 |
| A-5 无收割兜底 | settings-onboarding 8 覆盖 running + 磁盘 ended/interrupted/none/running；账本结束后读盘次数 0 | 通过 |
| A-6 全部叶子、默认段、角色来源 | settings-config 12～14，真实回包逐叶子核对与默认集合双向相等，来源徽标 SSR | 通过 |
| A-7 总览/侧栏、其它项目隔离、修复恢复 | shell-data 7～8、overview 7：独立警告，不增报警计数，修复后消失 | 通过 |
| A-7 看板卡片降级、设置错误原文 | board 9～10、settings-config 4：卡片保留、详情为空、原文转义显示、不调用 validate | 通过 |

### 2. 失败分支

- 拒绝授权、读盘失败、超时、内核不可达：原有 add-project、settings-onboarding、settings-config、shell-data、board 的错误矩阵随最终 74 用例执行；错误原文保留。
- Onboarding 磁盘读取失败：onboarding-disk 第 4 例以目录占据 .exit 路径触发错误，返回 interrupted 与错误原文；无目录返回 none。
- 空输入、过长输入和非法产物路径：原 settings-onboarding 保存边界与网关拒绝用例执行。
- 外部第三方服务不可达：不适用；本里程碑未引入外部服务，内核离线分支已覆盖。

### 3. 全部迁移

- 账本无会话 → 运行 → 正常结束：真实 run/tick/session.ended/router 重验链路；运行 → exit 4 崩溃：真实内核用例。
- 运行 → killed、timed_out、vanished：单元按内核 lifecycle 载荷覆盖（不为超时人为等一分钟）。磁盘 running → ended/interrupted 与仍 running/none、账本终态优先和不再读盘：兜底矩阵逐项执行。
- 项目配置正常 → 错误 → 修复：shell-data 7 在同一 gateway 上重新加载检查警告出现/消失；错误项目不影响另一个项目，board 单卡拒绝后基础行保留。
- Onboarding 步骤 3→4→5→completed、skip→skipped、各步失败不继续写：原有动作与真实步骤事件测试继续执行。

### 4. 输入域边界

- 路径空串、非法保存路径、正文空/单字符/262144 最大值/262145 超限：原设置测试逐项执行。
- session id 正则、无目录、最新会话选择、存活/死亡 pid，prompt 年龄 9999/10000/10001ms：onboarding-disk 四例保留并通过。
- 配置 null、空串、0、false、空对象、未知键（含 toString）、角色来源、默认集合之外不得标默认：settings-config 通过。
- 日期闰日/跨年与前导零任务号：原 board/shell-data 边界用例继续执行；本轮没有新增日期解析域。

## 负向对照

每次只改一个生产分支，运行指定单例，退出 1 后 finally 还原原文件；最终契约与相关测试在还原后重新通过。

| 变异 | 判别用例 | 结果/日志 |
|---|---|---|
| canConfirm 不看 rejection | add-project：四种拒绝在填表前 | 红，`r3-negative-probe.txt` |
| 任意 task.get 失败返回整页错误 | board：坏配置下卡片仍渲染 | 红，`r3-negative-board.txt` |
| configRows 忽略 defaults | settings-config：默认段全部且只有它们有默认徽标 | 红，`r3-negative-defaults.txt` |
| 账本 running 时不读磁盘 | settings-onboarding：账本运行中才读磁盘标记 | 红，`r3-negative-disk.txt` |

## 环境与清理

真实内核测试全部使用 w9Kernel 临时 HOME/AUTOME_HOME 和本检出 target/debug/automed，close/afterEach 等待 core.stop 并清理临时目录。额外配置抓取在 finally 中 close；没有接触真实账本、launchd 或已安装面板。Playwright setup 的进程扫描在 spawn 之前失败，因此没有启动浏览器/服务。相关 exclusive 进程表复核受 ps EPERM 阻塞，不能声称完成系统级无残留扫描；没有按进程名结束任何用户进程。环境限制不属于协议失败，宿主需复验相关监听/浏览器用例。
