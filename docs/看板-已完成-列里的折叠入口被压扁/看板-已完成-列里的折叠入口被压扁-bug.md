# 看板「已完成」列里的折叠入口被压扁

status: 已定位
fix-round: 3/3
复现命令: cd apps/web && npx playwright test e2e/board-fold.spec.ts
根因: `.btn`（app.css 第 336 行）只写固定 height、未禁止 flex 收缩，放进内容溢出的纵向弹性容器 `.board-column-body`（第 103 行）时被压到文字行高
复核: 待复核

## 现象

看板「已完成」列底部的折叠入口（`btn btn-text btn-sm`，应为 30px）在列内容溢出时被压到 21–22px；内容少的项目为 30px。

## 复现

前置：`sh scripts/ci/install-node.sh apps/web`、`cargo build`（夹具要 target/debug/autome）、`npm run build --prefix apps/web`。

命令：`cd apps/web && npx playwright test e2e/board-fold.spec.ts`

新用例 `e2e/board-fold.spec.ts`：不新建任务，直接用共享夹具里已有的 30 个已取消任务，点开「已取消」展开卡片，视口 1512×420；先断言 `.board-column[data-column="done"] .board-column-body` 的 scrollHeight > clientHeight（确认到达溢出场景），再逐个量该列可见 `.btn` 高度须为 30±1 或 38±1。

未修代码上的结果：溢出断言通过，随后失败于 `按钮高度 21px 应为 30 或 38`（实测 21px，失败原因是高度断言）。

格式化 / lint：对新文件跑 `npx prettier --write e2e/board-fold.spec.ts`，再跑项目 quick 检查 `sh scripts/ci-quick.sh`，全部通过；格式化后重跑复现命令，仍以 21px 高度断言失败。

## 根因分析

`.board-column-body` 是 `display:flex; flex-direction:column; overflow-y:auto`。内容超过列高时，浏览器先压缩允许收缩的子元素再出滚动条。`.btn` 只有固定 `height`，flex-shrink 默认为 1，于是被压到文字行高（约 21px）；任务卡最小高度即内容高度，不受影响。统一按钮放进任何溢出的纵向弹性容器都会如此，第 64 行 `.bell-footer .btn-sm { flex: none; }` 是同一根因的局部补丁。

修法方向：在 `.btn` 公共规则加 `flex-shrink: 0`（或 `flex: none`），并删除第 64 行的局部补丁。

## 修复说明

`.btn` 公共规则加 `flex-shrink: 0`，并删除 `.bell-footer .btn-sm { flex: none; }` 局部补丁（`apps/web/app/styles/app.css`）。复现命令 `npx playwright test e2e/board-fold.spec.ts` 通过（1 passed）。

## 回归测试缺陷

- 用例：`apps/web/e2e/board-fold.spec.ts` › 已完成列溢出时折叠入口仍保持统一按钮高度
- 缺陷：用例在共享夹具（`state()`）里新建并取消 3 个任务，且不清理；同一轮 e2e 里后跑的 `layout.spec.ts:169` 断言 `board-cancelled` 下任务卡恰为 30，被污染成 33。
- 证据：产品修好后复现命令通过（1 passed）；`layout.spec.ts` 「已取消展开」单跑通过，但 `sh scripts/ci-quick.sh --since ...` 全量 e2e 连续两次失败于 `toHaveCount(30)`，Received 33。
- 建议：定位轮让该用例不污染共享夹具（如用独立项目/夹具，或结束时清理这 3 个任务），或改成相对计数；不要改 layout.spec 的 30。

## 回归测试修正

用例不再新建/取消任务，改用共享夹具自带的 30 个已取消任务制造溢出，不再污染 `layout.spec.ts` 的 `toHaveCount(30)`。验证：`npx prettier --write` 无改动；修好的 CSS 上复现命令通过（1 passed）；临时去掉 `.btn` 的 `flex-shrink: 0` 后仍失败于 `按钮高度 21px 应为 30 或 38`（已恢复 CSS）。

## 复核意见

第 2 轮结论：打回。静态复核未发现根因、产品修复或回归测试修正缺陷，产品改动范围最小；但本轮独立复现及相关浏览器子集仍在夹具初始化阶段被沙箱权限阻断，未取得页面断言的独立执行证据。按本轮测试失败交回要求记录，不把环境阻塞认定为产品失败，也不因未跑全量门禁打回。

