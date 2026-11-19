# 看板「已完成」列里的折叠入口被压扁

status: 已定位
fix-round: 4/4
复现命令: cd apps/web && npx playwright test e2e/board-fold.spec.ts
根因: `.btn`（app.css 第 336 行）只写固定 height、未禁止 flex 收缩，放进内容溢出的纵向弹性容器 `.board-column-body`（第 103 行）时被压到文字行高
复核: 通过

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

### 第 4 轮独立复核（当前结论）

结论：通过。对受审 HEAD `315cb0981538786abbc5eba2b694763e207e1ee0` 的静态复核未发现产品修复缺陷、无关扩展或回归断言弱化；当前 HEAD 的宿主复现通过、复现加快检退出码为 0。本轮独立单测通过，浏览器复验受沙箱进程检查权限阻断，按操作者备注记为验证缺口，不作为打回理由。此结论不表示项目全量门禁通过。

- 差异范围：核对定位提交 `e03c372544ee1d23f9dd007cc1eaac3f114af478` 至受审 HEAD 的差异，并分别核对产品修复 `c5c2f239` 与测试修正 `69a3185e`。产品仅在 `apps/web/app/styles/app.css` 的公共 `.btn` 增加 `flex-shrink: 0`、删除 `.bell-footer .btn-sm { flex: none; }`；产品修复提交未改测试。`69a3185e` 至受审 HEAD 的 `apps/web` 差异为空，第 4 轮仅有任务文档与宿主简报变化。原始差异：`.autome/output/independent-review-4/diff.log`。
- 根因成立、范围最小：`app.css:102` 的 `.board-column-body` 是可溢出的纵向 flex 容器；`app/routes/projects.$id.tsx:71` 的折叠按钮是其直接子元素，固定 height 不阻止默认的 flex 收缩。`app.css:335` 禁止按钮收缩直接修复该原因。铃铛底部按钮仍为 `btn btn-sm`（`app/components/bell.tsx:46`），公共 shrink=0 配合默认 grow=0、basis=auto 保留被删局部 `flex: none` 的效果，未发现相关行为改变。
- 回归完整性：`69a3185e` 移除新建并取消 3 个任务及相关 import，改用夹具自带的 30 个已取消任务，与“回归测试缺陷”记载的共享数据污染一致。用例名、1512×420 视口、展开操作、溢出断言、按钮数量下限及 30±1 或 38±1 高度断言均保留，无新增跳过标记；`layout.spec.ts:169` 的 30 个任务断言未改。实际 DOM 中折叠入口仍为可见 `.btn`，测试先确认溢出再量几何尺寸，对本回归有意义。沿用定位轮及测试修正轮记录的 21px 负向证据，本轮未注入缺陷，未重新声明测试有缺陷。
- 当前 HEAD 的宿主证据：已读取 `/Users/dannie/project/autome-v2/.autome/output/gates/T-136/fix_gate-315cb0981538.log`、同名 `.exit`、`.kind`、`.report`。日志记录折叠回归 1 passed（用例 927ms），`.exit` 为 `exit_code=0`，结束时间 `2026-11-19T06:46:25Z`；`.kind` 确认为复现加 quick。快检列出 `layout.spec.ts › 1512×944 总览铃铛` 和 `overlay-layout.spec.ts › 1512×944 浮层排布 · 铃铛面板`。精确区分执行来源：本次实跑折叠回归；Format、Clippy、Web lint、Web typecheck 四步按输入指纹沿用 `69a3185e2ea2` 的成功结果（执行 0 步、沿用 4 步、失败 0 步）。铃铛用例清单不单独视为本次重新执行这两项的证明，整套通过信息按操作者提供的宿主证据记录。未把旧 HEAD 的日志冒充本轮实跑，也未把 quick 当作全量门禁。
- 独立复现：执行原命令 `cd apps/web && npx playwright test e2e/board-fold.spec.ts`，退出码 1，`test/fixtures/process-scan.ts:5` 的 `ps -axo pid=,ppid=,command=` 报 `spawnSync ps EPERM`，globalSetup/teardown 受阻，未进入页面断言。日志：`.autome/output/independent-review-4/reproduction.log`，退出码见同名 `.exit`。
- 相关浏览器子集：执行 `cd apps/web && npx playwright test e2e/board-fold.spec.ts e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '折叠入口|已取消展开|1512×944 项目$|全部已完成|铃铛'`，退出码 1，同一沙箱初始化错误，未取得浏览器断言结果。日志：`.autome/output/independent-review-4/related-e2e.log`，退出码见同名 `.exit`。此项与原命令均为验证缺口，不认定为产品或回归测试失败。
- 相关单测：执行 `cd apps/web && npx vitest run test/styles.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/bell-view.test.ts test/board.test.ts test/board-cancelled.test.ts`，退出码 0，6 个文件、79 个用例通过。日志：`.autome/output/independent-review-4/related-unit.log`，退出码见同名 `.exit`；单测不替代浏览器高度验收。
- 边界与遗留：未运行 `sh scripts/ci-local.sh` 或重跑快检；全量门禁仍由宿主在合并前运行。铃铛单跑 98.97 > 96 按操作者备注作为 T-123 运行顺序/夹具状态遗留事项，由操作者另行处理，本轮未将其归因于本修复、未重跑基线对照、未要求更改该用例。本轮仅改本 Bug 文档的复核字段与复核意见，保持 status、fix-round、复现命令、根因与 repos 不变；不合并、不推送、不启动后续会话。

