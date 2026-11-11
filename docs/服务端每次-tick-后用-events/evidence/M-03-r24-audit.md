# M-03 审计复验（审计 #3）

验收：lint 0、typecheck 0、build 0、`npm test --prefix apps/web` 0（22 文件 / 224 用例 / 0 跳过；比契约 219 多 5，全部来自 M-02 审计的 208 vs 203，M-03 新增 16 = 8+6+2）、desktop 104 / 103 过 / 1 跳过、crates/desktop/protocol/plugins diff 为空。本环境真实监听可用，实现轮的 EPERM 阻断未复现。

负向对照（逐个注入、跑 live-client/live-routes、git checkout 还原）：
- 不设待处理批直接 revalidate：live-routes 3 个失败。
- revalidate 一调用就清批：3 个失败。
- 退避不封顶：live-client「退避序列封顶」失败。
- 纯重验判断写成 URL 对象比较：3 个失败。
- reload 不清 lastSeq：live-client ⑧ 失败。

验证缺口：去掉 generation 守卫（finally 无条件清批）→ 原 14 个用例全绿。补用例后注入变红，还原变绿，lint/typecheck 通过。无产品缺陷。
真实浏览器项已在 H-01..H-03。
