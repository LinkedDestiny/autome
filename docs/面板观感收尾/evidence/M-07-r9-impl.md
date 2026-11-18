# M-07 · 实现 #9 · 验收通过，待独立审计

## 范围与实现

仅推进 M-07（D-1、D-2、D-Q1/D-Q2）。时间线改为 40px 最小高度网格：时间、类别、可展开完整文字、耗时/花费、结果、会话日志。折叠文字单行省略，展开恢复换行；所有事件都有文字入口，失败详情独立且原有标记不变。保留 key、SessionLog、运行中默认打开、分页链接参数与滚动保存逻辑。更早会话入口使用 btn btn-sm。

TimelineEntry 仅增加可选 cost/outcome；耗时复用 durationMs。会话、门禁、用户退回投影结果胶囊。原型两行约 50px 与任务上限冲突，遵循设计采用折叠一行。已同步真实内核测试的角色胶囊选择器，布局用例补折叠行高、前十条完整可见、普通事件文字展开检查。未改变其它里程碑。

上轮审计为 M-06 通过，无退回缺陷或需要逐字搬入的审计检查。

## 命令、用例总数与基线差

- 同范围改前命令：`(cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts)`。78/78 通过，原始输出 `.autome/output/m07-baseline.txt`。
- 改后同命令（另加 `--silent=false --reporter=verbose` 以记录 SSR 字节）：84/84 通过、0 跳过，6 文件；相对同范围起点 **+6**，全部来自 task-timeline（29→35），分别覆盖结构、文字可达、日志展开、CSS/分页、长事件、多种结果状态。其它文件分别 6、3、9、16、15，无用例删除。输出 `.autome/output/m07-final-accept.txt`。
- 上轮 M-06 审计总数为 95，本轮 84 的跨轮表面差为 -11，原因是里程碑命令换了测试文件；不代表删除 11 个用例。应以本轮同范围 78→84 对照。
- `(cd apps/web && npx vitest related --run --project unit app/components/task-timeline.tsx app/components/task-timeline-model.ts app/styles/app.css)`：21 文件、244/244 通过。该集合仅 task-timeline 增加 6 个；该集合起点为 238 个（由当前集合减新增数核对，未另跑一遍），其它文件用例数不变。输出 `.autome/output/m07-related.txt`。kernel/exclusive 由指定快检覆盖，不重复扩大手工验证。
- SSR 实测：60 会话首屏 81,267 字节；同 URL 数据 25,689 字节；钉住 session-40 后 65 会话首屏 92,939 字节，均 HTTP 200 且 ≤153,600 字节。测试会构建生产包，无监听端口。
- `git diff --check` 通过。未运行 `sh scripts/ci-local.sh`。

## 负向对照与修正

在当前实现上逐项临时注入，分别运行 `cd apps/web && npx vitest run test/task-timeline.test.ts`，使用已有构建标记避免仅组件负向试验重复构建；每项 finally 还原。每次总数均 35，无跳过：

1. 将 summary 中摘要 span 恢复为 p：2 失败 / 33 通过，文字结构与 R1-P01 失败。
2. 删除 min-block-size 并允许折叠文字换行：1 失败 / 34 通过，紧凑行守卫失败。
3. 仅 session 条目输出文字格：5 失败 / 30 通过，R1-P01、类别结构、门禁与日志路径检查失败。

原始输出 `.autome/output/m07-negative-{summary,height,system}.txt`。还原后完整验收 84 通过。实现期间修正了旧测试把全部 details 当作日志数量的断言（现只数 session-log，仍要求历史 2 条日志且仅运行中 open），以及新增 cost 字段遗漏美元符号的问题；不放松业务断言。

