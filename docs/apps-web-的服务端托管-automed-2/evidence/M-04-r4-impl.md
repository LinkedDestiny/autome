# M-04 · 实现 #4

结论：M-04 安全模块验收 14/14 通过，提请待审；全量监听用例受沙箱阻塞，须由审计轮复验，不宣称全量通过。上一轮审计 M-03 通过，没有退回缺陷，因此没有需要逐字迁入的审计回归检查。

## 改动

- 新增 `server/security.ts`：Host 小写后精确比较、Origin 精确比较；GET/HEAD 只查 Host，其余方法同时查 Origin；API 拒绝回 JSON，页面拒绝回中文纯文本。
- `http.ts` 在 healthz 前装安全中间件，使用请求时的 `server.address().port`；Vite 显式关闭 CORS。Vite 初始化后移除旧 upgrade 监听器，装唯一守卫，暴露 `registerUpgrade`；通过检查后转交旧监听器及路径处理器，均不存在时 404。
- 新增 `security.test.ts` 14 个用例，对应设计 M-04 的 14 项；未接入 M-05 的网关、内核启动或端到端测试。无新增依赖，无 Cookie/令牌逻辑。

## 命令与结果

验收命令各段独立执行（使 Web 环境失败后仍能完成 desktop 检查）：

- `npm run lint --prefix apps/web`：退出 0；日志 `.autome/output/m04-lint.txt`。
- `npm run typecheck --prefix apps/web`：退出 0；日志 `.autome/output/m04-typecheck.txt`。
- `npm test --prefix apps/web`：收集 **152** 用例，比上一轮审计 **138 增加 14**，全部来自新 security 文件。145 通过、7 未执行，13 文件通过、server.test.ts 的 beforeAll 失败。启动日志包含 `EMFILE: too many open files, watch` 和 `listen EPERM: operation not permitted 127.0.0.1`；没有主动 skip 或删减测试。原始日志 `.autome/output/m04-web.txt`。真实内核用例通过；不能把这 7 个环境阻塞算成验收成功。
- `npm test --prefix apps/desktop`：104 用例，与上一轮差 **0**；103 通过、0 失败、1 跳过（原有 electron dom harness），日志 `.autome/output/m04-desktop.txt`。
- 所有负向修改还原后 `npm test --prefix apps/web -- test/security.test.ts`：14/14 通过、0 跳过，退出 0；日志 `.autome/output/m04-security-final.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：
   - GET、HEAD：正确 Host 放行，缺失/外站 Origin 不影响；错误 Host 在 `/healthz` 也拒绝。用例 5 通过。
   - 其它方法：POST/PUT/PATCH/DELETE/OPTIONS 在 `/api/write` 和 `/` 均测试可信源放行、缺失源与外站源拒绝；JSON 与纯文本格式分别断言。用例 6 通过。
   - upgrade：错误 Host、缺失 Origin、错误 Origin 均写 403 并销毁，旧监听器与注册处理器均未收到请求。用例 10 通过。
   - 无凭据、查询参数、Cookie 不提升权限、拒绝响应无 CORS、中文错误、源码禁用鉴权逻辑分别由用例 7/8/9/13/14 验证通过；开发模式唯一监听器及 CORS 配置由用例 12 通过。
2. **失败分支**：Host/Origin 拒绝、页面/API 拒绝、upgrade 403 与没有处理器的 404 都已执行（用例 2/4/5/6/9/10/11）；Host 与 Origin 同时错误时优先 Host。读盘失败、超时、外部服务不可达不适用：本模块只检查请求头、同步分派，不访问磁盘或外部服务。
3. **全部迁移**：无持久状态机。HTTP 放行→next / 拒绝→403 均验证；upgrade 对既有监听器有/无 × 注册处理器有/无四种组合逐一验证，转发参数保持不变，路径不匹配回 404，拒绝时两个通道都不触发。用例 10/11 通过。
4. **输入域边界**：Host 缺失、空串、缺端口、错误端口、前导零端口、IPv6、恶意域名后缀、前导空白、数组全部拒绝；127.0.0.1/localhost/LOCALHOST 和端口 1/7420/65535 通过。Origin 缺失、空串、null、https、错误端口、尾斜线、路径、大写域名、自定义协议、恶意后缀、数组全部拒绝，两个精确可信源通过。用例 1–4 通过。端口 0 的真实监听值路径由实现动态读取；本轮真实监听受 EPERM 阻塞，待审计复验。

## 负向对照

逐次临时修改 `security.ts`，每次运行 `npm test --prefix apps/web -- test/security.test.ts`，结束后 finally 恢复原文件：

- Host 改为 startsWith：2 失败 / 12 通过，拒绝表识别恶意后缀；`m04-negative-host-prefix.txt`。
- Origin 仅查 `/api/*`：3 失败 / 11 通过，页面写请求识别绕过；`m04-negative-api-only.txt`。
- 写入 Set-Cookie：2 失败 / 12 通过，无凭据响应与源码守卫识别；`m04-negative-cookie.txt`。
- 加入查询参数兑换 303：2 失败 / 12 通过，查询参数与源码守卫识别；`m04-negative-exchange.txt`。
- 保留原 upgrade 监听器：3 失败 / 11 通过，拒绝不转发与唯一监听器断言识别；`m04-negative-duplicate-upgrade.txt`。

以上日志均在 `.autome/output/`；五次退出码均为 1，恢复后 14/14 绿。本轮无上一轮缺陷，因此这些是设计负向对照，不冒充审计缺陷红绿回归。
