# M-01 · 实现 #1

结论：实现与无监听验收通过；完整验收被沙箱监听权限阻塞，M-01 保持「开放」，未标待审。implementation-round 为 1/30。没有上一轮审计，故没有需逐字移植的审计回归检查。

## 改动与修复

- 新增 paths.ts：HOME realpath 守卫、最深存在祖先处理、完整输入校验、只列目录、符号链接逃逸过滤、隐藏目录前缀、50 项上限。
- 新增 admin.ts：confirm 的守卫 → probe → add → 可选 skip；拒绝整个 HOME、空工作区；skip 失败保留已添加的项目并返回原文。
- gateway 的 LocalOpError 映射及 probe 守卫；write-gate 加 remove 和 confirm 严格参数；main/http/load-context 接线，沿用 securityMiddleware。
- 新增四个测试文件 25 条，write-gate 新增 2 条；真实内核回包以 Node 原生 TypeScript 抓取到 fixtures/w9-replies.json，抓取程序为 fixtures/w9-replies.ts。
- 必要旧测试变更：write-gate 的 confirm 参数补必填 path、全白名单检查增加 remove；gateway 的本地操作测试同样补 path，继续断言未注册 501、注册后可调。设计原先认为 gateway 不受影响，但实跑证明必填校验先于处理函数查找，已就地修正设计说明。没有删除或跳过测试。
- 初次运行 120 条中 17 失败：14 条监听 EPERM；另 3 条是测试假设错误（macOS 大小写不敏感目录、project.list 的 project 嵌套、旧 confirm 缺 path）。已修正，并在最终完整命令中证明这 3 条通过。

## 命令与结果

工作目录除构建外均为 apps/web；原始输出只在 .autome/output/。

1. `cargo build -p automed -p autome-cli`：退出 0（m01-build.txt）。
2. `node apps/web/test/fixtures/w9-replies.ts`（仓库根）：退出 0，真实 probe/add/skip 回包抓取成功。先前尝试 npx tsx 因网络不可用失败，已改用项目规定的 Node 原生运行，无新增依赖。
3. 完整验收：`npx vitest run test/paths.test.ts test/project-confirm.test.ts test/project-confirm-real.test.ts test/w9-http.test.ts test/write-gate.test.ts test/read-gate.test.ts test/gateway.test.ts test/gateway-http.test.ts test/load-context.test.ts`：退出 1；9 文件、120 用例，106 通过、14 失败、0 跳过。14 条均在 server.listen 阶段报 `listen EPERM: operation not permitted 127.0.0.1`，分属 w9-http 3 与 gateway-http 11；不是业务断言失败。另有 Vite watcher 的 EMFILE 诊断。日志 m01-acceptance.txt。未尝试绕过沙箱。
4. 无监听子集：`npx vitest run test/paths.test.ts test/project-confirm.test.ts test/project-confirm-real.test.ts test/write-gate.test.ts test/read-gate.test.ts test/gateway.test.ts test/load-context.test.ts`：退出 0，7 文件、106 用例全部通过、0 跳过（m01-subset.txt）；后续补强后的这 106 条也在最终完整验收中全部通过。
5. `git diff --check`：退出 0。

用例总数差：无上一实现轮，使用设计轮记录的同范围基线。完整命令的既有 5 文件为 93 条（44+15+13+11+10），本轮 120 条，增加 27＝新四文件 25＋write-gate 2；无监听同范围基线 79，本轮 106，同样 +27。旧 gateway 只改参数不增减数量。没有把原有全量 535 条当成本轮执行量。

按本轮用户指令，未运行 ci-local、cargo fmt/clippy、Web lint/typecheck/build 等宿主门禁。宿主快检仍待执行。

## 自审清单（逐项执行）

### 1. 情形表逐行

