# 看板「已完成」列里的折叠入口被压扁

status: 已定位
fix-round: 1/3
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

（待复核轮填写）