#### 操作者备注核对

- 备注 #1 / 复核要求 1：照做。沙箱浏览器阻塞已明确列为验证缺口，结论依据静态复核及当前 HEAD 宿主证据，不再仅因 EPERM 打回。
- 备注 #1 / 复核要求 2：照做。HEAD 已变化，已读取并引用 `fix_gate-315cb0981538.log` 与 `.exit`，同时核对执行种类和沿用结果，未只引用 `fb526afec993` 的旧日志。
- 备注 #1 / 复核要求 3：照做。保留已有 21px 负向证据，并核对测试修正未弱化高度断言；不重复缺陷注入。
- 备注 #1 / 复核要求 4：照做。铃铛单跑问题保留为操作者另行处理的遗留事项，不作为本任务打回理由。
- 备注 #1 / 修复轮要求：已核实照做。第 4 轮“修复说明”的“操作者备注”小节逐项记录了处理与宿主日志路径；第 4 轮没有产品或测试差异，也未重跑铃铛基线对照。没有遗漏项，无需退回修复轮。

### 第 3 轮独立复核（历史记录）

结论：打回。静态复核未发现产品修复缺陷、无关扩展或回归断言弱化；但本轮独立复现及相关浏览器子集均因沙箱禁止进程检查而退出 1，未取得页面断言的独立执行证据。按本轮测试失败交回要求记录环境阻塞，不将其认定为产品失败，也不因未跑全量门禁打回。

