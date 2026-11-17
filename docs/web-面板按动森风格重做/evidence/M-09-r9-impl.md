# M-09 · 实现 #9

结论：实现与可执行测试子集通过；浏览器验收在启动前被沙箱阻断，M-09 保持「开放」，reopen 仍为 0。状态轮次为 9/65；未标「待审」或「已完成」。没有运行全量门禁，没有启动下一会话。

## 实现范围

- 技能页替换占位页：项目切换、四种筛选与动态计数、名称/路径搜索、内部滚动网格、键盘可选卡片；显示来源、工具、角色必用与冲突。无项目时只显示全局选项，隐藏本项目筛选。
- 绑定抽屉提供全局与当前项目两个范围，六个角色按该范围的角色工具、技能来源计算可见性，复用 role-rules。纯项目技能禁用全局；混合同名技能允许全局，但不合并项目来源的工具。
- 每次保存重新读目标范围配置，只对成员关系发生变化的角色调用 config.set_role。项目写 resolved 列表，全局读全局范围配置，避免把项目覆盖带入全局。全局保存后逐项目读回 resolved 技能，报告缺失/仍保留名单；中途失败返回内核原文及已写角色。
- open.path 仅新增 skill 类型：服务端通过 skills.list 按名称和来源序号解析目录。拒绝额外路径和非法序号，未知技能/来源返回 404，其它类型仍 501。startServer 注入 opener，默认以参数数组调用 /usr/bin/open；单元测试使用记录调用的替身。
- 未改内核、协议、技能文件写入能力、依赖或其它里程碑。真实测试只在各自临时 HOME 和仓库创建技能。
- testids 的内部滚动白名单保留所有原成员，仅加入 skills-grid 与 skills-drawer-body；后者是抽屉里的实际滚动容器。增加 skillCard 注册与渲染断言。样式、颜色、圆角与标签守卫未放宽。
- 新浏览器标题严格对应设计的「1512×944 技能」和「F-1 技能按仅当前项目绑定后只进该项目」。前者创建 24 张卡检查内部滚动、一屏并截图；后者验证搜索、绑定、范围切换、取消绑定、项目隔离及返回全局，finally 恢复配置。没有删除或改名已有用例。

## 命令、结果与基线差

原始输出在 `.autome/output/`，本文件只记录判据。

1. M-09 验收：
   `(cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/open-path.test.ts test/write-gate.test.ts test/gateway.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "技能|F-1")`
   - 首次 Vitest 141/144：标签守卫拒绝页面中的“完成”字面量；两个新 SSR 测试错误假设属性序列。改用“无需修改绑定”，测试按 React 实际属性顺序检查；同时正确断言三个 Codex 角色禁用。
   - 第二次 Vitest 144/144，通过；浏览器 globalSetup 在 leftoverProcesses 的 `ps` 调用即失败：`spawnSync ps EPERM`。两条用例实际执行 0 条，清理扫描也遭同一拒绝。见 `m09-vitest.log`、`m09-accept.log`。
   - 最后增加一个技能卡稳定选择器断言后，验收指定九文件为 **145 个**，均在第 2 项最终命令通过。
2. 最终验收子集加直接相关守卫：
   `(cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/open-path.test.ts test/write-gate.test.ts test/gateway.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts test/styles.test.ts test/live-routes.test.ts test/clock.test.ts)`
   - **12 文件 185 个全部通过**，见 `m09-final-tests.log`。验收 145；直接相关 styles、live-routes、clock 共 40。重复运行不重复计数。
   - 新文件贡献：skills-model 16、skills-page 8、skills-real 10、open-path 14，共 48 个；既有 testids 新增 1 个。
3. 数量对照：上一轮 M-08 审计验收基线 Rust 2 + Vitest 96 + 浏览器 2 = **100**。本轮指定范围 Vitest 145 + 浏览器 2 = **147**，净增 **47**：Web +49（四个新文件 +48，testids +1；换入 write-gate/gateway、换出 M-08 专属文件的用例数净差为 0），本轮无 Rust 验收 -2，浏览器数量不变但换成 M-09 两条。这里是里程碑命令范围对照，不冒充全项目累计总数。本轮实际通过为 185（含直接相关 40），浏览器 2 条待执行。
4. `npm run lint`、`npm run typecheck`（apps/web 内）：均退出 0，见 `m09-static-final.log`。
5. 上轮审计 M-08 为通过，没有审计退回的判别检查需要逐字搬入。本轮首轮失败均保留在日志中；快检发现的替身返回类型与时钟守卫问题也已修复：restart 替身返回 `{ restarted: true }`，搜索使用确定性的 toLowerCase，最终 185 用例包含原 clock 守卫并转绿。

## 标待审之前的四项自审

本轮仍因浏览器阻断保持开放；四项均已逐项核查，未取得的证据明确列出。

