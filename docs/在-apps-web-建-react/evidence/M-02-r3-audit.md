# M-02 审计复验（审计 #2）

结论：通过。

- 验收命令：`npm test --prefix apps/web` exit=0，3 个文件、24 个用例通过（本环境可正常监听，实现轮的 EPERM/EMFILE 阻塞未复现）。
- 负向对照（逐一改错、跑 `test/node-version.test.ts`、还原；还原后 12/12 通过，`git status` 干净）：
  - 字符串比较：4 个用例失败（9.9.0、22.9.0 等）。
  - 只比主版本 `>=22`：5 个失败，含伪造 22.21.1 的子进程用例。
  - `MIN_NODE_VERSION` 改 22.0.0：6 个失败，含结构/消息用例。
  - `start.js` 静态 import `./main.ts`：结构用例失败。
  - 版本检查挪到动态 import `main.ts` 之后：4 个失败（两个子进程用例、消息、结构）。
- 自造检查：Homebrew Node 22.23.1 真实运行 `server/start.js`（`AUTOME_WEB_PORT=0`），输出「autome-web 已启动」，即真实 22 线放行。
- 未覆盖：22.22.0 本身无法实测，靠 engines 声明与单测支撑。
- `lint`/`typecheck` 脚本尚未存在，属 M-04 范围。
