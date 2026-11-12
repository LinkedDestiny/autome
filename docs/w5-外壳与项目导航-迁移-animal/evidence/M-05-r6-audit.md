# M-05 r6 审计复验

结论：通过。

## 负向对照（apps/web/app/components/palette-model.ts，逐个注入、跑 test/palette.test.ts、还原）
- mac 上也接受 ctrlKey → 1 红（快捷键用例）
- down/up 不跳过禁用项 → 2 红（迁移用例、写门禁用例）
- filterCandidates 只匹配标题 → 1 红（过滤用例）
- 新建候选不读写判定 → 2 红（写门禁、SSR 语义）
全部还原，`git status --porcelain` 为空。

## 验收命令
- `npm test -- test/palette.test.ts test/shell.test.ts`：2 文件 11/11。
- Web lint 0、typecheck 0、build 0。
- Web 全量：43 文件 364 用例全过，skipped 0。基线 M-04 实现轮运行时 357（含因监听 EPERM 的失败/跳过），+7 = 364，与新增 7 条吻合；设计里的 350 是静态枚举数，运行时多出的是参数化展开（M-04 已澄清）。
- Desktop：165/165，skipped 0；本里程碑未改 apps/desktop，设计里的 104 是静态数，同理。
- 宿主门禁 05a7e11acd：全部通过（Desktop、Web lint/typecheck/test/build）。
- 全部检查后 `git status --porcelain` 为空。

## 说明
filterCandidates 另外匹配副标题（含状态名），比设计多一个匹配字段，不违反 R-03，记入 Backlog。
人工项 H-03/H-05/H-07 已在清单中，由用户合并前确认。