旧结构检索：`grep -rnE 'timeline-meta|task-timeline h3|<h3><span' apps/web/test apps/web/e2e apps/web/scripts`，改前 1 处（真实内核测试 h3 前缀），改后 1 处（新增禁止 timeline-meta 的反向断言），无旧期待残留。输出 `.autome/output/m07-old-before.txt` 与 `m07-old-final.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：D-Q1 会话/门禁/你的动作/系统四类别均渲染时间与类别；六项新增用例逐项通过。所有条目完整 title/summary 位于 details > summary，details 数组在展开体，无 tooltip 复制。长 processes_stopped 多命令及末尾 EPERM、protocol_failure、多里程碑变化分别验证。未核实红字及原始失败标记由既有测试和新结构用例通过。D-Q2 会话日志仍为直接子元素，open 跨列；普通文字格独立展开，分页 btn 保留行为。真实行高/一屏十条已有 e2e 断言，本轮不声称取得浏览器通过证据。
2. **失败分支**：timeline-failure-path 六用例覆盖门禁连续失败、门禁超时、准备失败、空/缺失路径、中文路径、原始字段保留；task-timeline 覆盖读取失败提示、截断提示、会话超时/崩溃/进程消失及空记录。授权拒绝、外部服务不可达不适用：本里程碑不新增授权或网络请求，沿用现有读取错误入口。
3. **全部迁移**：运行中→正常结束/停止/超时/崩溃（结束事件优先于运行标志）、门禁排队→运行→通过/失败/超时、用户/里程碑退回，由模型用例逐项检查；历史日志关闭/运行日志打开均检查，筛选全部/门禁/退回保留 URL 的既有测试通过。文字折叠→展开→折叠采用原生 details（CSS 与完整 DOM 已断言，浏览器点击往返交宿主）；没有新增受控状态机。分页 0→1 入口边界已检查，滚动保存代码不变，实时滚动人工项 H-10 保留。
4. **输入域边界**：空 entries、无 details、无 failures、单条事件、earlier=0/1、费用缺失/0/1.25、长中文标题/摘要、空/单文件名/中文空格日志路径均有通过断言。前导零、闰日、跨年、最大值不适用新增输入域：本轮未增加日期或数字解析，时间复用既有时区格式化、费用沿用原有 toFixed(2)，没有新声明的数值范围。

## 环境与交接

指定快检结果见下方最终记录。真实浏览器视觉与滚动验收仍由宿主及 H-05/H-10 承担；不把静态结构通过表述为真实一屏可见已验收。不接触真实账本、用户面板或已安装程序。

### 指定快检最终记录

`sh scripts/ci-quick.sh --since 522764e690180fa4d6c992f8e89d243ce3de5819` 最终退出 1：41 步，5 步失败，147 秒。Format、Clippy、Web lint、Web typecheck、Web build 均通过；本轮 task-timeline 35 个通过。输出 `.autome/output/m07-final-quick.txt`。较首次快检的 6 个失败步骤减少 1 个（cost 美元符号修复），剩余均为环境阻塞，不声明快检通过。

四个 kernel 文件均在 `ps` 清理检查/afterAll 被 `spawnSync ps EPERM` 拒绝：
- `task-timeline-real.test.ts`：「正常出口无存活进程」（两个实际页面业务用例通过）。
- `task-detail-real.test.ts`：「正常出口与异常 teardown 均无残留；杀不掉保留目录，清理可重复」。
- `task-docs-real.test.ts`：「收尾无存活进程」。
- `task-merge-real.test.ts`：「收尾无存活进程」。

宿主复验：`(cd apps/web && npx vitest run test/task-timeline-real.test.ts test/task-detail-real.test.ts test/task-docs-real.test.ts test/task-merge-real.test.ts)`。

Web e2e globalSetup 的 ps 检查失败，浏览器整套未执行；本轮新增断言所属用例「1512×944 需求详情展开日志」没有浏览器通过证据。宿主针对性复验：`(cd apps/web && npx playwright test e2e/layout.spec.ts -g '1512×944 需求详情展开日志')`；完整选中用例清单由快检原始输出保留，不把未执行计为通过。

快检 120 秒准入预算另留下三个 exclusive 文件未执行：`shell-live.test.ts`、`task-live-real.test.ts`、`task-log-live.test.ts`；由宿主合并前门禁覆盖，或运行 `(cd apps/web && npx vitest run test/shell-live.test.ts test/task-live-real.test.ts test/task-log-live.test.ts)`。

状态格式检查通过：implementation-round=9/65、current-milestone=M-07、M-07 待审、M-08 开放、证据指针唯一。未将任何里程碑标成已完成。

两次指定快检的 Vitest 汇总均为 29 次文件运行、281 个用例，总数差 0；第二次仅修复前次 2 条 cost 断言，未删改用例数。该快检集合与里程碑验收/related 集合重叠，不相加当作去重总数。
