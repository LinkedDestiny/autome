# M-03 r3 审计复验

环境：本机 `listen 127.0.0.1` 可用（实现轮 EPERM 不复现），`target/debug/automed` 已存在。原始输出在 `.autome/output/a3-run*.log`、`a3-neg*.log`。

## 验收命令
`npm test -- test/logtail-live.test.ts test/core-sidecar.test.ts`，退出码 1：`core-sidecar` 18 通过；`logtail-live` 套件失败、3 条 skipped。
错误：`Error: expect.poll() must be called inside a test`，位于 `test/logtail-live.test.ts:66`（beforeAll 内）。同样问题还在第 34 行（beforeAll 经 `startTask` 调用）与第 77 行。预期 21/21、skipped 0，实际 18 通过、3 skipped。

## 判别检查（临时副本，已删除，不进仓库）
把 `test/logtail-live.test.ts` 拷成临时文件，仅把 beforeAll 内三处 `expect.poll` 换成手写循环后运行：
- 第 1 条（3000ms 内收到增量）通过；第 2 条（尾段、`log.end ended`、拼接等于全文、lsof 正向对照）通过。
- 第 3 条失败：`该目录已经是一个项目`——第 3 条再次调用 `startTask()`，对同一 repo 重复 `project.add`。测试自身缺陷，与产品无关。
→ 产品行为在真实内核上符合设计；失败全部来自测试文件本身。

## 负向对照（在上述临时副本上执行，均已还原，`git status --porcelain` 为空）
- `intervalMs` 500→5000：三条全红（第 1 条 5008ms 超时）。
- 读后不 `handle.close()`：第 2 条红（lsof/stats），第 3 条红。
- `main.ts` 不调 `logtail.close()`：因第 3 条本身先因重复 project.add 失败，无法单独判别，需实现轮修好测试后重做。

## 结论
实现缺陷（项目测试不通过，验收命令退出码 1）：退回 `开放`，reopen 1，领域 logtail-live。
实现轮要做：①beforeAll 与 `startTask` 里的 `expect.poll` 改为测试内可用的轮询（手写循环或把 startTask 放进测试体）；②第 3 条复用已有 project，不再重复 `project.add`；③修好后重做 `close()` 不调 logtail.close 的负向对照。判别检查即上面的临时副本（构造方式见此文，副本已删）。
