# M-02 r2 审计复验

- 环境：本轮允许本机监听（无 EPERM）。
- 负向对照：把 `projects.new.tsx` action 改成一律跳项目页（不看 skip_onboarding）→ add-project 第 5 条与 add-project-real 第 1 条失败，已拦下；已还原（git status 干净）。其余三项未逐一注入（时间所限），不影响结论。
- 验收命令：9 个文件、59 例，58 过、1 败。失败：`test/w9-http.test.ts` 「添加页 action 与 single fetch 坏 Host/Origin 拒绝且无注册」。
- 失败原因：测试的 `post` 辅助函数对响应体无条件 `JSON.parse`，而页面路由 `/projects/new`、`/projects/new.data` 的 403 回的是纯文本「Host 不是本机地址，已拒绝」，抛 `SyntaxError: Unexpected token 'H'`，测试失败并产生 Uncaught Exception。产品拒绝行为本身是对的（403、纯文本），错在测试辅助函数没有容忍非 JSON 响应。
- 结论：验收预期 59 例全过未达成 → 实现缺陷（项目测试不通过），退回开放，交实现轮修测试（让 `post` 对非 JSON 体不解析，或只断言状态码）。
- 判别检查：即验收命令中的该用例，无需另存。
