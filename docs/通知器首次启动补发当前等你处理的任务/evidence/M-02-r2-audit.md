# M-02 · 审计 #2 独立复验

验收命令：store 18 + notifier 17 + rules 12 + api 7 = 54 通过（顶层 test 数 18/17，rules/api 文件相对共同祖先未改动）；lint、typecheck 通过；server/app 改动只有 store.ts、notifier.ts。

负向对照（在 notifier.ts 上临时注入，已 git checkout 还原，工作区干净）：
- 去掉 L1==L0 确认 → N3、N6 失败
- 去重改回每条都算进入（不读持久化）→ N1、N3、N5 失败
- 不播种 → N5 失败
- 换账本不补发/不清状态 → N5、N6 失败
- 离开事件不落状态 → N2、N3 失败
都被拦住，验收有效。

全量 npm test：20 失败（core-sidecar 等 9 个文件），原因是未 cargo build 的真实二进制用例，属 M-03 范围，与本里程碑无关。
结论：通过。
