# M-09 · 审计 #15 复验（对应实现 r16）

结论：**通过**。验收命令全绿；负向对照 3/4 能拦住错误实现；对照 2 原写法不能区分（验证缺口），已改契约并按新写法复验。

## 验收命令
- vitest 三文件 33 条全过（e2e-preflight 3、core-sidecar 19、ci-config 11）。
- `npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts`：16 通过、0 跳过，globalTeardown 残留断言通过。

## 负向对照（均已还原，git 工作区干净，build 已按还原源码重建）
1. 预检去掉 headless shell 判断 → e2e-preflight 第 1 条失败（1 failed | 2 passed）。✔
2. 原写法：去掉看板列体 `data-scroll-region` → layout 11 条仍全过。✘ 不区分：`oneScreen` 只检查 main 及其直接子元素，列体的标记不被读取，且列溢出由 CSS 包住。属验证缺口，产品未错（标记在）。
   改后写法：去掉 `task-timeline.tsx` 的 `data-scroll-region` → manual-items 第 2 条（W7 H-04）失败。✔ 契约已改。
3. `SessionLog` 加 `key={Math.random()}` 重建 → manual-items 第 1 条（H-03）失败。✔
4. 新增 `e2e/zz-skip.spec.ts` 含 `test.skip(` → core-sidecar 守卫「浏览器测试与配置不按条件跳过用例」失败。✔

## 说明
- 前一轮缺陷（`?tab=docs` 断言）已修，实跑通过。
- 无人工项需要移出。输出在 `.autome/output/a15-*.txt`。提交由 Autome 兜底（如被拒）。
