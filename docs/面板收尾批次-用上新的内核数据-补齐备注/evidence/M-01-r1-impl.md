# M-01 实现 #1

## 范围与实现

仅推进 A-1～A-3。文档首标签合并任务卡，删除过时提示，默认分支标注主检出来源；改动读取内核类型、改名来源和删除数，多仓库按 `repos[].name` 分组，done 仍读取改动；删除全账本缓存，改为按 subject 每次从零分页；时间线优先使用 session.ended 的结束时间和方式，旧会话退回 lifecycle。

迁移所有旧事件服务调用者（含已有真实测试），不放开浏览器 events.since 权限，不修改内核、账本、协议或其他里程碑。测试只使用临时 AUTOME_HOME 与假 CLI。

设计的测试数量有一处现状偏差：task-detail-real 已有 4 个用例，不能当作不存在而覆盖。本轮保留全部 4 个，增强已有 Bug 用例，新增合并终态和三种结束方式 2 个，最终 6 个。因此契约实际为 59 个，不是原估 56 个。

## 标待审之前的自审清单

1. **情形表逐行**
   - 文档：任务卡并入首标签、Bug 文档和复盘教训标签、证据分组、取消空清单提示、非取消空清单无旧提示、worktree 不标注、default_branch 标注主检出；task-docs-view 对应 8 个测试覆盖。
   - 文档读取：默认卡片仍走 brief/request；指定文档仍用清单路径校验与原读盘错误。真实 Bug 正文、合并且 worktree 清理后设计正文/原标签/2 个里程碑终态均检查。
   - 改动：新增/删除/修改/改名、单仓库/多仓库、各仓库删除数、顶层删除阈值、done 的 merged 提示与进度；task-changes 9 个及 task-detail-data 4 个覆盖。仓库名按内核源码的 `name` 字段，不用假定的 `repo`。
   - 事件：每次带 subject 从零读取、短页/空页、恰好整页、整页噪声、水位高于本任务、空任务、过滤三类噪声、最新 5000、非空页不前进；task-history 6 个覆盖。
   - 时间线：exited/stopped/timed_out/crashed（退出码/消失）、无事件回退、冲突时事件优先；task-timeline 22 个覆盖。真实内核核对 exited/crashed(3)/stopped；真实 timeout 至少 1 分钟，按设计使用同形载荷单测。
2. **失败分支**
   - 拒绝授权/路径输入：原 task-changes 错误原文与 task-docs-view 的拒绝路径、清单缺失检查保留；本轮没有新增写授权路径。
   - 读盘失败：默认分支来源 + 文档不存在返回原错误，页面 alert 保留；相关 task-docs 测试覆盖实际读盘边界。
   - 内核不可达/超时：task-detail-data 的 503/504、config/history 降级；task-history 拒绝后下一次从零重读；task-changes 原样错误提示。
   - 空输入：无文档/无事件/无文件/无会话均检查；不前进异常包括 seq=0/-1 与噪声页。
3. **全部迁移**
   - 会话 running → exited/stopped/timed_out/crashed；running 标志与结束事件冲突仍显示已结束；旧 lifecycle killed/exited/vanished 与 timed_out 回退；有/无 started 事件均检查。
   - 改动读取：draft/cancelled/done、active 各合并节点、failed merge_gate、授权开/关组合；合并进度 await_merge/rebase/merge_gate/merging/main_check/cleanup/done 与失败状态均覆盖。
   - 文档 worktree → 合并清理 → default_branch：真实 CLI merge 两任务并 run --until-idle 后验证；取消无文档使用独立提示。
   - 事件分页：起点→整页→短页/空页、整页噪声→下一页、错误→新请求从零；无缓存生命周期需要关闭。
4. **域边界**
   - 删除阈值 (0,0)/(1,1)/(2,4)/(2,5)/(4,10)/(5,11)，6 个仅删行的 modified 且 deleted_count=0、二进制 deleted 零行数。
   - 分页 0/1/1000 条、5000/5001/6001 条保留上限、序号不前进/回退、全账本水位与任务游标无关。
   - 文档空清单、空正文/缺正文沿用既有语义、中文与 & 编码；会话退出码 0/3/4/null、无成本/零成本、闰日与跨年用时。
   - 不适用：前导零日期/金额最大值校验；本轮没有新增此类输入解析器。

## 验收命令与结果

执行本轮契约命令：

`cargo build -p automed -p autome-cli && (cd apps/web && npx vitest run test/task-docs-view.test.ts test/task-changes.test.ts test/task-timeline.test.ts test/task-history.test.ts test/task-detail-data.test.ts test/task-detail-real.test.ts test/test-groups.test.ts) && test ! -e apps/web/server/task-events.ts && ! grep -rq createTaskEvents apps/web/server apps/web/app apps/web/test`

