# 审计 #6 · M-06 · 实现缺陷（退回，领域：Web 测试分组配置）

失败命令：`cd apps/web && npm run test`（即验收契约里的全量 Web 测试与交替基准所依赖的命令）。
输出尾部：`Error: Projects "kernel" and "unit" have different 'maxWorkers' but same 'sequence.groupOrder'. Provide unique 'sequence.groupOrder' for them.` → `Test Files  no tests`，退出非 0。vitest 5.1.0 拒绝这份配置，三个 project 都是 `groupOrder: 0` 而 `kernel` 设了 `maxWorkers: 4`、`exclusive` 设了 `fileParallelism: false`。
违反：设计 D.1「同一 `groupOrder`，三组同时开跑」落不了地；验收预期「全量 Web 测试 111 / 840、5 次全绿」「墙钟 ≤ 50%」全部无法取数。

之所以验收契约里的四文件命令全绿：只选单个 project 的文件时，分组冲突不触发；只有同时命中多个 project 才报错。`test-groups.test.ts` 只读配置对象，没有真实启动 vitest，因此也放过。

其余复验：四文件命令 30 个全绿（4+18+7+1；先 `cargo build` 刷新过期的 target/debug，之前 eventbus-live 的 3 个失败是二进制比账本旧）。负向对照：eventbus-live 放进 `unit` → 用例 2 红；删 `globalSetup` 且移走 `build/` → `test-groups` 用例 3 红、`gateway-http` 11 个红（缺 build/server/index.js）；均已还原。
未取数：交替基准与 5 次连跑（完整命令无法启动）；ENOTEMPTY 结论待实现轮。

修复提示（不在审计轮落地）：让各 project 满足 vitest 对 groupOrder 的约束，或改成不触发该检查的并行方式，再用实测确认三组仍能同时开跑；`test-groups.test.ts` 的用例 3 需改为断言当前断言之外，再加一个真实启动完整配置并要求出现用例的回归。
判别检查：`.autome/output/m06a/full-run-check.sh`（当前红，修复后应绿）；下一轮实现轮搬进项目测试体系。
