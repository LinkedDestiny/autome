# M-05 · 实现 #7

结论：项目报警实现与不监听端口的验收通过；完整验收被沙箱监听权限阻断。M-05 保持「开放」、reopen 0，implementation-round 为 7/30，不标待审或已完成。下一轮首先补验同一条 M-05 验收命令中的 HTTP 7 例。未推进 M-06。

## 实现与修复说明

- `alerts-data.ts` 从 dashboard 按项目过滤、按时间倒序排序和计数；不修改原始数组。
- 项目页组合读取看板和 dashboard；报警卡显示种类、详情原文（转义 HTML、保留换行）、时间、可选任务链接与「知道了」。消除走既有写入口，失败原文可见，离线禁用；不保存业务状态副本。
- loadShell 增加 dashboard 读取，非零报警数进入侧栏标签及 aria-label；dashboard 失败只影响标记，不把外壳置为离线。environment 同次读取透传，未实现 M-06 页面。
- 根和项目路由既有 `byKind` 匹配即可处理 project.alert 与 project.alert_dismissed，不改事件基础设施。未知 kind（含 constructor / __proto__）按原文展示，避免继承属性被当成名称。
- 旧测试必要更新：shell-data、load-context 的调用次数增加一次 dashboard；board 的 SSR hydration fixture 补新增 alerts 字段。均保留原用例与断言含义，不增减旧用例。
- 真实预算报警由临时 HOME / AUTOME_HOME、真实 automed 和假 CLI 生成；fixture 的 alert 字段来自本轮真实回包。`test/fixtures/w9-replies.ts` 的 captureReplies 已增加报警抓取流程。HTTP setup 也使用假 CLI，避免测试推进真实模型会话。
- 上一轮独立审计 M-04 通过，无审计缺陷检查需要搬入。本轮首跑发现旧 SSR fixture 缺字段、边界测试误假设现有网关拒绝空 ID，以及首次抓取时模块已加载旧 fixture；均已修正。真实内核对空 ID、未知 ID、512 字符 ID 返回 409 且不改变现有报警；没有为测试擅自收紧既有白名单。

## 命令与结果

工作目录为仓库根；验收命令在 apps/web 执行。使用已有 target/debug/automed 与 autome，没有运行全量门禁、宿主快检或全量 Web 测试。

1. 完整验收命令：

   `cd apps/web && npx vitest run test/alerts.test.ts test/alerts-real.test.ts test/w9-http.test.ts test/w9-layout.test.ts test/shell-data.test.ts test/load-context.test.ts test/board-live.test.ts test/board.test.ts test/shell.test.ts test/labels.test.ts test/styles.test.ts`

   最终完整执行结果：11 文件、73 例，66 通过、7 失败、0 跳过。失败全在 w9-http，均为 `listen EPERM: operation not permitted 127.0.0.1`，请求尚未到达服务；并有 Vite watcher EMFILE 提示。输出 `.autome/output/m05-final.txt`。不把环境失败记为通过。首跑和修正中间结果另存 `.autome/output/m05-first.txt`、`m05-acceptance.txt`。
2. 不监听端口的同一验收子集（上面命令只去掉 test/w9-http.test.ts）：10 文件、66 例全部通过，无失败、无跳过，输出 `.autome/output/m05-related-green.txt`。
3. 补充消除失败的页面反馈断言后，重跑 `cd apps/web && npx vitest run test/alerts.test.ts test/alerts-real.test.ts`：2 文件、8 例全部通过，无失败、无跳过，输出 `.autome/output/m05-alerts-final.txt`。没有新增测试条目。
4. 设计三项负向对照：分别临时将项目路由改为按 subject 匹配、去掉按 project_id 过滤、将 dashboard 失败映射为外壳离线，各运行 alerts.test.ts 的第 6 / 1 / 5 条。三次均 exit 1 且目标断言失败，全部在 finally 还原；每次筛选 1 例失败、5 例未选中，不计入验收跳过数。输出 `.autome/output/m05-negative-{subject,filter,offline}.txt`，执行脚本 `.autome/output/m05-negative.mjs`。还原后完整验收的上述单元用例均通过。
5. `git diff --check` 通过。真实浏览器视觉检查未取证：本地 HTTP 监听被沙箱拒绝；本轮提供 SSR 和静态布局验证，不代替设计 H-01 的浏览器人工验收。

### 用例总数与基线差