1. **情形表逐行**
   - F-1 全部/全局/本项目/已绑定计数随项目变化、搜索名称和路径：真实回包模型测试通过；无项目时隐藏筛选与项目选项，SSR 通过。实际浏览器切换待执行。
   - F-1 卡片字段、来源路径、必用角色、未绑定与 conflicts 红色说明：SSR 通过；网格内部滚动属性通过。1512×944 的实际滚动与截图待执行。
   - F-Q2 项目绑定只进 A、不进 B/C/全局：真实内核通过。全局绑定进入 B/C，A 只改模型仍收到，C 显式技能覆盖则列入缺失名单：真实内核通过。
   - F-Q2 项目取消全局绑定写完整列表；全局取消后仍保留项目绑定的名单：真实内核通过。全局写入不会带上 A 的 local 技能，且未变化角色不写：模型及真实内核通过。
   - F-Q3 纯全局、纯项目、同名混合三类 × 两范围：模型通过；同名混合 project_scoped=true 仍能全局写入，且全局 Codex 不可见、项目 Codex 可见：真实内核通过。
   - F-Q4 按名解析、多来源按序号、404/501、替身收到服务器路径：14 个 open-path 用例通过。真实 Finder 打开属既有 H-07，本轮未实跑。
   - F-Q1 描述不适用：明确归 M-12，本轮卡片使用真实来源路径，不编造描述。
2. **失败分支**
   - 拒绝授权：SSR 验证 fieldset、保存、打开按钮禁用；注入 opener 拒绝时返回 503 和原原因、不报告成功。页面交互时动态禁用待浏览器验证。
   - 内核拒绝：不可见技能绕过页面直写，action 保留与直接 RPC 相同的原始 message；中途拒绝不写后续角色，真实内核通过。
   - 读盘失败：破坏临时 config.toml，loader 和保存均返回相同内核错误；open.path 的读取错误可见，均通过。
   - 超时：注入带 timeout 标记的读取失败，网关返回 504 且不调用打开器，通过。
   - 空输入：空技能名、无项目时请求项目范围被拒；空搜索匹配全量、无匹配显示空提示，通过。
   - 外部服务不可达：本里程碑没有第三方服务调用，不适用网络验收；无网关返回“内核不可用”，通过。
3. **全部迁移**
   - 未绑定→绑定→取消，在全局与项目两个方向均有模型/真实内核断言；项目继承→显式技能列表覆盖、全局变更后继续保持覆盖，通过。
   - 全局新绑定→项目模型覆盖仍继承、项目技能列表覆盖不继承；全局取消→项目显式列表仍保留，通过。
   - 合法→内核拒绝、部分保存→停止、离线/读盘失败→不写、打开成功/未知名/未知来源/非 skill/超时/打开器失败：对应测试通过。
   - 页面项目切换、筛选/搜索、抽屉打开/关闭、范围往返与保存反馈的真实交互已写进 F-1；因 globalSetup 阻断尚未执行。未把 SSR 和模型测试等同于真实浏览器证据。
4. **输入域边界**
   - 空列表、单技能、空搜索、空白包裹搜索、大写查询、无匹配、单元素绑定取消为空数组：通过。
   - 来源序号：默认 0、第二来源 1、负数、小数、字符串“0”、超出安全整数、最大安全整数但超出来源数量：通过；错误时均不打开。
   - 名称中路径片段、请求额外 path、未知名、空名称：拒绝，通过。
   - 日期前导零/闰日/跨年不适用：此功能没有日期或数字文本输入；序号是服务端验证的整数。

## 浏览器阻断与交接

以下两条均未执行，不能计为通过或跳过：
- `1512×944 技能`
- `F-1 技能按仅当前项目绑定后只进该项目`

审计/宿主在允许原有进程扫描及浏览器启动的环境运行：
`(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "技能|F-1")`
并检查 `.autome/output/m09-layout.png` 与 `m09-drawer.png`。没有改弱进程清理、超时、控制台或一屏判据，也没有取得可冒充视觉验收的截图。

## 指定快检与提交

- 已按指定起点运行 `sh scripts/ci-quick.sh --since 187329105c6a99d020ab93914e87a1c6e32b9bc5`。首次 92 步、失败 3 步（替身类型、clock 守卫、浏览器 ps EPERM）；前两项修复后重跑同一命令。
- 最终快检执行 95 步、沿用 0 步、失败 1 步，用时 161 秒，退出 1；唯一失败是浏览器准备阶段 `spawnSync ps EPERM`。Rust fmt/clippy、Web lint/typecheck/build 均通过。脚本列为超出 120 秒预算的未跑步骤留给宿主合并前门禁，不计通过；快检自动选择的其它测试不重复计入本轮固定子集总数。见 `m09-quick-final.log`。
- `git diff --check` 通过；状态字段、9/65、M-09 开放状态与单行证据指针校验通过。设计文档仅更新状态块和本里程碑证据指针；retro 仅追加一行。