- 构建通过。最终 7 文件 **59 个用例，58 通过、1 失败、0 跳过**；失败是既有“正常出口与异常 teardown 均无残留；杀不掉保留目录，清理可重复”的 ps EPERM，afterAll 同因报错。所有 5 个真实业务测试通过，非内核 6 文件 53 个全部通过。原始输出：`.autome/output/m01-acceptance-final.txt`。
- && 后的删除检查因前面失败未自动执行，另单独执行同样的 `test ! -e ... && ! grep -rq ...`，退出 0；旧文件和旧服务符号均已删除。
- 没有上一轮实现基线。改前 vitest list 实测 **46 个**（docs 5、changes 5、timeline 19、旧 events 6、data 3、real 4、groups 4），改后 **59 个，+13**：docs +3、changes +4、timeline +3、history 替换 events 净 0、data +1、real +2、groups 0。基线清单：`.autome/output/m01-baseline.txt`。没有删除现有真实验收来凑设计估算的 56 个。
- 按设计运行相关测试（仓库根执行）：`(cd apps/web && npx vitest related --run $(git diff --name-only --diff-filter=AM -- app server | sed 's#^apps/web/##') server/task-history.ts)`。最终选中 53 文件 **339 个**，274 通过、41 失败、24 因 beforeAll 失败未执行。与本轮初跑 339 个差 **0**，夹具修复使通过 +1、失败 -1；没有前轮相关测试实跑基线，不将其伪称为历史基线。原始输出：`.autome/output/m01-related-final.txt`。
- 相关命令最终没有 AssertionError/TypeError；失败来自 ps EPERM 或 listen EPERM。未增加 skip/环境提前返回。Vitest globalSetup 按项目原机制执行 Web 生产构建，该构建成功。
- 四项自审逐项由上述最终命令执行，能运行的检查通过；环境受阻项如下逐条交接。`git diff --check` 通过。

结论：依任务总-6、§6 约束 7、§9 第 1 条，M-01 以**业务与模型证据通过、环境项待独立复验**交审计，标待审，不宣称完整验收命令全绿。

## 修复与负向对照

没有上一轮审计，因此无待搬运的审计回归用例。

新增真实夹具初次把 `brief` 固定写成 `# test`，内核据此生成 slug=test，假 CLI 找不到 crash-end 的步骤，得到退出码 92。已改为每个用例自身的标题；原断言保持退出码 3，修复后真实结束方式用例通过。红：`.autome/output/m01-acceptance.txt`；绿：`.autome/output/m01-real-fixed.txt`（5 个业务测试通过，清理核对仍被 ps EPERM 拦截）。

设计指定的四个负向对照逐一注入并在 finally 还原，均退出 1：
- 删除数换回仅删行估算 → `deleted_count 不把六个只删行的修改文件误报为删除` 失败。
- events.since 去掉 subject → `每次调用带 subject，从零重读，不缓存` 失败。
- 分页终止换回 cursor >= latest_seq → `其它任务水位更大及没有事件的任务正常结束，不依赖 latest_seq` 失败；替身拒绝多余请求，避免错误算法无限循环。
- 忽略 session.ended → `事件给出四种结束方式与结束时刻，崩溃区分退出码和消失` 失败。

原始记录 `.autome/output/m01-negative-{deletion,subject,watermark,ended}.txt`；全部还原后 6 文件 53 个测试通过（`.autome/output/m01-unit-final.txt`）。

## 环境边界与交接

按任务总-6、§6 约束 7、§9 第 1 条，环境拦截不弱化断言、不加条件跳过，交审计/宿主复验。未运行 ci-local、ci-quick 或用户列出的宿主快检；未运行全量门禁。未改 Rust。

真实用例 teardown 已执行 task.stop/core.stop；`ps` 被沙箱拒绝，不能声称已核实无残留，相关临时目录按既有 teardown 规则保留。没有手工常驻面板服务；只读诊断 CLI 用的是本轮临时账本并已退出。Chromium 视觉探测在 macOS bootstrap_check_in Permission denied (1100) 处失败，未取得截图；关闭回收也记录 kill EPERM（`.autome/output/m01-visual-probe.txt`），交宿主核对浏览器残留。

### 宿主复验清单

完整命令使用上面的 M-01 契约命令与 related 命令，保持全部断言。以下 41 个测试被环境拒绝（4 个进程清理测试为 ps EPERM，其余为 listen EPERM）：

