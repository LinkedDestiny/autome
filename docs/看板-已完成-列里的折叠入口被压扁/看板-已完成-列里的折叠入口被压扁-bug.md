# 看板「已完成」列里的折叠入口被压扁

status: 已定位
fix-round: 0/3
复现命令: cd apps/web && npx playwright test e2e/board-fold.spec.ts
根因: `.btn`（app.css 第 336 行）只写固定 height、未禁止 flex 收缩，放进内容溢出的纵向弹性容器 `.board-column-body`（第 103 行）时被压到文字行高
复核: 待复核

## 现象

看板「已完成」列底部的折叠入口（`btn btn-text btn-sm`，应为 30px）在列内容溢出时被压到 21–22px；内容少的项目为 30px。

## 复现

前置：`sh scripts/ci/install-node.sh apps/web`、`cargo build`（夹具要 target/debug/autome）、`npm run build --prefix apps/web`。

命令：`cd apps/web && npx playwright test e2e/board-fold.spec.ts`

新用例 `e2e/board-fold.spec.ts`：取消 3 个任务，点开「已取消」展开卡片，视口 1512×420；先断言 `.board-column[data-column="done"] .board-column-body` 的 scrollHeight > clientHeight（确认到达溢出场景），再逐个量该列可见 `.btn` 高度须为 30±1 或 38±1。

未修代码上的结果：溢出断言通过，随后失败于 `按钮高度 21px 应为 30 或 38`（实测 21px，失败原因是高度断言）。

格式化 / lint：对新文件跑 `npx prettier --write e2e/board-fold.spec.ts`，再跑项目 quick 检查 `sh scripts/ci-quick.sh`，全部通过；格式化后重跑复现命令，仍以 21px 高度断言失败。

## 根因分析

`.board-column-body` 是 `display:flex; flex-direction:column; overflow-y:auto`。内容超过列高时，浏览器先压缩允许收缩的子元素再出滚动条。`.btn` 只有固定 `height`，flex-shrink 默认为 1，于是被压到文字行高（约 21px）；任务卡最小高度即内容高度，不受影响。统一按钮放进任何溢出的纵向弹性容器都会如此，第 64 行 `.bell-footer .btn-sm { flex: none; }` 是同一根因的局部补丁。

修法方向：在 `.btn` 公共规则加 `flex-shrink: 0`（或 `flex: none`），并删除第 64 行的局部补丁。

## 修复说明

（待修复轮填写）

## 复核意见

（待复核轮填写）
