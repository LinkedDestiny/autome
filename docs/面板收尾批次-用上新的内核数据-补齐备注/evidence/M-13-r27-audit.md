# M-13 · 集成复验 · 审计 #19（覆盖提交 9a895b1）

结论：**通过**，里程碑表不动（M-13 保持已完成）。

- 改动范围：只加 D-1 首步的观察屏障（`flowSteps` 新增可选 `holdFirstStep`，仅 feature 第 1 步；默认行为不变），spec 在看到 running 列后放行。直接对应触发原因（running 态被 100 ms 驱动抢先越过而不可观察）。无产品代码改动，rebase 无消失提交。
- 测试 diff：flows.spec.ts 未删改任何断言，只在 running 断言后加放行；e2e-fixture.test.ts 纯新增 3 组用例（含默认屏障不变的 feature/bug 对照），没有削弱既有用例。
- 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' 9a895b1 -- . ':(exclude)docs/…/'` 结果为空。
- 负向对照：把 `holdFirstStep && step === '1'` 改成 `false && …` 后，e2e-fixture 新增 2 个用例失败（6 过 2 败）；已 `git checkout` 还原，复跑 8/8 通过，工作区干净。
- M-13 验收命令：build 后 task-detail-ssr / task-detail-payload / test-groups 3 文件 11 用例全过，与契约预期一致。
- 未做：D-1 浏览器流程本会话无法跑（ps EPERM，已知环境限制），由宿主合并前门禁复验。