- 本轮验收去重总数 **73**，与设计 M-05 契约 73 一致；不是 83（工作中曾口算错，已纠正）。相同 11 文件的本轮新增前基线是 63，新增 **10**：alerts 6、alerts-real 2、w9-http 1、w9-layout 1。没有删除、跳过旧用例。
- 上一轮 M-04 审计实际基线 **37**，本轮多 **36**：移出 settings-config 7 与 settings-real 4，共 -11；纳入 alerts 6、alerts-real 2、w9-layout 3、shell-data 6、load-context 10、board-live 7、board 8、shell 4，共 +46；共用 HTTP 从 6 增至 7，再 +1。即 37 - 11 + 46 + 1 = 73。
- 上一实现轮 #6 只跑两文件 11 例；与之差 +62 来自验收范围不同，不是声称本轮新增 62 例。重跑与负向对照不重复计入去重总数。

## 标待审之前的自审清单（本轮已执行；HTTP 未通过，因此不标待审）

1. **情形表逐行**（设计 §6、M-05 内容）
   - 本项目过滤、新的在前、多项目计数：alerts #1 通过；跨项目负向对照能失败。
   - 四种中文名、未知种类原文、detail 换行和 HTML 转义、时间、task_id 有/无：alerts #2 通过；未知继承属性名也有覆盖。
   - dismiss 精确参数、在线可用与离线禁用、操作错误展示：alerts #3 通过。
   - 侧栏有报警标签和 aria、零报警不带 alerts 键、环境快照透传：alerts #4 通过。
   - dashboard 失败保持看板、报警错误原文、外壳保持在线：alerts #5 通过；错误映射负向对照能失败。
   - 两种事件分别用项目 ID 和报警 ID 作 subject：alerts #6 通过；按 subject 匹配的负向对照能失败。
   - 真实报警出现与消除：alerts-real #1 通过，持久 memory router 的根及项目 loader 仅由账本事件触发重载，SSR 出现/移除报警；真实回包键集合和字段类型与 fixture 一致：#2 通过。
   - 项目 action 与 single fetch 的坏 Host/Origin、报警不变和合法请求消除：已加入 w9-http，因监听 EPERM 未取证；没有删掉或跳过。
   - 报警区域限高、内部滚动和详情保留换行：w9-layout 通过；真实窗口尺寸检查归 H-01，未声称实跑。
2. **失败分支**
   - dashboard 的读盘失败、超时、内核离线、读取被拒均经 loader/SSR 检查，错误原文保留、外壳在线（alerts #5）。
   - dismiss 授权拒绝、超时、内核未连接经 action 与页面反馈检查，报警仍显示（alerts #3）；无网关返回既有离线错误。
   - 非法字符串 NUL、513 字符由真实网关拒绝且内核零调用；空和未知 ID 的真实内核拒绝不改变报警（alerts-real #1）。
   - Host/Origin 拒绝检查已写但环境阻断，未取证。独立外部服务不可达不适用：本里程碑没有外部服务调用，内核离线已覆盖。
3. **全部迁移**
   - 无报警 → 真实预算报警 → 存在报警：事件重载根及项目，显示卡片与侧栏计数，通过。
   - 存在报警 → 消除 → 无报警：报警 ID subject 事件重载两处，通过。
   - 存在报警 → 消除失败 → 仍存在：拒绝原文与卡片保留；真实未知 ID 消除后 dashboard 不变，通过。
   - dashboard 成功 → 读取失败 → 恢复：失败隔离与恢复重读通过；连接允许/禁止时按钮启用/禁用通过。
   - 读通道只取 dashboard / project 数据，写通道只发既有 dismiss；HTTP 安全头跨两种路由的矩阵尚待取证。
4. **输入域边界**
   - 空报警列表、单元素、多项目、新旧跨年时间排序、可选 task_id null、未知 kind、detail 特殊字符通过。
   - alert_id 空串、单字符、512 字符按既有网关语义转发；513 字符、NUL 拒绝；真实空/不存在/最大长度 ID 不误消除已有报警。
   - 前导零、闰日不适用：ID 为不透明字符串，界面不解析数字 ID；日期只展示内核时间戳，没有用户日期输入或日历计算。

## 后续与提交

M-05 的实现和测试留在当前分支；下一节点应在允许 loopback 监听的环境复跑完整验收，取得 73 例全过后再标待审。未改 crates、desktop、协议、Autome 配置，未推送、未启动下一个会话。提交结果见会话最终回复；若 git 元数据写入被拒，由 Autome 兜底。