| 设计 §2/§3 情形 | 检查与结果 |
|---|---|
| 只列目录、码点顺序、~/ 显示 | paths 第 1 条通过，文件被过滤 |
| .. 子串、NUL、反斜杠、相对路径 | paths 第 2、3 条通过，check 与 complete 同时拒绝 |
| 绝对路径在 HOME 内/外、同名前缀兄弟目录 | paths 第 4 条通过 |
| 链接逃逸、尾斜杠、有子路径、半截前缀、断链 | paths 第 5 条通过；完整输入拒绝，半截前缀空结果 |
| 50 项上限与 truncated | paths 第 6 条对 49/50/51/60 项通过 |
| 隐藏目录与大小写前缀 | paths 第 7 条通过 |
| HOME 自身为符号链接 | paths 第 8 条通过 |
| 不存在目录按最深祖先与内部链接解析 | paths 第 9 条通过 |
| confirm 再守卫、禁止 HOME 本身、空工作区 | project-confirm 第 2–5 条通过，未调用 add |
| confirm 必先 probe，相同绝对路径，单仓库不传 docs_repo | project-confirm 第 1 条通过 |
| 单仓库与三个成员工作区真实注册 | project-confirm-real 第 1、2 条通过 |
| 重复注册内核拒绝原文 | project-confirm-real 第 3 条通过 |
| skip 成功 / skip 失败保留项目 | project-confirm 第 6 条通过；真实 skip 的回包由第 5 条键集合比对 |
| read probe 守卫及不注入守卫的兼容模式 | project-confirm 第 8 条通过 |
| remove 严格 id 白名单、合法转发 | write-gate 新增用例及 project-confirm-real 第 3 条通过 |
| HTTP 合法请求与坏 Host / Origin | 3 条 w9-http 已入测试体系；监听 EPERM，未取到 HTTP 行为通过证据 |

### 2. 失败分支

- 路径授权拒绝：HOME 外绝对路径、逃逸链接、整个 HOME，替身零调用；真实拒绝后 project.list 不变，外部目录无 .autome，均通过。
- 读盘失败：文件作为补全目录返回 error；断链、缺失 HOME、无效 HOME 拒绝，均通过。不存在目录正常空列表，区别于错误。
- 空输入：check 拒绝、complete 解释为 ~/；HTTP 非字符串/多余字段测试已写但监听受阻。
- 内核拒绝、超时、不可达：project-confirm 第 7 条分别验证 409 原文、504、503，只调用 probe 一次，未继续 add，均通过。
- skip 抛错：返回 200、project 与 skip_error 原文，不回滚，通过。
- Host/Origin 拒绝：HTTP 测试未取证，原因同上；没有用静态接线替代运行证据。
- 其它外部服务：不适用，本里程碑无网络外部服务及终端启动。

### 3. 全部迁移

- 未注册 → probe → add → onboarding step 3：真实单仓库/工作区通过。
- 未注册 → 守卫拒绝 / probe 拒绝：未 add，通过；add 重复拒绝原文 409，通过。
- add 成功 → skip 成功：onboarding skipped、init_commit；add 成功 → skip 失败：保留项目、带 skip_error，均通过。
- 已注册 → 重复 confirm 拒绝 → remove → 不在 list → 再 confirm：真实内核第 3 条覆盖通过，移除后 .git 仍存在。
- 读通道 probe 配置守卫/未配置守卫、写通道 confirm/remove：已分别覆盖；HTTP 通道及上下文经真实请求的证据仍受监听限制。
- UI 状态迁移不适用，本里程碑没有页面。

### 4. 输入域边界

- 路径空串、非字符串、511/512/513 字符、~/、整个 HOME、相对路径、NUL、..、反斜杠、符号链接、未存在祖先均实跑通过。
- 补全空列表、单元素、49/50/51/60 项，隐藏前缀及区分大小写均通过。
- confirm 必填 path、额外键、docs_repo 非名称、布尔字段 null/数字；remove 缺失/空/id 路径/额外键均拒绝，合法形状通过。
- 日期、闰日、跨年不适用，此接口无日期字段。

## 负向对照

临时变异程序 `.autome/output/m01-negative.mjs` 每次以 finally 还原源码；执行退出 0（四项都捕获缺陷），最后重新运行真实源码验收。

| 故障注入 | 命令子集 | 判别结果 |
|---|---|---|
| 祖先 realpath 改为字面路径 | paths + project-confirm | 17 条中 14 失败，exit 1 |
| confirm 移除 probe 调用 | project-confirm | 8 条中 3 失败，exit 1 |
| 移除 LocalOpError 映射 | project-confirm | 8 条中 3 失败，exit 1 |
| 移除 project.remove 白名单 | write-gate | 46 条中 1 失败，exit 1 |

负向对照不是新增用例，以上重复执行量不累计到 120。HTTP remove 对照未执行，因为基线就无法监听。

## 后续与提交

M-01 保持开放，下一轮在允许本机监听的环境原样重跑完整验收；120 条全过并复核清单后才标待审。未启动下一会话，未推进 M-02，未改协议文件或 .autome 配置（仅写允许的 output 产物）。
