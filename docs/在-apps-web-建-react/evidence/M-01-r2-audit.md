# M-01 审计 #1 独立复验

结论：通过。

## 验收命令
`npm ci` + `npm test --prefix apps/web`：exit=0，2 个文件、12 个用例通过（与契约的基线 0 → 12 一致）。宿主门禁对 fa8c480 的 ci-local 也已通过（退出码 0）。

## 负向对照（均已还原，git status 干净）
- `listen(port)` 去掉 HOST：「只监听 IPv4 回环地址」与「端口被占」用例失败（2 failed）。
- 去掉两个 `express.static`：「prod 首页与引用的 JS 资源可访问」失败。
- `config.ts` 追加 `enum`：`server.test.ts` 整文件失败（子进程起不来），仅 config 5 个通过、7 个未跑。

## 自造检查
- 真实子进程 `AUTOME_WEB_PORT=0 node server/start.js`：dev 与 `build`+prod 两条路径均打印「已启动：http://127.0.0.1:<端口>」，`/healthz` 200，首页含「Autome 面板」「面板骨架已就绪」。
- Homebrew Node 22.23.1 下 prod 路径可起服务。
- 22.22.0 本身未实测（本机没有）。
- lsof 过滤未能证明监听地址（命中无关进程），绑定地址以负向对照 A 的用例为准。

人工项 H-01/H-02（浏览器 HMR、hydration 控制台）已在设计文档人工验收清单中，不阻塞关闭。