- test/task-detail-real.test.ts：正常出口与异常 teardown 均无残留；杀不掉保留目录，清理可重复
- test/task-docs-real.test.ts：收尾无存活进程
- test/task-merge-real.test.ts：收尾无存活进程
- test/task-timeline-real.test.ts：正常出口无存活进程
- test/board-live-listen.test.ts：1 external stop reaches websocket within 4500 ms
- test/board-live-listen.test.ts：2 new SSR request after stop places card in needs-you column
- test/gateway-http.test.ts：无凭据 config.get 到达真实内核
- test/gateway-http.test.ts：无凭据写后读回新值
- test/gateway-http.test.ts：缺失及错误 Origin 拒绝写且状态不变
- test/gateway-http.test.ts：错误 Host 的 GET 拒绝
- test/gateway-http.test.ts：所有响应无 Cookie/CORS 且 home 不生成 web 目录
- test/gateway-http.test.ts：不合格 upgrade 回 403
- test/gateway-http.test.ts：合格但无处理器 upgrade 回 404
- test/gateway-http.test.ts：/ws 已注册，缺少握手字段回 400
- test/gateway-http.test.ts：close 后内核 pid 不存在
- test/gateway-http.test.ts：二进制缺失仍监听，读回 503 中文原因
- test/gateway-http.test.ts：坏 JSON 回 400 中文原因
- test/gateway-task-card.test.ts：60KB Bug 正文开头经 HTTP 到达真实内核并创建草稿，超限中文拒绝
- test/gateway-task-card.test.ts：最大合法任务卡正文经 HTTP 创建后从真实内核逐字节读回
- test/gateway-task-card.test.ts：开始与补充越过 HTTP 门禁进入内核分支，缺参及类型错误在门禁拒绝
- test/gateway-task-card.test.ts：project.probe 经 HTTP 由真实内核展开家目录，越界路径被拒
- test/gateway-task-lifecycle.test.ts：task.start 经网关开始 Bug 草稿，会话只落到替身
- test/gateway-task-lifecycle.test.ts：task.reply 经网关把补充信息原样送达等待补充的 Bug
- test/older-kernel-real.test.ts：真实旧内核 hello outdated、读 200、写 409
- test/older-kernel-real.test.ts：真实 autome core 启动日志含路径与标识且参数传到子进程
- test/shell-live.test.ts：3 listening development SSR contains the real project and needs-you count
- test/uploads-http.test.ts：1 upload endpoint returns id/name/size and exact persisted bytes
- test/uploads-http.test.ts：2 wrong Origin and Host reject before any disk writes
- test/uploads-http.test.ts：3 declared oversize immediately returns Chinese 413 without reading full body
- test/uploads-http.test.ts：4 chunked oversize returns 413 and removes partial data
- test/uploads-http.test.ts：5 JSON upload rejected with 415 while write JSON parsing remains intact
- test/uploads-http.test.ts：6 real startServer wiring uploads through gateway creates a real draft
- test/w9-http.test.ts：目录补全合法头、Host/Origin 拒绝与输入形状
- test/w9-http.test.ts：confirm 坏头或外部路径均不注册
- test/w9-http.test.ts：remove 坏头保留项目，合法头能够取消注册且文件仍在
- test/w9-http.test.ts：添加页 action 与 single fetch 坏 Host/Origin 拒绝且无注册
- test/w9-http.test.ts：设置 action 与 single fetch 安全头和超长保存经网关拒绝
- test/w9-http.test.ts：设置页移除 action 和 single fetch 坏 Host/Origin 保留项目
- test/w9-http.test.ts：报警 action 与 single fetch 坏 Host/Origin 保留报警，合法请求消除
- test/w9-http.test.ts：环境安装与登录坏 Host/Origin 不启动终端，合法请求执行
- test/w9-http.test.ts：环境 action 与 single fetch 坏 Host/Origin 拒绝，合法检测可达

以下 24 个测试在端口监听失败的 beforeAll 后没有执行；不是主动跳过，也不能计作通过：

- test/logtail-live.test.ts：真实会话通过同一 /ws 在一个 tick 内收到增量
- test/logtail-live.test.ts：收割后先发完整尾段再结束，并通过 lsof 正向对照证明无句柄
- test/logtail-live.test.ts：拒绝客户端路径，关闭服务释放正在跟随的订阅
- test/notifier-live.test.ts：建面板库，真实失败事件向两个连接推送严格五键消息
- test/notifier-live.test.ts：带 Origin 已读跨连接广播，无 Origin 403 且库不变
- test/notifier-live.test.ts：停服期间新增失败，重启只补一条且保留旧已读
- test/server.test.ts：dev /healthz 返回 200 与 ok
- test/server.test.ts：dev 首页 SSR 与模块脚本
- test/server.test.ts：prod /healthz 返回 200 与 ok
- test/server.test.ts：prod 首页与引用的 JS 资源可访问
- test/server.test.ts：prod 未知路径返回 404
- test/server.test.ts：只监听 IPv4 回环地址
- test/server.test.ts：端口被占时非零退出并提示中文
- test/server.test.ts：开发 HMR 外站 Origin 回 403
- test/server.test.ts：开发首页错误 Host 回 403
- test/task-actions-http.test.ts：两条 action 路径全部意图坏 Origin 拒绝且不调用网关
- test/task-actions-http.test.ts：两条 action 路径全部意图坏 Host 拒绝且不调用网关
- test/task-actions-http.test.ts：合法请求通过实际路由 action 才写网关
- test/task-live-real.test.ts：CLI 暂停后一个 tick 内收到事件、重验并返回新状态
- test/task-live-real.test.ts：八个前缀使用真实内核事件，本任务重验、其它任务不重验
- test/task-live-real.test.ts：收尾无存活进程
- test/task-log-live.test.ts：页面历史会话折叠，运行中会话默认展开
- test/task-log-live.test.ts：时间线历史与运行会话均能接入 W8 日志订阅且收到增量
- test/task-log-live.test.ts：收尾无存活进程

视觉截图未实跑；宿主可在可启动 Chromium 的环境核对文档来源标注及工作区分组，无须修改断言或权限白名单。
