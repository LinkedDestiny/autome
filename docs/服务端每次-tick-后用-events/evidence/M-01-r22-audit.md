# M-01 审计 #1 独立复验

环境：本审计会话可监听 127.0.0.1，cargo 产物 target/debug/{automed,autome} 存在。原始输出在 `.autome/output/a1-*.log`。

## 验收命令（各自退出码）
- lint 0；typecheck 0；build 0；`npm test --prefix apps/web` 0：18 文件、189 用例、0 跳过（含 `eventbus-live` 5 个真实跨进程用例，未跳过）。
- 189 比契约写的 188 多 1：来源是 `gateway-http.test.ts` 新增「/ws 已注册，缺少握手字段回 400」（/ws 注册后原 404 用例改指向未注册路径所必需）。契约文字已更正。
- desktop：104 / 103 过 / 1 跳过，与基线相同；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 输出为空。

## 负向对照
1. `pull()` 循环改为只拉一页（`break`）：`eventbus.test.ts`「2500 条分三页全部广播」失败，其余 7 过。✔ 拦得住。
2. `main.ts` 去掉 `onTick`：`eventbus-live`「命令行跨进程 cancel…」3725ms 超时失败。✔ 拦得住。
3. `/ws` 改用 `server.on('upgrade')` 自行处理：**契约原先点名的 `eventbus-live` ② 与 `security.test` 都不失败**（19 全过）。原因：`security.test` 用的是 `createHttpServer` 而非 `startServer`，守卫监听器仍在先拒绝外站；这是**验证缺口**，产品行为（走守卫）当前没错。
   - 补强：`gateway-http.test.ts`「/ws 已注册，缺少握手字段回 400」增加 `s.server.listenerCount('upgrade')` 为 1 的断言（用例数不变）。复验：注入绕过 → 该用例失败（10 过 1 败）；还原 → 11 过。
4. 全部注入还原；`git status --porcelain` 只剩本轮有意的测试补强与文档改动。

## 结论
通过。验证缺口当场补强并复验，不计 reopen。
