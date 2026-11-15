# M-11 r21 审计复验

- 验收命令（cargo build、npm run build、vitest 3 文件）：退出 0，3 文件 10 用例全过（含 kernel 组真实进程用例；本环境 ps 可用）。
- 负向对照（均临时注入后还原，`git status` 干净）：
  1. `runFromEnv` 不传 `onKernelBinaryGone`：web-binary-gone-real 失败（exitCode 为 null，15 秒后进程仍在）。
  2. 退出前跳过 `shutdown`（直接 process.exit）：web-binary-gone-real 失败于「server.json 已删」断言（expected true to be false）。
  3. `missing < 2` 改为 `< 1`（一次缺失即触发）：core-binary-watch「一次缺失不触发」等失败。
- 还原后复跑 core-binary-watch + web-binary-gone-real：6 用例全过，无残留 web-binary-gone 进程。
- 源码核对：连续两次缺失才触发；读盘错误（EACCES）记日志并清零；退出码 0；close() 停 watch。
- 自造检查：无新增（三条变异即判别检查）。
- 宿主快检在 ff87ffa4 通过（引用，不替代以上）。
结论：通过。无人工验收项。
