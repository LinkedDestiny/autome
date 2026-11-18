# M-08 实现轮 #15

范围：D-3…D-6。上一轮审计 M-07 通过，没有退回缺陷或须搬入测试体系的审计检查。

## 实现

- 模型导出 `hasVisibleOps`，排除 `decide`。无可见操作且无错误/通知时不渲染操作区；有结果仍展示，没有操作时不附带写入授权提示。
- 里程碑概要固定在滚动列表前；全部里程碑单行横向滚动。当前项黄色实心；挂载与当前 ID 变化时仅调整列表 `scrollLeft`，不滚动页面；普通实时更新不重置滚动。
- 删除文档链接和证据 summary 的额外 padding；证据入口使用胶囊，展开占整行，链接按 gap 换行；保留 nav 内部滚动。
- 阶段条单行省略、完整 title 及混合计费回归用例已由前轮交付，本轮保留并复验。
- `layout.spec.ts` 补里程碑单行、文档胶囊在 nav 内完整可见的几何断言。本轮未执行真实浏览器布局验收，仍交宿主/H-05 确认。

## 命令与结果

验收命令（修改前、修改后均执行）：

`(cd apps/web && npx vitest run test/task-ops-view.test.ts test/task-header.test.ts test/task-stage.test.ts test/task-docs-view.test.ts test/task-summary.test.ts test/task-page.test.ts test/styles.test.ts)`

- 修改前 79 项，修改后 84 项，全部通过，0 失败，0 跳过。与上一轮代码在相同命令下的基线相比 **+5**：操作区 +1、里程碑 +3、文档胶囊 +1。阶段条 21 项已在基线内，不重复计新增。上一轮 M-07 的 86 项属于不同文件集合，不可直接作数量差。
- 原始输出：`.autome/output/m08-baseline.txt`、`m08-acceptance.txt`。
- 三条负向对照分别在修改后执行，每次还原源码：删掉空操作区返回、把 nowrap 改 wrap、重新加入文档链接 padding；分别运行对应 `task-ops-view`、`task-stage`、`task-docs-view` 文件，退出码均为 1。日志 `.autome/output/m08-negative-{empty,wrap,padding}.txt`。没有遗留注入。
- `(cd apps/web && npx playwright test --list e2e/layout.spec.ts)` 成功，验证用例可发现；不等同于布局用例通过。日志 `.autome/output/m08-layout-list.txt`。
- `git diff --check` 通过。未运行 `scripts/ci-local.sh`。

## 标待审前四项自审

1. **情形表逐行**：D-Q3 无操作/仅 decide（done、cancelled；授权/未授权）均无空框；有操作正常显示；错误与成功通知保留，空结果对象不制造空框。D-Q4 0/1/30 项渲染、概要计数、当前项开放/待审/已完成的黄色实心与全部项保留通过；横向 overflow/nowrap/flex-none 规则通过。D-Q5 38 份证据全部可达、选择末份证据后展开、summary 胶囊、无额外 padding、展开占整行规则通过。D-Q6 既有混合计费及工具文案完整 title 和省略规则通过。
2. **失败分支**：既有未授权禁用、取消确认、文档路径拒绝/清单缺失/读盘失败及 EACCES、状态块读取失败用例通过；空输入和缺文档回退通过。本里程碑不新增外部服务请求或超时处理，外部不可达/超时分支不适用。
3. **全部迁移**：既有 stage 测试遍历 feature/bug 的 active、paused、queued、stopped、failed 节点映射以及 done/cancelled 回退；新增当前项 open/pending/done 表示、左右越界/已可见/超宽项/无当前项滚动检查。effect 仅依赖 currentId 的源码守卫通过，防止普通刷新重置。文档选中证据时展开与未选中时默认收起由渲染检查覆盖；真实挂载和实时更新几何仍属浏览器验收，不声称静态检查证明实际像素。
4. **输入域边界**：0/1/30 里程碑、0/38 证据、空操作/仅 decide/空结果/错误/通知、混合计费 12 个无计费与全无计费均通过。未新增日期/数值解析输入域，前导零、闰日、跨年、最大值解析不适用。

## 快检与交付

- 已执行指定 `sh scripts/ci-quick.sh --since 90607571d70daa9ad0043ab422f11aebf75a2305`，日志 `.autome/output/m08-quick.txt`：38 步中 36 步成功、2 步失败。Format、Clippy、Web lint/typecheck/build 与已执行相关单元用例成功。`task-detail-real` 6 项中 5 通过、1 失败，另 suite teardown 失败，均为 `spawnSync ps EPERM`；Web e2e globalSetup/清理同样 EPERM，浏览器用例未执行。相关阶段 136 秒超过 120 秒准入预算，余下 9 步未跑。该结果是环境阻塞，不声称快检通过；须由宿主重验。
- 本轮验收总数以明确的 M-08 命令 84 项为准，快检自动选出的不同分组不混入此基线。未修改测试环境、权限或进程扫描机制规避失败。
