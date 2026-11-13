# 审计（集成复验）· M-16 · 实现 #22 的提交 1f666e5

结论：**通过**，里程碑表不动（M-16 保持已完成）。

1. 改动范围：`git diff 462e349..1f666e5` 在文档目录外只有 `apps/web/test/eventbus-live.test.ts`（+5/-2）：`rmSync(home,{recursive,force})` 换成 `await rm(..., {maxRetries:5, retryDelay:100})`。只针对门禁的 afterAll ENOTEMPTY；仍先停子进程再删目录，持续失败仍会让钩子失败。没有产品代码改动；rebase 丢失提交 0 个。
2. 测试 diff：七个用例的断言、等待上限、跳过条件都没动，没有削弱。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' 1f666e5 -- . ':(exclude)docs/内核补齐批次-2026-11-13-4/'` 为空。
4. 本轮自己跑：`npm run test --prefix apps/web -- test/eventbus-live.test.ts` → 1 文件 7 用例全过（实现轮在沙箱里被 listen EPERM 阻断，这里拿到了真实绿）。M-16 验收命令 `cargo test -p automed -- protocol_integrity_ the_version_in_force` → 2 + 6 = 8 个通过，与契约一致；本次改动不涉及 Rust，M-16 的负向对照对它无意义，未重做。
5. 局限：瞬时 ENOTEMPTY 无法按需复现，无法给出该故障的红绿；重试是对已观测错误的有界缓解，宿主合并前门禁复跑为准。
