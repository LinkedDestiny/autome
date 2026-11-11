# M-05 实现轮 #6

## 结果与范围

M-05 提交待审，实现轮为 6/35。上一轮 M-04 审计通过，没有退回缺陷或须逐字移植的红绿回归检查。本轮只推进 M-05。

新增 bell-system.ts：SystemNotifier 可替换接口、浏览器适配器、级别/权限/后台判定，以及列表缺失时的消息降级。bell.tsx 接入原控制器的 onNotification；权限只由按钮请求，拒绝显示中文提示。同通知 id 用作 tag，不增加跨页协调或依赖。设计 §7 首条“无 props”与后文可注入接口矛盾，已就地改为接受可选 systemNotifier。

新增 bell-system 8 例；原 bell-view 增加 1 例权限渲染，既有 7 例不改。首次定向 15/16，原因是测试辅助函数把适配器当 spy；首次 typecheck 报测试类参数属性不可剥离。已修正夹具，最终定向与类型检查通过，无产品缺陷退回。

## 验收命令与结果

原始输出均位于 `.autome/output/`，表内为摘要。

| 命令 | 结果 | 输出 |
|---|---|---|
| `npm test --prefix apps/web -- test/bell-system.test.ts test/bell-view.test.ts` | 最终退出 0，16/16 | m05-target-final.txt |
| `npm test --prefix apps/web` | Vitest 失败：33 文件，300 例，270 通过、11 失败、19 初始化失败后跳过 | m05-web.txt |
| `npm run lint --prefix apps/web` | 通过，无警告 | m05-lint.txt |
| `npm run typecheck --prefix apps/web` | 最终通过 | m05-types.txt |
| `npm test --prefix apps/desktop` | 104 例，103 通过、1 基线跳过 | m05-desktop.txt |
| `npm run build --prefix apps/web` | 通过 | m05-build.txt |
| `npm test --prefix apps/web -- --exclude test/eventbus-live.test.ts --exclude test/server.test.ts --exclude test/notifier-live.test.ts --exclude test/gateway-http.test.ts` | 退出 0，29 文件、270/270，无跳过；仅补充验证 | m05-non-listen.txt |

Web 用例总数 **291 → 300（+9）**，文件 **32 → 33（+1）**；增量为 system 8 例与 view 1 例，没有删除旧用例。Desktop **104 → 104（+0）**，自带 1 个跳过不变。

全量 30 例环境阻断与上个实现轮相同：gateway-http 11 例 listen EPERM；eventbus-live 7、server 9、notifier-live 3 初始化不能监听（server 同时 watcher EMFILE）。没有新增 skip 或改断言。完整命令的 shell 最后执行 tail，所以 shell 返回 0 不代表 Vitest 通过，以上按 Vitest 失败摘要报告。审计须在允许监听环境复跑全部 300 例。本轮不宣称全量通过。

## 标待审前四项自审

### 1. 情形表逐行

| 设计 §7 情形 | 检查及结果 |
|---|---|
| 需要你 + 后台 + granted | system #1 控制器刷新成功后仅 show 一次，消息 id/title、列表 body、解析 href 精确匹配 |
| done 知会 / 未知级别 | #2 不弹 |
| 前台 | #3 不弹，后台再弹，切回前台不弹 |
| default / denied / unsupported | #4 不弹、不自动请求；view #8 分别显示按钮/拒绝文案/无提示 |
| 两后台标签页同通知 / 不同通知 | #5 两控制器各显示一次，同 id 同 tag，不同 id 不同 tag |
| 找不到条目 / GET 失败 | #6 空列表或 null 均保留消息标题且无正文；原 bell-client 的 GET 拒绝后仍交 null 检查在全量中通过 |
| 点击通知 | #7 每次 focus；可用任务目标跳转，null 只 focus |
| 浏览器适配器与替换 | #8 构造参数精确等于 title、body/tag；组件接线源码守卫，使用注入实现，无全局构造或权限请求；无锁/存储协调 |

### 2. 设计点名的失败分支

- 拒绝授权：#4 request 返回 denied，view #8 显示“系统通知已被浏览器拒绝，铃铛照常提醒”，无自动重试授权。
- 不支持 API：#4 permission/request 返回 unsupported，不调用浏览器 API。
- 刷新失败/列表缺项/空正文：#6 null、[]、空串、null 正文均降级，不吞提醒；bell-client 原有网络失败回调检查通过。
- 目标页面未开放/无任务：#6 href=null，#7 点击只 focus。
- 读盘失败不适用：本模块无磁盘操作。独立超时/外部服务不可达沿用铃铛 GET 失败的 null 分支，无新增网络服务或超时机制。

### 3. 全部迁移

- #4 穷举 4 权限 × 2 可见状态 × 7 通知 kind，共 56 个判定组合；只有后台+granted+6 个需要你 kind 弹出。
- #3 前台→后台→前台逐次处理；#4 浏览器权限依次 default→denied→granted，每次显式 request 返回对应状态，unsupported 返回自身；view #8 验证四种输出。
- #1/#5 消息→列表刷新→show；#6 列表不可用→降级 show；#7 点击→focus→可选跳转。
- 权限 UI 的真实手势/授权框与浏览器设置变更由 H-02/H-04 人工验收；源码守卫确认 request 只接在 onRequest，effect 不请求权限，未把 SSR 断言当作真实点击证据。

### 4. 输入域边界

- 列表 null、空、单元素；body 空串/null/非空；task_id=null；项目目标与任务目标、默认不可用目标：#1/#6 全过。
- 同 id 与不同 id：#5 验证 tag 区分；未知 kind：#2 不弹。
- 本模块没有日期、数值范围或用户输入解析，前导零、闰日、跨年、最大数值不适用。ID 不重新解析，沿用 bell-targets 已有 URL 编码与默认不可用路由守卫。

## 负向对照与恢复

运行 `.autome/output/m05-negative.mjs`，每项修改后运行两文件 16 例，并在 finally 恢复原文。五项均退出 1：去掉级别过滤（1 失败）、反转 hidden（5 失败）、固定 tag（2 失败）、effect 自动 request（1 失败）、无条目即 return（2 失败）。输出分别为 m05-negative-level/visibility/tag/auto-request/fallback.txt。全部恢复后定向最终 16/16；无错误注入遗留。

## 限制与交接

H-02/H-03/H-04 的真实授权框、系统横幅与 tag 合并未取证，仍由用户合并前确认。未运行 W12 端到端、未改 Electron。M-06/M-07 保持开放，待独立审计后继续。
