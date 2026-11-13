# M-04 r4 审计复验（审计 #4）

结论：通过（含一处验证缺口，已就地补强并复验，不计 reopen）。

## 验收命令
原命令全部通过：静态门退出 0；`protocol::` 71 个通过；`prompt_rendering` 40 个通过；`## protocol/v` 标题计数 9。

## 负向对照（均已还原，`git status` 干净）
1. 漏标记行（删 `prompts/fix.md` 的 `<!-- regression-test-defect -->`）：`test_fix_marker_is_in_the_v9_fix_prompt` 红，70 过 1 败。有效。
2. A-1 缺 eval 目录：先编译再移走目录直接跑二进制，静态门退出 1，报「A-1：eval 目录不存在」。有效。（不先编译时是 `include_str!` 编译失败，不算静态门在拦。）
3. 把 `回归测试: 有缺陷` 写进 `bug-status-block` 契约区：`protocol::` 仍 71 过、静态门仍 0。**原验收拦不住**——期望契约哈希由种子本身 `contract_regions_of(seed())` 算出，种子内改动自洽；现有 `verify_contract` 用例只拦「候选版本与编译内期望不一致」。

## 验证缺口与补强
产品协议文本没有错（新键在契约区之外，位于独立小节「回归测试本身有缺陷」）。缺口在验收：契约承诺的第一条负向对照实际不会红。
已在设计文档 M-04 验收命令末尾加 `! awk '/kernel-contract: bug-status-block/,/\/kernel-contract -->/' protocol/bug-protocol.md | grep -q '回归测试'`，并改写负向对照说明。
复验：未注入时退出 0；注入后退出 1；还原后退出 0。

## 自造检查
- 契约区内容：区内只有五键示例，新键仅出现在区外小节（bug-protocol.md 第 46、55 行）。
- 新条文与 prompts/fix.md、fix-review.md、locate 的文字对得上；CHANGELOG A-1..A-4 齐全，A-1 的 eval 目录存在。
