# M-13 · 实现 #21

## 范围与修复

共用 Drawer 挂载时记录当前焦点并移入首个可聚焦控件，卸载时恢复仍连接的触发元素；容器带 `tabIndex=-1`。每次 Tab 重新查询可聚焦控件，过滤禁用、hidden/inert、不可见和负 tabindex 控件，正反向在首尾循环，零控件时回退容器。监听绑定到容器且卸载时移除。沿用命令面板的 effect 生命周期方式；角色与技能抽屉均通过共用骨架获得行为。没有更改其它导航或弹窗。

上一轮审计为 M-12 通过，没有需要逐字迁移的退回检查。

## 命令、数量与负向对照

- 基线：`(cd apps/web && npx vitest run test/ui-base.test.ts test/loop-page.test.ts test/skills-page.test.ts test/test-groups.test.ts)`，4 文件 56 用例通过。原始输出 `.autome/output/r21-baseline.txt`。
- 验收：`(cd apps/web && npx vitest run test/drawer-focus.test.ts test/ui-base.test.ts test/loop-page.test.ts test/skills-page.test.ts test/test-groups.test.ts)`，5 文件 66 用例全部通过，0 失败、0 跳过。相同既有文件基线净增 10：新文件 11 个，含从 ui-base 原样迁入的 1 个 Esc/按钮用例，因此 ui-base 减 1。新文件已登记 unit。最终输出 `.autome/output/r21-final.txt`。
- 删除焦点恢复：单跑 `npx vitest run test/drawer-focus.test.ts`，11 用例中 2 失败、9 通过，角色/技能恢复断言均捕获。输出 `.autome/output/r21-negative-restore.txt`。
- 移除反向循环：同命令，11 用例中 2 失败、9 通过，Shift+Tab 与单元素反向循环失败。输出 `.autome/output/r21-negative-reverse.txt`。
- 两项变异均已还原，之后重新运行上述完整里程碑验收命令，66 个全部通过。首次新测试存在导入路径笔误，纠正到真实 route 导出后通过；不是产品行为缺陷。

## 标待审之前的自审清单

1. **情形表逐行**：M-13 无单独情形表，按设计列举逐项检查：角色打开→首控件、关闭卸载→触发按钮；技能打开→首控件、关闭卸载→触发按钮（2 个真实组件 SSR + 模拟 hooks/DOM 用例）；Tab 末→首、Shift+Tab 首→末（2）；零控件→容器（1）；卸载清理监听（两个集成用例均断言）。全部通过。
2. **失败分支**：触发元素断开连接时不恢复但仍清理（1）；禁用、隐藏、不可见、负 tabindex 不作为候选（1）。授权、读盘、超时、空输入和外部服务不可达不适用：本里程碑没有 I/O 或输入请求；既有 skills-page 权限禁用检查仍通过。
3. **全部迁移**：关闭→挂载移入、打开→卸载归还（角色/技能各一次）；末→首、首→末；中间节点双向 Tab 保持原生行为；容器/候选外焦点→首或末；候选由禁用→启用后可达、非空→空后回退；卸载后键盘不再被拦截；Enter 不关闭、Esc 和按钮触发关闭。全部由 11 个焦点测试覆盖并通过。
4. **输入域边界**：零、单、多（3）控件均覆盖；全部候选被移除、disabled/hidden/不可见/tabindex=-1、触发元素断开均覆盖并通过。数值、日期、字符串输入域不适用：本改动只处理焦点与键盘事件。

## 验证边界

使用设计规定的模拟 DOM 焦点调用与键盘事件，不依赖端口；没有声称真实浏览器焦点验收通过。未运行全量门禁 `scripts/ci-local.sh`。

## 快检与静态验证

- `npm run lint --prefix apps/web` 与 `npm run typecheck --prefix apps/web` 均退出 0；输出分别为 `.autome/output/r21-lint.txt`、`.autome/output/r21-typecheck.txt`。`git diff --check` 通过。
- 按指定基线运行 `sh scripts/ci-quick.sh --since ab28bcc32d19a2b3273362fae1ba1f0937d59c2f`。首次 97 步、失败 2 步：add-project 测试初始化构建缺少 `.vite/manifest.json`（当时与本轮验收/变异测试的构建并行，存在共享构建目录冲突）；Web e2e 遭 `spawnSync ps EPERM` 沙箱限制。超预算项目由脚本留给合并前门禁。原始输出 `.autome/output/r21-quick.txt`。随后以同命令串行重试，期间不再启动其它构建或测试。
- 串行重跑退出 1：执行 97 步、失败 1 步，唯一失败为 Web e2e 的 `spawnSync ps EPERM`；add-project 18 用例通过，manifest 缺失未复现。超 120 秒预算的步骤由脚本标记未跑，留给宿主合并前门禁；不把这些步骤或浏览器验收计为通过。输出 `.autome/output/r21-quick-retry.txt`。M-13 的 66 用例验收及 Web lint/typecheck 均通过，故按协议标为待审，由审计独立复验。
