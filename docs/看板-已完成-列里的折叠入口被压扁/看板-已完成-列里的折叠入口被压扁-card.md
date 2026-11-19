# 看板「已完成」列里的折叠入口被压扁：统一按钮在可滚动的纵向排布里会被挤到 21px

## 现象

T-124 合并后（提交 `15e05924`），操作者在已安装的面板上用真实账本、1512×944 实测：autome-v2 项目看板「已完成」列底部的两个折叠入口「更早 · 67 个 →」「已取消（24）」带着统一的小号文字按钮类名（`btn btn-text btn-sm`，应为 30px），实际高度只有 **22px 和 21px**。同一页面上 offchat、voice-schedule 两个任务少的项目，同样的入口是 30px。

根因（先修根因，不要只给折叠入口打补丁）：`apps/web/app/styles/app.css` 第 103 行的 `.board-column-body` 是纵向弹性排布、内部可滚动。列里的内容超过列高时，浏览器先压缩允许收缩的子元素再出滚动条；`.btn`（第 336 行）只写了固定 `height`，没有禁止收缩，于是被压到文字行高。任务卡不受影响是因为它们的最小高度就是内容高度。也就是说，**统一按钮放进任何一个内容溢出的纵向弹性容器都会被压扁**，不是折叠入口独有的问题。第 64 行的 `.bell-footer .btn-sm { flex: none; }` 就是此前为同一个根因在铃铛面板里打的局部补丁。

T-124 的「全站清扫」用例（`apps/web/e2e/action-controls.spec.ts`）没拦住，是因为夹具里「已完成」列的内容不够多，列没有溢出。

## 复现

仓库根目录执行，面板用隔离夹具，不碰真实账本：

```sh
sh scripts/ci/install-node.sh apps/web
npm run build --prefix apps/web
cd apps/web && npx playwright test e2e/board-fold.spec.ts
```

`e2e/board-fold.spec.ts` 是定位轮要新建的回归测试文件（见「线索」）。

## 期望

0. 根因：统一按钮在任何排布里都不会低于它的标称高度。在 `.btn` 这条公共规则上禁止被弹性容器压缩（`.btn` 已是 `white-space: nowrap`，横向不受影响），不要给 `.board-fold` 或各页面单独写；改完后删除第 64 行 `.bell-footer .btn-sm { flex: none; }` 这条已经多余的局部补丁，铃铛面板页脚按钮仍为 30px。
1. 看板「已完成」列内容溢出、列内出现滚动时，「更早 · N 个 →」「收起」「已取消（N）」「收起已取消」这些折叠入口高度都是 30px（±1），列内滚动照常，1512×944 下不出现整页纵向滚动。
2. 其它页面的按钮高度不变（只能是 30 或 38px），现有浏览器用例全部继续通过。

## 线索

- **回归测试写进新文件** `apps/web/e2e/board-fold.spec.ts`，**不要改** `e2e/action-controls.spec.ts`、`e2e/layout.spec.ts`、`e2e/overlay-layout.spec.ts`、`e2e/env-rows.spec.ts`，也不要把断言补进已有测试文件（内核的「修正回归测试」流程只对新建的测试文件可用）。复用 `./harness/browser.ts` 与 `./harness/fixture.ts`。
- 夹具里造出折叠入口：新建一个任务再用命令行取消它，「已完成」列底部就会出现「已取消（1）」（`data-testid` 为看板的已取消开关，见 `app/lib/testids.ts` 的 `boardCancelledToggle`）。
- 让列溢出有两种办法，任选其一并在用例注释里写明：多取消几个任务后点开「已取消」把卡片展开；或把视口高度调小（例如 1512×420）。
- **先断言确实到达了溢出场景，再量高度**：`.board-column[data-column="done"] .board-column-body` 的 `scrollHeight > clientHeight`，否则用例是空过的（T-123 的环境页用例就是量早了，一直空过）。然后断言该列里每个可见的 `.btn` 高度为 30±1 或 38±1px。
- 先红后绿：新用例在未修的代码上必须因高度约 21px 而失败，把实测值写进定位说明；失败原因必须是高度断言，不是环境问题。整套 `npx playwright test` 一起跑一遍确认不互相影响。
- 只改 `apps/web`，不动内核。硬规则不变：全中文、1512×944 不出现整页纵向滚动、保持动森风格令牌。
- 复核轮在沙箱里跑不了浏览器用例，这是已知限制，记为验证缺口即可；浏览器用例由宿主门禁实跑。
