# r24 交接修复独立复验 · 审计 #15 · 通过

## 范围与结论

- 起点：`25ece3dbe81df8e9f9f2f5fd5c03a59f14ce4cc0`，工作区干净。设计状态为 `已完成`、`implementation-round: 24/66`，13 个里程碑均已完成，没有待审项。
- 按操作者备注 #3 延续 `M-13-r24-impl.md` 的交接索引，本证据使用配对的 `M-13-r24-audit.md`；实际测试修复归属 M-07 的 D-Q1/D-Q2（文字展开与日志展开互相独立）。不重复审计已闭合的 M-13 焦点实现。
- 结论：**通过**。可在本会话执行的 M-07 契约和独立判别检查通过；未发现产品缺陷，不计 reopen。实时日志、浏览器布局两条命令受沙箱权限阻塞，转入 H-12/H-13，由宿主运行、用户合并前确认，不能表述为本轮通过。
- 本轮没有待审状态可关闭。为满足本审计指令要求的里程碑表更新，仅在 M-07 标题补明设计已有的“会话日志展开”；13 行状态与 reopen 均不变，不制造新的开放/待审项。备注 #3 的“不改表”约束针对已结束的实现交接轮，已核对该轮遵守；本轮按审计指令维护表。
- 审计不增加实现轮号，保留 `24/66`、`current-milestone: 无`、`convergence-mode: normal`。不改既有关闭证据指针，不将本轮结论抄入设计正文。

## 先负向对照，再还原复验

每个变体单独运行、在 `finally` 中用原始内容还原。前三项均执行 M-07 完整六文件验收命令：

`cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts`

| 错误注入 | 实测 | 判别意义 | 原始输出 |
|---|---|---|---|
| 将摘要从 summary 移到独立 p | 退出 1，4 失败 / 82 通过，共 86 | 紧凑结构与完整文字入口断言拦截 | `.autome/output/audit15/negative-summary-separate.log` |
| 删除 min-block-size 并允许折叠摘要换行 | 退出 1，1 失败 / 85 通过，共 86 | 40px/折叠省略断言拦截 | `.autome/output/audit15/negative-row-wrap.log` |
| 只为会话渲染文字 details | 退出 1，5 失败 / 81 通过，共 86 | 系统事件完整文字入口断言拦截 | `.autome/output/audit15/negative-session-only.log` |
| 把 defaultOpen 改成 !entry.session.running | 新造检查退出 1，1 失败 | 按会话 ID 检测历史/运行状态颠倒，即使总展开数仍是 1 | `.autome/output/audit15/negative-reversed-log.log` |

另做过摘要标签从 span 改成 p 的较弱探针（2 失败 / 84 通过），随后补做了契约所述的独立 p 变体，以上表后者为正式证据。新造检查首次运行有临时模块路径错误（0 用例），修正临时检查路径后重新注入才得到表中的真实断言失败；不把工具加载失败当作负向命中。

## 还原后的结果与基线

| 命令 / 检查 | 当前结果 | 总数与基线差 |
|---|---|---|
| M-07 六文件验收命令 | 退出 0，86 通过 / 0 失败 | 86，对比 M-07-r14-audit 的 86，差 0 |
| `cd apps/web && npx vitest run --config ../../.autome/output/audit15/vitest.config.mjs` | 退出 0，1 通过 / 0 失败 | 本轮新增临时检查 1；未修改项目用例数 |
| `cd apps/web && npx vitest run test/task-log-live.test.ts` | 退出 1，setup 失败，3 skipped | 收集 3，与实现证据 3 相同；本轮执行通过数为 0 |
| `cd apps/web && npx playwright test e2e/layout.spec.ts` | 退出 1，setup 的 ps 权限失败 | 未进入浏览器用例，不能以历史 16 passed 代替 |
| `cd apps/web && npx playwright test --list e2e/layout.spec.ts` | 退出 0，收集 16 | 对比实现证据 16，差 0；仅清单检查 |

