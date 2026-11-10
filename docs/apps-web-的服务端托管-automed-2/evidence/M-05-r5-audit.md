# M-05 r5 审计证据（独立复验）

结论：通过。

## 验收命令
`cargo build -p automed && lint && typecheck && npm test --prefix apps/web && build && npm test --prefix apps/desktop` 退出码 0。
Web 16 个文件 175 个用例全过（152+23，与预期一致，skipped 0；本环境允许回环监听，实现轮受阻的监听用例已真实运行）；desktop 104 个：103 过、1 跳过（electron dom harness，缺 electron 二进制，与本里程碑无关）、0 失败。
真实 `~/.autome/state/automed.sqlite3` mtime 前后均为 1794305868；`~/.autome/web` 前后均不存在。

## 负向对照（逐条注入、跑、还原；`git status` 干净）
1. `core.restart` 当普通 op 转发 → gateway.test 2 个失败（本地处理、真实内核重启）。
2. 安全中间件移到 `/api` 之后 → gateway-http 3 个失败（Origin 不对的写、Host 不对的 GET、无 Cookie/CORS）。
3. 写路径要求 Cookie → gateway-http 5 个失败（无凭据读、无凭据写后读回等）。
4. `close()` 不停 supervisor → 「close 后内核 pid 不存在」失败。
5. `launch()` 不传临时 AUTOME_HOME → core-sidecar 扫描用例「no test starts a real core against the developer own ~/.autome」失败。
五条均能被拦住。

## 自造检查（通过，不留存）
生产构建起服务（临时 home、端口 7611），curl 验证：合格 Host+Origin、无凭据 `config.get` 回 200 且响应头无 Set-Cookie；外站 Origin 写回 403；Host 为 evil.com 回 403；`project.pick` 回 501 中文原因；SIGTERM 后内核进程消失；临时 home 下只有 config.toml、state，没有 web/。
日志：`.autome/output/m05-audit-accept.log`。
