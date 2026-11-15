# M-05 · 实现 #8

结论：待审。服务端视图、时间线窗口和按需标签读取已实现；可运行的验收通过。浏览器及进程表验收受沙箱限制，按任务总-6、设计验收契约的 ★ 规则交审计/宿主复验，不作为已通过证据。

## 实现与范围

- `loadTaskDetail` 在服务端算 header、stage、notice、ops、docs、timeline；不再发送原始 task.get、config.get、dashboard.get 或事件。操作已计算完成，因此配置启用表也不必发给浏览器。文档清单转成名字/标签视图，绝对路径仅用于服务端 pickDocument。
- 最近 20 个会话窗口；URL 的 from 固定起点，非法 id 回退最近窗口；timeline=all 展开全部。服务端先计算完整事件对应的稳定条目，再裁窗口，保留窗口前快照对会话结果的影响。
- 会话只投影时间线/日志需要的字段。文档仅选中 docs 时读取和解析，改动仅选中 changes 时读取；缺省选中规则保持原样（已完成但未授权任务缺省 docs，不再后台预取 changes）。
- 主标签、任务卡/正文/证据链接、更早链接、钉住 effect 统一用 withParams；钉住使用回调保留现有参数。展开更早时恢复时间线 scrollTop，React key 仍用会话 id 与事件既有 key。
- 自审发现：依赖只读判定需要完整历史中的 impl 会话，不能用裁剪后的 sessions。新增 dependencyReason 服务端视图，并传给依赖组件；增加“实现后重跑设计且旧 impl 在窗口外”的页面回归，避免错误显示保存依赖。
- 既有视图/真实内核测试适配新的 loader 合约，原始事件的领域断言改从 taskEvents 服务读取，UI 断言使用 loader 的 timeline。没有改动内核、协议、门禁、重试次数或跳过策略。

## 验收命令与结果

原始日志均在 `.autome/output/`，本文件只记录结论。

1. `(cd apps/web && npx vitest run test/task-detail-payload.test.ts test/task-detail-data.test.ts test/task-timeline.test.ts test/test-groups.test.ts)`：最终四文件为 **41 例**（4 + 11 + 22 + 4）。最后一次加跑 `test/queue-deps.test.ts` 的同批为 **49/49**，日志 `m05-acceptance-final.txt`。上一轮 M-04 的验收是 50 例，当前四文件比它少 9，原因是里程碑切换、文件集合不同；不能解释成删测。同范围改前为 30 例，本轮 **+11**：payload 新增 4、data 从 4 增至 11；timeline 与分组守卫不变。设计原预期 40，额外 1 例就是历史实现依赖只读回归。
2. `cargo build -p automed -p autome-cli`：通过，`m05-cargo.txt`。
3. `npm run build --prefix apps/web`：通过，最后版本 `m05-build-final.txt`。
4. `(cd apps/web && npx playwright test e2e/manual-items.spec.ts e2e/layout.spec.ts)`：启动前 `spawnSync ps EPERM`，未运行浏览器用例；`m05-browser.txt`。`--list` 实数 **19**，相对同范围基线 17 **+2**（C-2、C-2b）；layout 改前/改后均 12，manual-items 从 5 到 7。C-2 使用真实内核+假 CLI 的 22 会话任务（含整理与七轮设计/评审/裁决），放行后第 23 会话进入；验证最早可见日志和两个滚动位置、文档导航与全量展开。未提高 90 秒用例上限。
5. `npx vitest related --run app/components/task-detail-data.ts app/components/task-url.ts app/components/task-timeline.tsx app/components/task-docs-model.ts app/components/task-docs.tsx 'app/routes/tasks.$id.tsx'`：17 文件 115 例，105 通过、4 个清理断言因 ps EPERM 失败、6 个监听服务用例因 beforeAll 的 listen EPERM 未执行；另有同根因 afterAll 报错。日志 `m05-related-final.txt`。4 个真实内核文件的 12 个功能用例均通过；不声称整个命令通过。该次在最后增加 dependencyReason 回归前，计数比当时对应源代码基线 +10，无原用例删除。
6. 最后追加 `app/components/task-dependencies.tsx`，以相同 related 命令加 `--project unit`：13 文件 **113/113**，日志 `m05-related-unit-final.txt`。本范围按未增删的旧用例计为 102，本轮 +11（同第 1 项）；新纳入依赖组件后多覆盖 queue-deps 与 task-notes，不能与第 5 项直接相减。各次范围重叠，不把它们相加当作总用例数。
7. TypeScript compiler API 只报告本轮改动文件的诊断：0；eslint 仅传本轮改动 TS/TSX：退出 0，`m05-static-final.txt`。`git diff --check` 通过。未跑 ci-local、ci-quick、全仓 lint/clippy/typecheck 门禁命令。

## 体积与负向对照

口径按 C-Q1：loader JSON + loadShell JSON + 静态页面标记。是夹具估算，**不是生产 SSR 网络字节**；真实 SSR 留给 M-13，真实账本 H-02 未接触。

| 情形 | loader 数据字节 | 文档估算字节 |
|---|---:|---:|
| 同一 60 会话夹具，改前 | 527528 | 572880 |
| 改后默认 20 会话 | 23650 | 44882 |
| from=session-40，60 会话 | 23650 | 46274 |
| from 不变，增加 5 会话 | 26860 | 52414 |
| timeline=all，60 会话 | 49675 | 95789 |