- 复核范围：定位提交 `e03c372544ee1d23f9dd007cc1eaac3f114af478` 至本轮受审 HEAD `29c5a6757626e8191a950de5193f3c143c3ba57a`。分别核对产品修复 `c5c2f239`、测试修正 `69a3185e`；后者至 HEAD 的 `apps/web` 差异为空，第 2 轮确实没有再次修改产品或测试。宿主简报不属于产品改动。
- 根因与范围：`app.css:102` 的纵向 flex 容器允许内容溢出；原公共按钮固定 height 但默认 shrink=1，能够被压缩。`app.css:335` 新增 `flex-shrink: 0` 直接阻止压缩。删除铃铛局部 `flex: none` 后，`bell.tsx:46` 的按钮仍有 `btn btn-sm`，默认 grow=0、basis=auto 与公共 shrink=0 保留原效果。产品差异仅上述两处，未发现无关扩展。
- 回归测试完整性：产品修复提交未改测试；`69a3185e` 的夹具输入调整与已登记的污染缺陷相符，只移除额外新建并取消 3 个任务及相关 import，改用 `e2e/harness/fixture.ts:35–38` 自带的 30 个已取消任务。用例名、1512×420 视口、展开操作、溢出断言、按钮数量下限及 30±1 或 38±1 高度断言均保留，未增加跳过标记；`layout.spec.ts` 的 `toHaveCount(30)` 未改。测试先确认溢出再量真实几何尺寸，能够针对所述压缩回归，不是只检查 CSS 字符串。本轮浏览器无法启动，因此没有重新取得修复前失败的负向证据，未做临时缺陷注入。
- 独立复现：`cd apps/web && npx playwright test e2e/board-fold.spec.ts`，退出码 1。`apps/web/test/fixtures/process-scan.ts:5` 执行 `ps -axo pid=,ppid=,command=` 时发生 `spawnSync ps EPERM`，globalSetup 及 teardown 均受阻，1 个用例未执行。尚未进入页面或高度断言，不能记作复现通过或产品失败。原始日志：`.autome/output/independent-review-2/reproduction.log`。
- 相关浏览器子集：`cd apps/web && npx playwright test e2e/board-fold.spec.ts e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '折叠入口|已取消展开|1512×944 项目$|全部已完成|铃铛'`，退出码 1，同样在初始化阶段发生 `spawnSync ps EPERM`，6 个用例全部未执行。原始日志：`.autome/output/independent-review-2/related-e2e.log`。
- 相关单测：`cd apps/web && npx vitest run test/styles.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/bell-view.test.ts test/board.test.ts test/board-cancelled.test.ts`，退出码 0，6 个文件、79 个用例通过。原始日志：`.autome/output/independent-review-2/related-unit.log`。该结果不能替代真实浏览器几何断言。
- 第 2 轮既有失败的处理：修复记录称 `overlay-layout.spec.ts:73` 铃铛面板高度 98.97 > 96，且定位版 CSS 对照同样失败。本轮核实了该测试的 96px 断言及铃铛按钮样式等价关系，但受上述权限限制，未能独立重现该失败或其基线对照；不能把修复轮陈述记成本轮已确认的无关失败，更不能称该相关子集全部通过。保持原测试，交回修复轮保留并核验现有失败证据。
- 门禁边界：宿主提供的复现加快检退出码 0 已知，但不等于全量门禁通过，也不替代本轮独立复验。遵照要求未运行 `sh scripts/ci-local.sh`，未重跑快检、修改产品、测试或格式化文件，未声明回归测试有缺陷。
- 后续：需由 Autome 在允许进程检查及浏览器夹具运行的环境安排独立复验，重跑上述原命令与相关子集，并核验铃铛失败的基线对照。无需为沙箱错误修改 CSS、跳过进程检查或弱化断言。本轮只更新本 Bug 文档，保持状态、fix-round、复现命令与根因不变；不合并、不推送、不启动后续会话。

## 修复说明（第 2 轮）

复核打回的原因是沙箱阻断浏览器夹具，并非产品问题；本轮产品代码未再改动（`.btn` 的 `flex-shrink: 0`、删除 `.bell-footer .btn-sm` 补丁均已在位）。本轮在可运行浏览器的环境里取得真实执行结果：

- 复现命令 `cd apps/web && npx playwright test e2e/board-fold.spec.ts`：1 passed，退出码 0。
- 相关子集（board-fold、layout 的铃铛/项目/全部已完成/已取消展开、overlay-layout 铃铛）：6 个用例 5 通过；`layout.spec.ts` 的 `toHaveCount(30)` 与铃铛布局通过。
- 失败的 `overlay-layout.spec.ts:73`「铃铛面板」（通知卡高度 98.97 > 96）与本修复无关：把 `app.css` 还原到定位提交 `e03c372` 的版本单跑，同样失败于 98.97，已恢复修复后的 CSS。该用例不在本缺陷范围内，未改动。

## 修复说明（第 3 轮）

产品代码未再改动（`.btn` 的 `flex-shrink: 0`、删除 `.bell-footer .btn-sm` 补丁已在位）；回归测试未动。本轮在可运行浏览器的环境里独立取得执行证据：

- 复现命令 `cd apps/web && npx playwright test e2e/board-fold.spec.ts`：通过。
- 相关子集 `--grep '折叠入口|铃铛'`（board-fold + overlay-layout）：1 通过、1 失败。失败为 `overlay-layout.spec.ts:91` 铃铛面板高度 `Expected <= 96, Received 98.96875`。
- 铃铛失败的基线对照：把 `app.css` 临时还原为定位提交 `e03c372` 的版本单跑，同样失败于 98.96875，随后已恢复修复后的 CSS（工作区无多余改动）。故该失败早于本修复、与 `.btn` 收缩无关，不在本缺陷范围，未改动该用例。日志：`.autome/output/r3.log`、`r3-base.log`。
