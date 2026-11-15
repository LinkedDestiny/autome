# M-03 · 实现 #5 · 实现已落地，环境阻塞，保持开放

## 修改与结论

详情页增加「备注与依赖」标签：备注列表、角色/轮次/时间送达状态、8000 字符表单、终态说明、await_merge 的 reopen 复选框、原文错误与 warning。沿用写网关和连接/提交禁用态。新增 note 与 set_after 的 action 映射（不加入顶部 OP_RULES）；set_after 的界面仍属于 M-04。新增 7 个备注单测、3 个 action 单测、2 个浏览器用例，注册 unit 分组；globalTimeout 调至 360 秒。

上一轮审计 M-02 通过，没有针对本里程碑的退回检查，因而本轮没有必须逐字搬入的审计回归用例。

**不能标待审**：两条浏览器验收尚未执行；保持 M-03 开放、reopen 0。状态块 implementation-round 为 5/65。下一轮先在允许进程检查/监听的环境执行原验收命令，不绕过夹具安全检查。

## 命令与证据

- 基线：`(cd apps/web && npx vitest run test/task-action.test.ts test/write-gate.test.ts test/test-groups.test.ts)`，63/63；日志 `.autome/output/m03-baseline.txt`。
- `cargo build -p automed -p autome-cli`，退出 0；日志 `.autome/output/m03-build-core.txt`。
- `(cd apps/web && npx vitest run test/task-notes.test.ts test/task-action.test.ts test/write-gate.test.ts test/test-groups.test.ts)`，最终 73/73，4 文件；日志 `.autome/output/m03-unit-final.txt`。相对改动前实测 63 **+10**：task-notes 新增 7，task-action 5→8（+3），write-gate 54 与 test-groups 4 不变。
- `npm run build --prefix apps/web`，退出 0；日志 `.autome/output/m03-build-web.txt`。
- `(cd apps/web && npx playwright test e2e/controls.spec.ts -g "B-1|B-4 命令行备注")`，退出非 0。新增用例总数 2（原先没有 controls.spec.ts，+2），**执行 0**；globalSetup 第一项 `leftoverProcesses(root)` 报 `spawnSync ps EPERM`，尚未 prepare 或 spawn 面板。随后 teardown 同样受阻。日志 `.autome/output/m03-browser.txt`。不可把未执行计为通过。该验收范围合计定义 75 个用例，比原基线 63 增加 12；已跑通过 73，浏览器 2 未跑。
- `(cd apps/web && npx vitest related --run app/components/task-notes.tsx app/components/task-action.ts app/components/task-ops.ts app/components/task-changes-model.ts app/components/task-detail-data.ts 'app/routes/tasks.$id.tsx')`：19 文件、**145 个用例**，134 通过、5 失败、6 跳过。日志 `.autome/output/m03-related.txt`。本轮没有该动态 related 范围的改前实测基线，不能把它与上轮 M-02 的 74 个不同范围用例作差；按本轮测试文件增量可解释 +10，其余文件用例未改。
  - 5 个失败用例均是 task-detail-real / task-docs-real / task-merge-real / task-ops-real / task-timeline-real 的进程清理断言，报 `spawnSync ps EPERM`。
  - task-actions-http、task-live-real 共 6 个用例因 beforeAll `listen EPERM 127.0.0.1` 未执行；task-live-real 清理另报 ps EPERM。整组 7 文件失败，不能称通过。
- `git diff --check`：通过。没有运行 ci-local、ci-quick 或全项目 lint/typecheck/clippy 门禁。

## 标待审前自审清单（已执行的层次及缺口）

1. **情形表逐行**
   - 列表编号、时间、正文：task-notes「未送达与送达文案…」通过，正文含 HTML 字符及换行，验证转义与原文保留。
   - 未送达「等待下一轮」/已送达中文角色和轮次：同一用例分别渲染通过；未知角色保留原值。
   - 草稿、排队、运行、暂停、失败、停止：逐状态表单呈现通过；done/cancelled 隐藏表单、显示终态说明通过。
   - await_merge 才显示 reopen：active 各节点和 stopped/await_merge 对照通过。action 勾选传 true、未勾选传 false，并保留正文。
   - 合并期间 warning：role=status 呈现通过；action 成功回包透传 warning 通过。
   - 离线禁用：fieldset disabled 与原因通过。提交中禁用复用 useNavigation；尚无真实浏览器取证。
   - 真实网页写入下一轮简报、真实送达、CLI 五秒出现：已写 B-1/B-4 用例，但沙箱阻止执行，**待复验**。
2. **失败分支**
   - 空输入、纯空白、超长：action 拒绝且不调用写网关，通过。
   - await_merge 未勾 reopen、Bug 预算耗尽：用内核拒绝文案替身验证 action 和页面原文透传，通过；真实内核这两条拒绝尚未专项取证。
   - 授权拒绝、超时、服务不可达：逐条替身回包验证原文显示，通过；既有 action 400/409/503/504 透传用例通过。
   - 读盘失败：不适用，备注 UI 仅消费 task.get，不新增文件读取路径；简报读盘属于尚未执行的浏览器断言。
3. **全部迁移**
   - 待送达→已送达：两态渲染验证通过，真实迁移由 B-1 覆盖但未执行。
   - await_merge + 不 reopen → 拒绝；+ reopen → 参数 true：action/页面替身通过；真实送回实现轮仍待复验，未声称内核迁移已跑。
   - merging/main_check/cleanup：逐节点表单存在且无 reopen，warning 替身呈现通过。
   - 非终态→done/cancelled：分别呈现终态且不再提供表单；内核本身状态机未修改。
   - CLI→面板实时更新：B-4 尚未执行。没有新增其它传输通道。
4. **输入域边界**
   - 空串、纯空白、1 字符、8000、8001 逐项通过；保留前后空格和换行，maxLength=8000。
   - reopen 有/无，旧内核无 notes，未知角色，空列表、单条备注均检查通过。
   - set_after 空数组/两个前置参数组装通过（只做设计指定 action 预备，不推进 M-04）。
   - 前导零、闰日、跨年：不适用，备注输入域是自由文本，不解析数值或日期；显示时间复用已有 formatClock。

## 负向对照与还原

- 临时删除 `params.reopen = value('reopen') === 'yes';`，执行 `npx vitest run test/task-action.test.ts -t 'note 参数含 reopen'`：退出 1，精确参数断言失败。日志 `.autome/output/m03-negative-reopen.txt`。
- 临时将送达判断改为恒 false，执行 task-notes.test.ts：退出 1，送达与未知角色用例失败。日志 `.autome/output/m03-negative-delivery.txt`。这是单测补充，不冒充契约要求的浏览器负向取证。
- 临时将 WRITE_OPS.note 改为 task.reply，执行 task-action.test.ts：退出 1，note 精确 RPC 参数断言失败。日志 `.autome/output/m03-negative-method.txt`。浏览器简报负向对照仍待环境支持。
- 三次变异均在 finally 中还原。最终原验收单测 73/73，未保留变异。

## 环境与收尾

浏览器夹具在启动进程前拒绝运行。related 测试使用项目临时 HOME/AUTOME_HOME；其 teardown 即使 ps 失败仍执行 stopServices、task.stop 与 core.stop，然后抛出错误并保留临时目录。无法读取进程表，因此**未核实最终零残留**，不能宣称清理验收通过。未手工启动额外面板/浏览器/内核，未触碰真实账本。人工观感 H-05 本轮未验。
