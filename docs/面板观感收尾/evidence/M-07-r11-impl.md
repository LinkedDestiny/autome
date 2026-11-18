# M-07 · 实现 #11 · 宿主布局失败修复轮，保持待审

## 范围与修复

起点 `16afe9db0f9b13c089d446e3b7fa145be4adbf8a`。本轮不推进新里程碑，M-07 保持待审，implementation-round 更新为 11/65。

宿主失败的是 `apps/web/e2e/layout.spec.ts`「1512×944 需求详情展开日志」中的原检查：`expect(box.y + box.height).toBeLessThanOrEqual(bounds!.y + bounds!.height);`，实际 946.53125 > 920。失败在展开日志之前，违反总-Q1 首屏十条完整可见契约。该检查已在项目测试体系中，本轮逐字保留，未降低条数、移动滚动位置或放松断言。

上一轮只收紧行与分组间距，忽略详情顶部阶段区、统计卡组、提示区与区块间距共同占用可视高度。源码显示阶段区可占页面 24%、统计组上下 margin 合计 20px、提示内外纵向留白合计 48px、页面 gap 12px；已有截图 `.autome/output/m06-detail.png` 也显示上方占据大量空间，但它是旧图，不能当成本轮截图。

本轮仅对 ≥1100×800 的桌面详情收紧共同高度预算：gap 8px，阶段区最高 96px，统计组 margin 0，提示操作区最高 48px，提示 margin 0 / padding 8px 12px。阶段与提示操作区保留现有内部滚动；小视口原规则不变。未实现 M-08 的里程碑条等新内容。该调整是否足够满足真实几何边界，尚待宿主运行确认，不宣称本地已验证修复成功。

原 e2e 在几何断言前新增 `timeline-geometry` JSON 附件及 `.autome/output/m07-collapsed-detail.png` 截图，记录顶部区块、分组与全部行的实际矩形。此前截图位于断言之后，失败时无法留下折叠态截图；本轮修复这个诊断缺口，没有新增或删掉浏览器用例。

## 红绿与验收

1. 修复前原样执行 `(cd apps/web && npx playwright test e2e/layout.spec.ts -g '1512×944 需求详情展开日志')`：globalSetup 报 `spawnSync ps EPERM`，实际执行 0 个用例。日志 `.autome/output/m07-r11-red.txt`。本地未获得产品红复现；产品红证据是宿主原输出。
2. 新增桌面高度预算守卫、CSS 未改时执行 `(cd apps/web && npx vitest run test/task-timeline.test.ts)`：37 个，36 通过、1 失败（新增守卫），日志 `.autome/output/m07-r11-density-red.txt`。
3. 修改后执行原 M-07 验收命令 `(cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts)`：86/86 通过，0 跳过，6 文件。相对上一轮同命令 85 个 **+1**，来自新增高度预算守卫；原有检查全部保留。日志 `.autome/output/m07-r11-accept.txt`。
4. 修复后再次原样执行第 1 条浏览器命令：仍为 `ps EPERM`，0 个实际执行，较本轮修复前差 0，较宿主该子集的 1 个少 1 个（环境阻塞）。日志 `.autome/output/m07-r11-browser-after.txt`。静态红绿不能替代原几何断言的绿结果。

## 标待审之前的自审清单（本轮保持待审）

1. **情形表逐行**：D-Q1 会话、门禁、用户、系统四类共用原紧凑行，完整文字与结果字段由 task-timeline 验证；D-Q2 文字与日志独立展开、运行中日志默认打开的结构用例通过。总-Q1 实际首屏十条几何验收未执行成功，已保留原断言与前置诊断附件，待宿主确认。
2. **失败分支**：timeline-failure-path 的 6 个既有检查通过（门禁失败/超时、准备失败、空或缺失路径、中文路径及原始字段）；任务时间线既有读取失败、截断与崩溃检查通过。本轮纯高度布局，无新增授权或外部服务路径，拒绝授权和服务不可达不适用。
3. **全部迁移**：验收原集合覆盖运行到结束/停止/超时/崩溃、门禁排队/运行/结果、筛选和退回。未增加状态；文字折叠→展开→折叠与日志展开的原 e2e 未改，本地执行受阻。阶段和提示内部滚动的 CSS 守卫通过，实际滚动与小视口视觉未实测。
4. **输入域边界**：空列表、单条、长摘要、无失败详情、earlier=0/1、60 会话与新增五会话 SSR 上限检查通过。前导零、日期、跨年不适用：本轮未改输入解析。桌面规则限定 ≥1100×800；断点两侧实际浏览器几何未实测，不以静态测试替代。1512×944 十条下边缘边界交宿主复验。

未运行全量 `sh scripts/ci-local.sh`，未改沙箱清理或监听规则。无新会话、无切分支、无远端推送。

## 指定快检与最终检查

`sh scripts/ci-quick.sh --since 16afe9db0f9b13c089d446e3b7fa145be4adbf8a`：21 步、19 成功、2 失败，57 秒；相关阶段 36 秒（预算 120 秒）。Format、Clippy、Web lint、typecheck、build 通过。日志 `.autome/output/m07-r11-quick.txt`。

相关 Vitest 九次文件运行总计 90 个，89 通过、1 失败、0 跳过；相对上一轮快检 89 个 **+1**，来自本轮高度预算守卫。失败是 shell-live 的开发 SSR 监听 `127.0.0.1` 被 `listen EPERM` 拒绝。Web e2e 因准备/收尾的 `spawnSync ps EPERM` 阻断，35 个未实际执行；相对宿主基线实际执行 35 个少 35 个，测试定义没有减少。各集合有重叠，不相加当作去重总数。

本轮没有取得指定快检全部通过的结果，也没有取得原布局失败的浏览器绿证据。快检结论由 Autome 提交后重跑确定。`git diff --check` 通过，状态为 11/65、M-07 待审，设计文档保留唯一最新证据指针。
