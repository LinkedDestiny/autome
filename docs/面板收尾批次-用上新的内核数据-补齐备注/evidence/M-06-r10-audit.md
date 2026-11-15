# M-06 · 审计 #7 独立复验

## 验收命令
- node --test ci-summary + ci-reuse：26 通过（实现轮同数）。
- cargo build、npm run build：通过。
- Playwright live+layout：14 通过（live 2、layout 12），本环境浏览器可用。
- vitest w9-cleanup + live-client：12 通过。

## 负向对照（均已 git checkout 还原）
1. summary.mjs 删 Doc-tests 分支 → ci-summary 新用例失败（7 过 1 败）。
2. ci-reuse 夹具删去 `prep 'Build core'` → ⑪ 失败（新断言 build>=0；旧断言 -1<n 会放过）。
3. eventbus drain 对第二个 task.created 不分发（仅恢复路径）→ D-4 失败：卡片出现但 `data-live-seq` 218，期望 ≥219。首次用「内容含 offline-draft」的注入未生效（载荷不含标题），已改为按序计数，属注入写法问题而非用例缺陷。
4. app.css 列体 `overflow-y: visible` → layout「项目」失败，Received "visible"。

## D-5（ENOTEMPTY）
- 做法：临时把 test/ 下 11 个文件的 maxRetries 5→0（诊断后 git checkout 还原），`npx vitest run --project kernel` 循环；日志 `.autome/output/d5/before-*.txt`。
- 结果：前 8 次全过，第 9 次 1 个用例失败：`ENOTEMPTY rmdir …/w9-M8Cshv/home/.claude`（project-confirm-real）。随后停止循环（已复现，余下无必要）。
- 解读：不带重试会失败，所以重试有存在理由；失败条目是假 HOME 下的 `.claude`，指向写该目录的后台进程（非 state/、非 sessions），H2 的 sessions 假设对此次不成立；H4 面板日志不适用。未做 lsof/进程归因，未证实根因，故保留重试（设计允许的结论）。未做「改后 20 次」。
- 本轮未改任何项目文件；工作区干净。
