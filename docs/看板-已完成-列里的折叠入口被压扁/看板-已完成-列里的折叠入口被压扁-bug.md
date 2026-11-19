# 看板「已完成」列里的折叠入口被压扁

status: 已定位
fix-round: 2/3
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

本轮结论：打回。静态复核未发现产品修复或测试修正缺陷，但独立浏览器复验在夹具初始化阶段受当前沙箱权限阻断，尚未取得本轮复现通过证据。按本轮测试失败交回要求处理；不是因为未跑全量门禁，也不声明回归测试有缺陷。

- 复核范围：定位提交 `e03c372544ee1d23f9dd007cc1eaac3f114af478` 至当前 `69a3185e2ea2fd4385532d6a4a4441a35ac9ddba`；分别检查修复提交 `c5c2f239` 和测试修正提交 `69a3185e`。宿主生成的 `d2e0e28c` 简报不属于产品修复。
- 规范复核：未发现违规。修复提交未修改回归测试；随后测试修正与文档所述共享夹具污染一致。用例名、1512×420 视口、展开操作、溢出断言、按钮数量及高度断言均保留，没有删除、跳过或弱化断言；`layout.spec.ts` 的 `toHaveCount(30)` 原样保留。
- 需求与根因复核：`app.css:102` 的纵向 flex 容器和 `:335` 的固定高度按钮支持所述根因。公共 `.btn` 增加 `flex-shrink: 0` 直接阻止压缩；铃铛按钮同时带 `btn btn-sm`，默认 grow=0、basis=auto 配合公共 shrink=0 保留原 `flex: none` 效果。产品差异仅此两处，范围合理。`fixture.ts:35–37` 已创建 30 个已取消任务，删除额外创建的 3 个任务能够消除污染，并保留制造溢出的输入。
- 独立复现：原命令 `cd apps/web && npx playwright test e2e/board-fold.spec.ts` 退出码 1。`process-scan.ts:5` 调用 `ps -axo pid=,ppid=,command=` 时发生 `spawnSync ps EPERM`，globalSetup 失败，1 个用例未执行；清理同样被权限阻断。尚未进入页面或高度断言，不能记作复现通过或产品失败。日志：`.autome/output/independent-review-1/reproduction.log`。
- 相关浏览器子集：`cd apps/web && npx playwright test e2e/board-fold.spec.ts e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '折叠入口|已取消展开|1512×944 项目$|全部已完成|铃铛'`，退出码 1，同一初始化错误；6 个用例全部未执行，包括共享夹具污染检查与铃铛布局检查。日志：`.autome/output/independent-review-1/related-e2e.log`。
- 相关单测：`cd apps/web && npx vitest run test/styles.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/bell-view.test.ts test/board.test.ts test/board-cancelled.test.ts`，退出码 0，6 个文件、79 个用例全部通过；本轮未增删用例。该结果不能替代真实浏览器几何检查。日志：`.autome/output/independent-review-1/related-unit.log`。
- 验证边界：宿主提供的复现加快检退出码 0 不作为本轮独立复验结果，亦不等于项目全量门禁通过；本轮未运行 `sh scripts/ci-local.sh`。浏览器初始化受阻，因此未做负向缺陷注入；未改产品、测试或格式化文件。
- 后续处理：由宿主/修复轮在允许进程检查及浏览器夹具运行的环境，重跑上述复现命令和相关浏览器子集，取得真实执行结果后再交独立复核。无需据此修改 CSS、跳过进程检查或弱化断言；本轮只提交复核文档，保持状态、fix-round、复现命令及根因不变。

## 修复说明（第 2 轮）

复核打回的原因是沙箱阻断浏览器夹具，并非产品问题；本轮产品代码未再改动（`.btn` 的 `flex-shrink: 0`、删除 `.bell-footer .btn-sm` 补丁均已在位）。本轮在可运行浏览器的环境里取得真实执行结果：

- 复现命令 `cd apps/web && npx playwright test e2e/board-fold.spec.ts`：1 passed，退出码 0。
- 相关子集（board-fold、layout 的铃铛/项目/全部已完成/已取消展开、overlay-layout 铃铛）：6 个用例 5 通过；`layout.spec.ts` 的 `toHaveCount(30)` 与铃铛布局通过。
- 失败的 `overlay-layout.spec.ts:73`「铃铛面板」（通知卡高度 98.97 > 96）与本修复无关：把 `app.css` 还原到定位提交 `e03c372` 的版本单跑，同样失败于 98.97，已恢复修复后的 CSS。该用例不在本缺陷范围内，未改动。
