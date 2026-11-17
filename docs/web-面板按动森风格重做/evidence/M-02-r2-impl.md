# M-02 · 实现轮 2

结论：外壳的 9 文件 114 条单测通过，提交待审；浏览器与快检中的进程扫描被沙箱拒绝，未宣称快检全过。implementation-round: 2/65。

## 范围与裁定

本轮只推进 M-02：导航按品牌、总览、Loop 配置、技能、项目、添加项目、驱动、环境排序；顶栏面包屑、搜索、铃铛、新建入口；项目空闲行与稳定色块；两个占位路由、命令面板入口与 config/project 实时登记。

根 loader 的驱动数据复用 WebServices.drivers → server/drivers.ts（与环境页相同），不直接绕过旧内核回退。skills.list 只读全局数量，失败隐藏数字。项目 done 数只计 state=done，不把 cancelled 当已完成。每次 tick 后读驱动，按可见摘要去重广播 drivers，连接时补发最新状态；卡片订阅，使用共享时钟展示时间。

用户裁定：BL-01 纳入 M-12（只读解析头部 description 并显示）；BL-06 纳入 M-13（批准设计的评审裁决入口或缺失回退），均已写入里程碑表及验收契约，按一轮一里程碑规则后续实现。BL-02、03、04、05 忽略，不实现。预算使用宿主给定 65，不按新增里程碑推算。

上轮审计 M-01 通过，无退回缺陷，无需逐字移植审计检查。

## 命令、计数与结果

- 里程碑命令：`(cd apps/web && npx vitest run test/shell.test.ts test/shell-data.test.ts test/eventbus.test.ts test/live-client.test.ts test/palette.test.ts test/live-routes.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts)`。9 文件 **114 用例通过**，0 失败、0 跳过。日志 `.autome/output/m02-unit.txt`。
- 同子集起点源码基线为 **100**，净增 **14**：shell 4→8，shell-data 8→11，eventbus 23→25，live-client 8→9，palette 7→8，live-routes 14→15，styles 15 不变，labels 10 不变，testids 11→13（页面循环新增两路由）。源码声明计数核对见 `.autome/output/m02-case-counts.txt`，testids 循环展开单独计入。不是全项目基线。
- 与上一实现轮报的 68 相差 **+46**：上一轮与本轮验收子集不同；移出 ui-kit 20、bell-view 8、test-groups 4（共32），纳入 shell/data/eventbus/live-client/palette/live-routes 共76，加已有 testids 新增2，即 -32+76+2=46。原断言未删除；palette 精确候选列表补入新入口，overview 的空闲项目断言由 count-zero 更新为本轮设计规定的“没有任务在跑”。样式、文案与选择器守卫没有放宽。
- 直接相关回归：`(cd apps/web && npx vitest run test/overview.test.ts test/drivers.test.ts)` 实际匹配 overview 一个文件，7 用例通过；drivers.test.ts 不存在，不把它算成执行过。日志 `.autome/output/m02-related.txt`。
- 浏览器验收：`(cd apps/web && npx playwright test e2e/layout.spec.ts -g "总览")`，启动夹具即报 `spawnSync ps EPERM`，**3 条用例未执行**，不是通过或正常跳过。日志 `.autome/output/m02-browser.txt`。待审计运行：1512×944 总览、总览命令面板、总览铃铛。已在这三条加侧栏四入口及驱动卡可见断言，并在总览记录截图；本环境未取得截图，视觉验收未实跑。
- `npm run typecheck --prefix apps/web` 通过，日志 `.autome/output/m02-typecheck.txt`；lint 日志 `.autome/output/m02-lint-final.txt`。
- 快检原样执行 `sh scripts/ci-quick.sh --since f33d4c4959e56615e8c6795e265bb7378232734d`。第一次发现 overview 旧断言，修正后该7用例已绿；另有 task-timeline-real 与 Web e2e 的 ps EPERM。最终复跑日志 `.autome/output/m02-quick-final.txt`，最终统计见下方补记。
- 未运行全量 `scripts/ci-local.sh`。

## 标待审之前的自审清单

1. **情形表逐行**：A-2 导航顺序及面包屑由 shell 用例核对；有任务双计数、空闲70个完成、零任务行、未知读取破折号由 shell/overview 检查。A-Q1 数量0/1、skills返回失败或抛错隐藏数字、done排除cancelled均通过。A-Q2 未知（含旧内核推断）、无登记、闲置、推进四种口径均通过；两个驱动选推进者，多推进选最新tick，self只在推进时选中。总-Q5 新入口及编码、两个占位页渲染通过；总-Q6 config/project触发、task不触发已测。
2. **失败分支**：shell-data 原有四种失败（拒绝读取、读盘失败、超时、内核离线）仍逐项执行，保持局部错误并验证恢复。新增 skills.list 504和抛错均不破坏外壳；driver unknown_method保留推断。拒绝写授权由既有 shell/palette SSR禁用测试继续覆盖。占位页没有新写操作，不适用新的写入失败流程。
3. **全部迁移**：eventbus 用未知/无登记/闲置/推进四状态的16个有向组合核对是否广播；相同摘要不发、tick时间变化发、关闭后不发、重连补发最新摘要均已检验。live-client订阅收到drivers更新，解除订阅后不再收到；两个新路由对config/project事件刷新、对无关task事件不刷新。没有新增业务状态机。
4. **输入域边界**：skills数量0/1、空项目、done/cancelled区分、未知计数、项目ID含斜线/空格/问号/中文的链接编码均检查；命令面板既有0/1/50/51边界继续执行。新页面无日期或数值输入域，闰日、前导零、最大数值不适用。本轮时间为只读展示，基于同一时钟测试3秒与1小时。

## 负向对照与修复

设计规定的三项均实际注入并还原：

- 把推进者筛选换成 self：shell“两个驱动…”失败，日志 `.autome/output/m02-negative-0.txt`。
- 删除摘要相等短路：eventbus“不变时不发…”失败，日志 `.autome/output/m02-negative-1.txt`。
- 删除 /loop 的 config. 登记：live-routes“Loop 和技能…”失败，日志 `.autome/output/m02-negative-2.txt`。

三项退出码均为1，随后原代码还原、完整114用例复跑绿。不是上一审计退回的回归红绿，而是本契约自审负向对照。初次开发中的测试拼写、令牌名及旧fixture缺少clock均已修复，没有降低守卫。

## 最终快检与交接

最终快检退出1，59步骤执行、0沿用、4步骤失败；Format、Clippy、Web lint、Web typecheck、Build core、Web build均通过。所跑Vitest共372个（368通过、4失败）；失败只在 task-timeline-real（ps EPERM，含suite setup）、web-status-driver-real（ps EPERM）、board-live-listen（两个listen EPERM），另Web e2e在setup/cleanup被ps EPERM阻断。相关测试准入预算120秒，最后步骤从119秒开始，14个后续步骤未跑，逐项清单保留在最终快检日志。372为快检动态选择子集，不与上一轮645直接比较覆盖增减，更不是全量回归通过。

最终114条验收在全部负向修改还原及样式修正后再次通过；lint退出0、git diff --check通过。状态块字段和13行里程碑表结构检查通过。环境缺口交审计/宿主复验，不调高预算、不绕过进程扫描、不降低断言。M-02标待审，未标已完成。
