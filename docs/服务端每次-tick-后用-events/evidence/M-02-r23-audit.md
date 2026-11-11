# M-02 r23 审计复验

验收命令（各自退出码）：lint 0、typecheck 0、build 0、`npm test --prefix apps/web` 0（19 文件 / 208 用例 / 0 跳过）、desktop 104 / 103 过 / 1 跳过、`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 为空。
用例数 208 比契约 203 多 5：eventbus.test.ts 现 22 个（契约 18），eventbus-live 7，其余为前序审计补强；只多不少，无整层跳过。
本环境真实监听可用：eventbus-live 的 7 个用例（含 ⑥ 断线 resume、⑦ 杀 automed 的 core offline→connected）全部实跑通过，实现轮的 EPERM 阻断不复现。

负向对照（逐个注入、跑 eventbus + eventbus-protocol 测试、`git checkout server/` 还原）：
- 缺口 `> 1000` 改 `>= 1000`：「缺口 1000 补发，1001 只 reload」失败。
- `restarting` 映射成 connected：protocol 映射用例与 core 推送用例失败。
- reload 不重置 `sentSeq`：账本回退两个用例失败。
- 补发期间不排队直接发：3 个用例失败（含小缺口严格递增）。
- 恢复时把游标重置为 latest（`initialized=false`）：「恢复后立即从原游标补拉」失败。
全部被拦，已还原（git status 干净）。

结论：通过，无验证缺口，无新增检查需保留。