- 复核范围：定位提交 `e03c372544ee1d23f9dd007cc1eaac3f114af478` 至受审 HEAD `fb526afec99324f3c0efbe43b7bb71c52d24c3ab`。产品修复在 `c5c2f239`，回归测试修正在 `69a3185e`；后者至受审 HEAD 的 `apps/web` 差异为空，第 2、3 轮均未再改产品或测试。完整差异记录：`.autome/output/independent-review-3/diff.log`。
- 根因与最小范围：`app.css:102` 的纵向 flex 容器内容溢出时，只有 height、默认 shrink=1 的按钮可被压缩；`app.css:335` 增加 `flex-shrink: 0` 直接消除该原因。产品差异仅公共按钮增加这一属性、删除铃铛局部 `flex: none` 两处。`bell.tsx:46` 按钮仍为 `btn btn-sm`，默认 grow=0、basis=auto 加公共 shrink=0 保留原局部规则的效果，未发现需扩大修复范围的代码证据。
- 回归测试完整性：产品修复提交未改测试。`69a3185e` 只移除额外新建并取消 3 个任务及相关 import，改用 `e2e/harness/fixture.ts:35–38` 自带的 30 个已取消任务，与已登记的共享夹具污染缺陷吻合。用例名、1512×420 视口、展开操作、溢出断言、按钮数量下限及 30±1 或 38±1 高度断言均保留，未新增跳过标记；`layout.spec.ts:169` 的 `toHaveCount(30)` 未改。测试先确认真实溢出，再检查浏览器几何尺寸，针对所述回归有意义。本轮因初始化受阻，未重新取得修复前失败的负向证据，未注入临时缺陷；不新增“回归测试有缺陷”声明。
- 独立复现：`cd apps/web && npx playwright test e2e/board-fold.spec.ts`，退出码 1。`test/fixtures/process-scan.ts:5` 执行 `ps -axo pid=,ppid=,command=` 发生 `spawnSync ps EPERM`，globalSetup 与 teardown 均受阻，1 个用例未执行。`e2e/harness/setup.ts:36` 在 prepare 和服务启动前即调用该检查，尚未进入页面或高度断言。日志：`.autome/output/independent-review-3/reproduction.log`。
- 相关浏览器子集：`cd apps/web && npx playwright test e2e/board-fold.spec.ts e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '折叠入口|已取消展开|1512×944 项目$|全部已完成|铃铛'`，退出码 1，同一初始化权限错误，6 个用例全部未执行。日志：`.autome/output/independent-review-3/related-e2e.log`。
- 相关单测：`cd apps/web && npx vitest run test/styles.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/bell-view.test.ts test/board.test.ts test/board-cancelled.test.ts`，退出码 0，6 个文件、79 个用例通过。日志：`.autome/output/independent-review-3/related-unit.log`。该结果不能替代浏览器高度验收。
- 铃铛既有失败：本轮读取了修复轮的 `.autome/output/r3.log` 与 `.autome/output/r3-base.log`；前者记录折叠用例通过、铃铛失败，后者记录铃铛单跑失败，两者均为 `overlay-layout.spec.ts:91` 的 `Expected <= 96, Received 98.96875`。日志与修复说明一致，但日志本身没有绑定执行时 CSS 哈希；本轮也未能独立重跑基线，因此不能把“基线同样失败、与修复无关”升级为本轮独立确认的结果。保留该问题与证据交回，不修改原测试。
- 门禁与后续：宿主报告的复现加快检退出码 0 不等于全量门禁通过，也不替代本轮独立复验。遵照要求未运行 `sh scripts/ci-local.sh`、未重跑快检；保持状态、fix-round、复现命令、根因及 repos 不变。本轮只更新本 Bug 文档，产品和测试无改动。需要 Autome 在允许进程检查和浏览器夹具运行的环境安排独立复验，并绑定 CSS 内容核验铃铛基线对照；无需为沙箱错误修改 CSS、绕过进程检查或弱化断言。不合并、不推送、不启动后续会话。

### 第 2 轮独立复核（历史记录）

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

## 修复说明（第 4 轮）

本轮未改任何产品代码或测试（`.btn` 的 `flex-shrink: 0`、删除 `.bell-footer .btn-sm` 补丁已在位）。

### 操作者备注

- 备注 #1：照做。三轮打回的原因是复核沙箱里 `ps` 被拒（`spawnSync ps EPERM`），浏览器夹具起不来，属环境限制，不是产品或测试问题；本轮不改代码，只记录处理。
  - 宿主门禁证据（本机无沙箱，对 HEAD `fb526afec993` 实跑）：`/Users/dannie/project/autome-v2/.autome/output/gates/T-136/fix_gate-fb526afec993.log` 与同名 `.exit`。其中复现命令 `npx playwright test e2e/board-fold.spec.ts` 1 passed，随后快检退出码 0，用例清单包含 `overlay-layout.spec.ts › 浮层排布 · 铃铛面板` 与 `layout.spec.ts › 总览铃铛`。本轮 HEAD 变化后以本轮那份为准。
  - 负向证据已有：定位轮在未修代码上实测折叠入口约 21px；回归测试修正轮临时去掉 `flex-shrink: 0` 后仍失败于「按钮高度 21px 应为 30 或 38」。
  - 铃铛用例单跑失败（98.97 > 96）是遗留事项：不在本任务范围，整套运行里通过，与 `.btn` 是否可收缩无关，由操作者另行处理；本轮未重跑基线对照。
  - 沙箱跑不了浏览器用例属验证缺口，不构成打回理由。
