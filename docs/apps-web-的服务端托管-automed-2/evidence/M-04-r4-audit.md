# M-04 审计 #4 独立复验

结论：通过。

## 验收命令
- lint、typecheck 无错误；Web 14 个文件 152 个用例全过（138 + 14），skipped 0；desktop 104 个，103 通过、1 跳过（electron dom harness，基线如此）、0 失败。
- 宿主门禁 ci-local.sh 在 6647b73 通过（简报所载）。

## 负向对照（均在 server/security.ts 上临时注入，逐个还原，git status 干净）
| 注入 | 结果 |
|---|---|
| Host 改为 startsWith('localhost') | 2 个 Host 用例红 |
| Origin 只在 /api/ 上检查 | 3 个用例红（含 POST `/`） |
| 响应写 Set-Cookie | 无凭据 POST 用例与源码守卫红 |
| 加 `?t=` 303 兑换 | 4 个用例红 |
| upgrade 守卫不查 Origin | upgrade 拒绝用例红 |
| 不移除既有 upgrade 监听器 | 3 个用例红（含 listenerCount===1） |

## 自造检查：真实监听（开发模式 createHttpServer，端口 0）
合格 Host /healthz 200；evil Host 403；POST 缺 Origin 403；外站 Origin 的 OPTIONS 403；GET 带另一 localhost 端口 Origin 200 且无 ACAO/Set-Cookie；upgrade 外站 Origin 403、坏 Host 403；合格 upgrade 到达 Vite（因无 Vite 内部 token 回 400，属 Vite 自身校验）；`listenerCount('upgrade')` = 1。
