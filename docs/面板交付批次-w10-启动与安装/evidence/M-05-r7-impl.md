# M-05 · 实现 #7

结论：本里程碑验收通过，提交待审。实现轮为 7/65。上一轮审计 M-04 已通过，没有退回检查需要逐字迁入或复现。

## 实现

- 新增 clock.tsx，root loader 序列化 now / IANA timeZone；首次 SSR 与水合使用同一快照，effect 后切换浏览器时钟，每 30 秒刷新并在卸载时清理 timer。
- 时间线起止时间、重试与冷却、卡片及任务提示、铃铛、项目报警统一格式；卡片运行时长使用分钟精度。保留机器字段及 time.dateTime 原值，可见文字不显示原始 ISO。
- 八种 ConfigViolation 中文化，未知 kind 保留字段；角色 plan / impl 映射设计 / 实现。配置既覆盖未配置时的 gate / gate_quick，也覆盖有值时的四个嵌套行键。
- 守卫中文表按当前源码覆盖 12 种，而非只按设计概述的数量实现；保留 message / detail 和未知代码。
- m05-config-resolved.json 来自既有 w9Kernel 隔离夹具，临时 AUTOME_HOME、临时 Git 项目中设置 gate.command / quick 后调用真实 config.get 捕获，无真实账本访问。

## 命令、数量与结果

1. 里程碑原验收命令：`cd apps/web && npx vitest run test/clock.test.ts test/task-timeline.test.ts test/settings-config.test.ts test/labels.test.ts`。
   最终 **4 文件、41 用例全过，失败 0，跳过 0**。同范围上一轮基线为 25（10 + 7 + 8），新增 16：clock 8、timeline 2、settings 4、labels 2，最终为 8 + 12 + 11 + 10。首次编写后 38 过 / 3 失败，暴露角色 plan 未映射、未配置 gate 标签缺失、scheduler 提取被字符串中的 cfg(test) 截断；均修正，最终通过。原始输出 `.autome/output/m05-accept-final.txt`。
2. 直接相关回归：`cd apps/web && npx vitest run test/task-card.test.ts test/task-notice.test.ts test/bell-view.test.ts test/alerts.test.ts test/settings-real.test.ts`。
   **5 文件、38 用例全过，失败 0，跳过 0**；与修改前同文件用例总数差 0。修正旧测试的隐式进程时区，改传 Asia/Shanghai，保持 14:20 等具体断言；铃铛按新格式检查，设置真实内核用例按中文警告检查。输出 `.autome/output/m05-related-final.txt`。
3. 额外执行 `test/task-timeline-real.test.ts`（同 related 命令一起）：3 用例中需求/Bug 两条真实内核流程通过；“正常出口无存活进程”和 afterAll 的 ps 调用被沙箱以 EPERM 拒绝。没有跳过或削弱断言。core.stop 已执行，但无法完成全进程残留核验，需宿主复验。输出 `.autome/output/m05-related.txt`。
4. 本轮去重用例总数 **82**（41 + 38 + 3），对应旧范围 **66，增加 16**；最终 81 通过，1 环境阻塞，0 跳过，另有同根因 afterAll 错误。上一轮 M-04 的 28 条属于不同范围，不能拿 82−28 充当新增覆盖。
5. `git diff --check` 通过。遵循本轮边界，没有运行全量 CI 或宿主快检；没有进行截图或真实浏览器水合验收，后者仍由 D-5 覆盖。

## 标待审前自审清单

1. **情形表逐行**：clock 的两个时区（上海、纽约）各检查当天、同年跨天、跨年、跨零点；同日区间终点只含时分；秒、整分、分秒、整时、时分、天逐行通过。时间线、铃铛、报警各有 provider 下 SSR 检查。卡片/提示重试与冷却由直接回归检查。八种警告逐字段断言；未知 kind、未知守卫回退通过；真实配置行键及角色中文通过。
2. **失败分支**：settings-config 既有第 4 条分别检查拒绝授权、读盘失败、请求超时、内核未连接、TOML 错误；其它设置卡照常显示。空时间与无效时间原文回退、非有限/负用时归零、空事件/未知事件均检查。外部服务不可达不适用：新增格式化不调用外部服务。
3. **全部迁移**：timeline 既有用例覆盖排队→运行→成功/失败/超时、同提交重跑、中断、运行会话→结束/停止、追加事件稳定键及全部用户操作。新时钟首屏在 UTC / Tokyo 两个进程 TZ 下逐字一致；effect 切浏览器时区、30 秒刷新及 clearInterval 已逐行静态检查，真实浏览器效果本轮未实跑，不能用 SSR 结果替代 D-5 水合验收。格式化与标签本身无持久状态迁移。
4. **域边界**：0、59/60 秒、3599/3600 秒、86399/86400 秒、整时、跨零点、跨年、2024 闰日、空串、无效日期、NaN、Infinity、负用时、空列表、单个会话、单条通知/报警均检查。输入是内核 ISO 时间与数值用时，不接受自由输入的数字字符串，前导零数值输入不适用；ISO 内的两位月日由格式表覆盖。

## 负向对照

运行 `.autome/output/m05-negative.mjs` 临时修改产品源码，每次 finally 还原，四次子测试退出码均为 1：

- 用 getHours 读取进程本地时间：clock 格式及源码守卫失败。
- 移除 same_model 对应映射：八种中文与 Rust enum 交叉核对失败；项目测试也对 Rust 枚举文本注入 FutureWarning，确认不再相等。
- 移除 gate.command 标签：真实行键测试失败。
- 时间线恢复 started_at 原文：无 ISO 的回归检查失败。

原始输出 `.autome/output/m05-negative-{timezone,warning,config-key,iso}.txt`。还原后重跑原验收命令 41/41 通过。

## 限制与交接

本轮仅推进 M-05，M-01…M-04 的审计证据指针不变。进程残留检查受 ps 权限阻塞，真实浏览器水合与视觉检查未实跑；保留测试由宿主/后续对应里程碑验证。没有把任何里程碑标成已完成。