输出分别为 `.autome/output/audit15/{acceptance,independent,live,layout,layout-list}.log`。本轮没有运行 `sh scripts/ci-local.sh`、全量单元测试或旧审计临时检查；宿主快检信息只作背景，没有替代独立复验。

## 新造判别检查与因果核对

临时检查 `.autome/output/audit15/log-state.test.ts` 构造一个历史会话、一个运行会话，为二者都加入失败详情，同时按正序/倒序渲染。断言共有 6 个 details（2 文字、2 失败、2 日志），日志筛选只能得到 2 个，并逐一按 `data-session-id` 核对 `open`。错误变体仍恰好展开一个日志，新检查能识别展开对象错误；还原后通过。原项目 `task-timeline.test.ts` 也已逐个覆盖 running=true/false，因此不是尚未闭合的产品缺陷或必须修项目测试的验证缺口。

源码独立核对：`task-timeline.tsx` 仍传 `defaultOpen={entry.session.running}`；文字格、失败详情与 `SessionLog` 是独立元素；`task-log-live.test.ts` 仅对 `class="session-log"` 计数，符合 D-Q2，没有回退为全部 details 计数。

## 环境阻塞及后续确认

- 实时日志命令的关键错误为 `Error: listen EPERM: operation not permitted 127.0.0.1`；清理阶段另报 `Error: spawnSync ps EPERM`。输出尾部：`Test Files 1 failed (1)`、`Tests 3 skipped (3)`。
- 浏览器命令的关键错误为 `e2e setup failed: Error: spawnSync ps EPERM`，发生在 `harness/setup.ts` 的 `leftoverProcesses(root)`，在 prepare/spawn 之前。尾部是 gate-reporter 的 setup 文件清单及 `autome-gate-cases end`，不是用例通过摘要。
- 因果证据：实时测试 beforeAll 的第一步就是监听权限探针，失败发生在内核与服务器启动之前；浏览器 fixture 在启动前扫描进程并失败。两项均不能证明产品行为错误，也不能声称完成了真实实时订阅/浏览器布局复验。
- H-12/H-13 保存两条原命令，宿主在具备监听端口和进程查看权限的环境执行，用户合并前确认结果。既有 H-01…H-11 保留；本轮不弱化断言或修改权限。

## 操作者备注逐条核对

- 备注 #2.1：实现证据已记保留 session-log 定向计数；源码与本轮 SSR 检查确认，未回退。
- 备注 #2.2：`fix-r22-impl.md` 已删除；实现证据明确解释 M-07 与 M-13 文件名冲突，按后到备注 #3 采用 M-13 索引，同时说明实际归属 M-07，未缺项。
- 备注 #2.3：实现证据记明 Scripts 用例 02 退出 143 为宿主当时负载/60 秒超时，不改 scripts、不放宽用例。本轮沿用操作者对该历史事件的说明，没有重新验证或把它算作通过。
- 备注 #2.4：实现证据有两条指定命令与历史 3/16 通过结果。本轮自己执行得到上述权限阻塞，明确区分历史结果与当前证据，转 H-12/H-13。
- 备注 #3.1：状态块为实现简报轮号 `24/66`，不是审计轮号 15。
- 备注 #3.2：配对实现证据为 `M-13-r24-impl.md`，该轮已从 r23 改名；内容保留四项备注处理与不适用自审理由。
- 备注 #3.3：历史提交 `ebd8ebdf` 已保存交接修复，本轮另提交审计文档；未改产品、测试或 scripts。

## 范围守卫

临时注入涉及的 `task-timeline.tsx` 与 `app.css` 已逐字还原，`git diff --exit-code -- apps/web` 为 0。最终只提交 `docs/面板观感收尾/` 内文档；`.autome/output/audit15/` 被 Git 忽略。未切分支、未推送、未启动下一会话。