夹具有 60 会话、30 里程碑、63 文档、480 事件；字段骨架来自既有 W6 实际回包。改前 ≥ 300 KB；payload 4 例原样运行结果为 3 红 1 绿（两项体积与原始字段拒绝红，全量条目绿），`m05-payload-red.txt`。修后 4 绿，最终字节 `m05-bytes-final.txt`。

以下变异均只临时注入、执行后 finally 原样还原；还原后的 49 例与 113 例已绿：

- 忽略 pinned 起点：窗口用例红，`m05-negative-window.txt`。
- 条目 key 改为窗口内序号：窗口稳定 id 断言红，`m05-negative-keys.txt`。
- withParams 从空参数开始：3 例红，`m05-negative-query.txt`。
- dependencyReason 使用已裁剪投影：新增依赖只读页面用例红，`m05-negative-dependency.txt`。
- 浏览器 C-2/C-2b 的变异执行仍受 ps/监听限制，交审计轮，不能用上述单测替代滚动状态实测。

上轮审计 M-04 通过，无退回缺陷及需要逐字搬入的检查；BL-13 不属于本里程碑，未改。

## 标待审之前的四项自审清单

### 1. 情形表逐行

C-Q1/C-Q2/C-Q3 无单独情形表，逐条落法核对：

- 服务端视图与路径字段裁剪：payload 的无 absolute/log_path/events 用例通过；stage/header/ops 的旧功能测试及真实会话结果用例通过。
- 缺省 20、earlier、from 固定、新增 5、非法 from、timeline=all：data 与 payload 用例通过，稳定 key 的负向对照能抓错。
- 文档/改动标签按需读取：四种 tab 循环验证调用次数、未选中不读正文/不解析，已完成显式 changes 仍可读，通过。
- 文档清单和清理后正文：docs-view 8 例通过，真实内核合并后文档/来源/终态/改动功能断言通过；该套收尾因 ps 权限不通过，已单列。
- 四处查询出口：`rg -n '\?doc=|tab=|timeline=|setSearchParams' apps/web/app` 输出在 `m05-query-exits.txt`，硬编码查询串已消除，仅剩钉住 API。三个参数工具用例及渲染编码断言通过。
- 展开日志与两个滚动区、一屏布局：原 W7/W9 用例保留，C-2/C-2b 已加入，19 例待宿主执行；不能声称视觉验收通过。
- 依赖旧历史防线：新增页面回归及 queue-deps 8 例通过。

### 2. 设计点名的失败分支

- 内核离线/null、503、504、404、not_found、其它 500 原文：data 既有用例通过。
- config/dashboard 超时、历史服务离线/缺失：不阻断详情，timeline 错误可见、ops 仍存在，data 用例通过。
- 文档路径拒绝、缺失文档、EACCES、合并清理后缺盘提示：data/docs-view/真实文档用例覆盖，通过；未选中 docs 时不触发读取。
- 改动授权拒绝、离线、超时、读盘错误：task-changes 既有原文呈现用例通过。
- 新授权/HTTP 接口失败分支不适用：本轮未加接口或修改写入授权层；浏览器访问与完整清理无法取证的环境失败已经逐项记录，不加 skip。

### 3. 全部状态迁移

- URL 窗口：缺省→from 钉住，已有 from→新增会话，非法/空 from→缺省，from→all 并删 from，all→标签/文档导航保持 all；纯函数/loader 用例已执行。DOM 展开/滚动迁移待浏览器复验。
- 四标签 docs/changes/notes/decisions 的服务端选择与互不预取、任务卡/文档切换的参数保留：已执行。普通状态、合并各节点、已完成/已取消的缺省与显式标签读取均检查。
- 旧实现会话→重新设计且旧会话落在窗口外：仍只读依赖，页面用例通过。
- 正常结束、停止、崩溃/超时与事件结果：原 timeline 22 例保留并通过；真实内核正常/退出码 3/停止功能断言通过。本轮没有新增内核状态迁移。

### 4. 输入域边界

- 会话数 0、1、19、20、21、60、65；from 为最早、窗口起点、外任务、空串、带前导零的不同 id；all 优先于 from：通过。
- brief 为 null/空串/非空；doc 空串/路径/缺清单，以及中文空格与 & 编码：通过。
- 最大窗口不适用固定数值上限：timeline=all 按设计无上限，以 60 全量及 65 会话数据验证；日期闰日/跨年未改日期解析域，由 related 的 clock 边界用例继续通过。

## 环境与清理

所有真实内核测试沿用临时 AUTOME_HOME。Playwright 在预检 ps 处失败，未起面板/浏览器。related 的真实内核 teardown 已按既有 finally 路径调用 task.stop、core.stop；由于 ps 被拒，无法证明进程表为空，测试按原规则保留临时目录。未手工另起常驻服务，未接触真实账本。宿主需复验完整 related 与 19 条浏览器验收，并检查这些测试临时目录的残留；不得把本记录解释为已证明无残留。

## 交接

M-05 标待审，implementation-round 为 8/65；审计先补跑受限验收。通过后下一实现轮推进 M-06。提交结果见本轮最终回复；若 git 权限拒绝则提交由 Autome 兜底。
