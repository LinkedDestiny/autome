# M-12 实现轮 22

结论：代码已实现，浏览器验收受沙箱阻塞，M-12 保持开放（reopen 不变）。implementation-round 为 22/65。上一轮审计关闭 M-11，没有本里程碑退回检查需要逐字迁入。

## 修改

- D-4 在 goto 前监听 WebSocket 收帧，只收集 event；离线前先确认在线草稿卡片和对应 task.created 帧均出现。
- 恢复后先确认离线卡片，再精确检查 seq、subject、kind 三者匹配目标创建事件，保留无整页刷新断言。
- 删除 live-client 的测试专用 DOM 标记和对应单测；新增已在线连接经历 restarting → connected 后从断开前游标续读的单测，逐条检查事件并验证不重发。

## 命令与结果

1. 改前 `(cd apps/web && npx vitest run test/eventbus.test.ts test/live-client.test.ts)`：31/31（eventbus 22，live-client 9）。原始输出 `.autome/output/m12-baseline.txt`。
2. 设计 M-12 完整验收命令原样执行：cargo build 与 Web build 成功；Playwright globalSetup 的 `spawnSync ps` 报 EPERM，测试体 0 个执行，globalTeardown 同样受限，串联后续命令未执行。原始输出 `.autome/output/m12-acceptance.txt`。不是测试通过或条件跳过。
3. 单独补跑上述 vitest 命令：31/31（eventbus 23，live-client 8），0 失败、0 跳过。相对本轮改前基线总数差 0，来自重连续读 +1 与 DOM 标记单测 -1。输出 `.autome/output/m12-unit.txt`，变异还原后复跑仍 31/31，输出 `.autome/output/m12-restored.txt`。
4. `! grep -rqE "liveSeq|live-seq" apps/web/app apps/web/e2e apps/web/test`：退出 0，无残留。
5. `(cd apps/web && npx playwright test e2e/live.spec.ts --list)`：浏览器用例仍为 2，未增减；执行证据仍欠 2 例。验收集合总数仍为 33（31 单测 + 2 浏览器），相对改前 33 差 0，实际完成 31。

## 负向对照

临时在 eventbus.setCoreStatus 的 core 状态更新前加入 `if (status.state === 'connected' && core.status !== 'connected') initialized = false;`，使恢复像初始化一样跳至 latest_seq，仅破坏恢复路径。执行 `(cd apps/web && npx vitest run test/eventbus.test.ts -t '已在线连接重连')`：1 失败，22 因名称过滤未选中；前置 `[11]` 断言通过，恢复请求期望 after_seq=11，实际为 MAX_SAFE_INTEGER，新测试判红（`.autome/output/m12-negative.txt`）。随后原样还原 eventbus.ts，完整两文件 31/31 判绿，git diff 确认 eventbus.ts 无修改。

浏览器同一变异的「前置成功、卡片出现、目标帧失败」尚未实跑，须在允许 ps 的宿主复验。未以单测结果替代浏览器证据。

## 标待审前四项自审（本轮不标待审）

1. **情形表逐行**：G.3 的监听先于 goto、精确三字段匹配、删除 DOM 标记、重连续读单测均已逐项核对；grep 与单测通过。浏览器正常及变异两种情形因 EPERM 未完成。
2. **失败分支**：两文件现有「续传内核不可用、超时或读盘失败，成功拉取后重做」「失败保留游标」「连接失败仍重试」均通过。拒绝授权不适用，本里程碑不改授权入口。空输入由 eventbus 非法消息检查覆盖。
3. **全部迁移**：既有五种 core 状态、connecting → open → reconnecting → open、stop 的三种时点、reload、续传断开/关闭均随两文件通过；新增 connected → restarting → connected 检查通过，已有 offline → connected 检查通过。真实内核离线横幅与恢复路径待宿主。
4. **输入域边界**：既有零与最大安全整数、重复序号、缺口 1000/1001、超前游标、空页、账本回退检查全部通过。日期、前导零字符串不适用，当前输入域为事件序号及协议消息。

## 环境与清理

没有运行 ci-local、ci-quick 或项目全量门禁。Playwright 在 setup.ts 的 start 首行 leftoverProcesses 检查处失败，早于 prepare 与 spawn；两个临时根尚未起面板、内核或浏览器，不涉及真实账本。未改变清理检查或添加跳过。浏览器正常与负向证据交宿主复验后才能标待审。

## 直接相关测试与最终检查

- 按设计执行 `(cd apps/web && npx vitest related --run app/lib/live-client.ts)`：选中 47/139 文件、329 个用例，321 通过、2 失败、6 因 beforeAll 失败而未执行；文件结果 43 通过、4 失败。原始输出 `.autome/output/m12-related.txt`。task-timeline-real 的进程清理检查为 ps EPERM；task-live-real / task-log-live 在监听探测失败后清理也报 ps EPERM；shell-live 为 listen EPERM，并记录 EMFILE watch 错误。没有新增条件跳过。
- 相关集合没有上一轮同命令实跑基线，不伪报差值；按本轮文件改动推算，相关集合只删除 live-client 的 1 例（330 → 329），新增 eventbus 用例不依赖 live-client，不在 related 集合中。此为代码计数推算，非改前实跑。上文验收两文件的 31 → 31 是实跑基线。
- 相关夹具的 teardown 即使 scanError 仍先尝试 stopServices、task.stop、core.stop，之后才抛错；已执行清理，但沙箱不允许最终进程表核验，不声称独立证实零残留。浏览器夹具则确定在 spawn 前失败。
- `git diff --check` 通过；eventbus.ts 变异已还原；未修改协议、其它里程碑或其它 worktree。
