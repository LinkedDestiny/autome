# M-05 · 实现 #6

## 范围与实现

只推进 M-05（R-03、R-05 的新建入口）。新增 palette-model、command-palette 与 7 条项目 Vitest 用例；根组件接入平台快捷键和 loader 候选，顶栏提供搜索与预填「新建」入口。沿用现有 token，未增加依赖，未改内核、desktop、协议或其它里程碑。

候选从根 loader 派生，不保存项目或任务副本；状态中文查 labels，归列查 columnOf，链接查 nav-links。查询为空只显示等你处理、进行中的任务和项目、新建入口；非空可查到草稿、完成、取消任务。最多展示 50 项并报告剩余数。新建入口读取实时写门禁，Enter 和点击都拒绝禁用项。面板支持输入聚焦、关闭恢复焦点、Tab 在面板内循环、方向键选项滚入可见区、Esc 与遮罩关闭。

上一轮 M-04 审计 #5 通过，没有退回缺陷或需要逐字迁入项目测试的检查。本轮四条负向对照是 M-05 设计规定的新检查。

## 标待审之前的自审清单

1. **情形表逐行**：palette 测试 1 覆盖 Mac/iPhone/iPad 的 Meta 与 Win/Linux 的 Ctrl、错误修饰键、双修饰键、Shift/Alt/组合输入；测试 2 覆盖监听、preventDefault、卸载；测试 3 覆盖任务全部状态候选、排序、项目、新建和编码链接；测试 4 覆盖 ID/标题/项目名、不区分大小写、首尾空白、默认列表与终态检索；测试 6 覆盖写门禁禁用和理由；测试 7 覆盖 dialog/combobox/listbox/option 语义和活动后代。逐项通过。
2. **失败分支**：测试 4/7 检查空输入、无匹配、空候选；测试 5/6 检查全部候选禁用及 Enter 拒绝导航；测试 6 检查内核离线理由传播到候选和顶栏。不适用：本面板不请求授权、不读盘、不新增网络请求，因此没有面板自己的拒绝授权/读盘失败/超时分支；内核失败仍由既有 loader 与 writeGate 处理。浏览器启动遭 listen EPERM，未取得视觉/真实键盘证据，保留 H-01..H-07 人工验收。
3. **全部迁移**：测试 5 覆盖 closed→open、open→input、down/up 正向与反向循环并跳过禁用项、open→enter 导航并关闭、open→close、closed 时 down/up/enter 无效；测试 6 覆盖全禁用 open/down/up/enter、强制指向禁用项也不能导航；测试 2 覆盖 shortcut attach→toggle→detach。测试 7 覆盖 open/closed 渲染。焦点恢复、Tab、遮罩等真实 DOM 行为本轮未取证，属于 H-03。
4. **域边界**：测试 4 覆盖空串、纯空白、无匹配、单元素、50/51 项；测试 5 覆盖空列表、单元素、active=-1；测试 3 使用含斜杠、空格、问号、中文的项目 ID 验证编码。不适用：日期、闰日、跨年不属于查询域；任务 ID 按字符串匹配，没有数值最大值输入契约。

## 验收与基线

原始输出均在 `.autome/output/w5-r6-*.txt`。首次 typecheck 发现两处 TypeScript 推断/索引错误，已修正；下列为修正后的结果。

| 命令 | 退出码与结果 |
|---|---|
| `npm run lint --prefix apps/web` | 0 |
| `npm run typecheck --prefix apps/web` | 0 |
| `npm test --prefix apps/web -- test/palette.test.ts test/shell.test.ts` | 0；11/11，0 跳过；恢复负向对照后再次 11/11 |
| `npm test --prefix apps/web` | 1；总数 364，333 通过、12 失败、19 跳过，43 文件中 38 通过 |
| `npm run build --prefix apps/web` | 0 |
| `npm test --prefix apps/desktop` | 1；总数 104，103 通过、1 失败、0 跳过 |
| `git diff --check` | 0 |

Web 相比上一轮审计运行基线 357 增加 **7**，全部来自新增 palette.test.ts 的 7 个用例，没有删除旧测试；设计原来的 350 是静态枚举数，运行时展开总数为 **364**（静态 350 + 原有参数化差 14）。desktop 相比 104 增加 **0**。

Web 环境受阻清单：gateway-http.test.ts 全部 11 条（读、写、Origin、Host、Cookie/CORS、upgrade、关闭进程、缺二进制、坏 JSON）均因 `listen EPERM 127.0.0.1` 失败；shell-live.test.ts 第 3 条监听 SSR 同样失败，前两条真实内核/跨进程用例通过。eventbus-live、notifier-live、server 的 beforeAll 因监听失败/启动超时而跳过其 19 条用例（server 9 条全部未执行）。日志另有 watcher `EMFILE`。未删用例、未降低断言、未把跳过数报为 0。详细名称与错误见 `.autome/output/w5-r6-full.txt`。

desktop 唯一失败是 `test/dom.test.js` 的 `electron dom harness: real-DOM assertions against the actual renderer`，子进程 status=null。按原 harness 命令单独复现，结果 status=null、signal=SIGABRT、stderr 空；没有证据判断更深层原因。本轮未改 desktop，交由可运行 Electron 的审计环境复验，不宣称 desktop 全量通过。

## 负向对照

四次逐一变异 palette-model，使用相同项目命令 `npm test --prefix apps/web -- test/palette.test.ts`；每次从原版开始，结束后恢复原文（runner 的 finally 也保证恢复）。

| 注入错误 | 红色结果（退出码 1） | 对应日志 |
|---|---|---|
| Mac 同时接受 Ctrl | 快捷键用例失败，1 失败/6 通过 | w5-r6-negative-mac-ctrl.txt |
| down/up 不跳过禁用项 | 键盘迁移及写门禁用例失败，2 失败/5 通过 | w5-r6-negative-skip-disabled.txt |
| 搜索只匹配标题 | ID 检索断言失败，1 失败/6 通过 | w5-r6-negative-title-only.txt |
| 新建项不读写门禁 | 写门禁及 SSR 禁用语义失败，2 失败/5 通过 | w5-r6-negative-write-gate.txt |

恢复后 palette + shell 11/11 通过，退出码 0（`.autome/output/w5-r6-restored.txt`）。这些是设计负向对照，不是上一轮退回缺陷；无需要迁入的审计临时测试。

## 结论与交接

M-05 定向验收及四条负向对照通过，标记待审，由审计独立复验。全量环境阻塞与 Electron SIGABRT 如上，未取得浏览器视觉或真实键盘证据，人工清单保持原样。本轮结束，不启动下一会话。
