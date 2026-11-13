# M-06 实现轮 8/30（快检修复轮）

结论：修复宿主快检发现的两条 preserve-caught-error 错误，本地指定快检通过。M-06 原为开放，保持开放；本轮不推进新里程碑，不关闭里程碑。最终快检结果以 Autome 提交后重跑为准。

## 修复与回归

- task-live-real.test.ts:45 与 task-merge-real.test.ts:25 的 setup/cleanup 双失败处理，给 AggregateError 增加 {cause:cleanupError}，保留 errors 中的 setup 和 cleanup 两个原始异常，不改变清理流程。
- 检查 apps/web/test 全部 AggregateError：其它五处已设置 cause，本轮两处补齐。没有更改 lint 规则或屏蔽检查。
- 红：原样执行 npm run lint --prefix apps/web，报告两条与宿主完全一致的 preserve-caught-error 错误、0 warnings。原始输出：.autome/output/r8-lint-red.log。输出命令末尾 tail 的退出码不能当作 lint 退出码；红依据为日志中的两条错误。
- 绿：执行下列宿主指定快检（整体退出 0），原始输出 .autome/output/r8-quick-check.log：

`cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`

- 直接相关运行时检查：Node 从两个文件提取实际 AggregateError 表达式并分别求值，断言 cause === cleanupError 且 errors 深度等于 [error, cleanupError]，2/2 通过。没有新增测试框架或测试用例；原有项目 lint 就是本缺陷的持续回归检查。
- 上轮审计为 M-05 通过，未退回实现缺陷；本轮来源是宿主 lint，不存在需逐字移植的审计临时判别用例。

## 用例总数与基线差

本轮运行运行时检查 2 条，快检不报告 Vitest 用例数；未运行 M-06 Vitest 验收，不能声明其通过。上轮验收基线为 33 条，本轮相同验收集合执行 0 条，差 -33，原因是用户指定本轮只修快检并结束，不是删除或跳过测试。测试源码用例数量变化为 0；上轮监听/ps EPERM 阻塞仍未解除。运行时检查为本轮临时验证，单独计数，不与 33 条混算。未运行全量门禁。

## 四项自审

1. 情形表逐行：本轮只涉及 setup 与 cleanup 同时失败的两处路径；逐处确认 AggregateError 保留两个错误与 cleanup cause，2/2 通过。产品情形表不适用，本轮未修改产品行为、未标待审。
2. 失败分支：两处双失败分支执行表达式检查通过；仅 setup 失败仍 throw error，cleanup 执行顺序未变。授权、读盘、超时、空输入、外部不可达不适用，本轮不改这些路径。
3. 全部迁移：不适用，无状态机、通道或产品状态迁移变更；M-06 保持开放。
4. 输入域边界：不适用，未改变任何输入域、校验或限制。

提交：本轮按要求尝试提交当前分支；若沙箱拒绝，则提交由 Autome 兜底。
