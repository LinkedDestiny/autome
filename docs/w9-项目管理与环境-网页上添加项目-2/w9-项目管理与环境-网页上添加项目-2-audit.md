# 实现审计

- M-01：通过。验收 120/120，四项负向对照均被拦下并已还原。证据：evidence/M-01-r1-audit.md。
- M-02：实现缺陷（领域 add-project）。验收 59 例 58 过 1 败：`cd apps/web && npx vitest run test/w9-http.test.ts` 中「添加页 action 与 single fetch 坏 Host/Origin 拒绝且无注册」抛 `SyntaxError: Unexpected token 'H', "Host 不是本机地址，已拒绝" is not valid JSON`（`test/w9-http.test.ts:16`，`post` 辅助函数无条件 JSON.parse，页面路由 403 回纯文本）。产品 403 行为正确，需修测试辅助函数。其余 8 个文件全过；负向对照有效。证据：evidence/M-02-r2-audit.md。
