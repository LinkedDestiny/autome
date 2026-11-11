# 项目可配置准备命令，依赖变化时由宿主在 worktree 里安装

## 背景
实现会话（Codex）的沙箱没有网络。W3 给 `apps/web` 加了新依赖 `ws`，`package.json` 与 lock 都写对了，但 worktree 的 `node_modules` 里没有它；会话装不上（ENOTFOUND / ENOTCACHED），门禁每轮都失败，T-14 因此连续白跑 17 个实现轮，最后靠人在宿主上 `npm install` 才解开。主检出的 `node_modules` 同样不会跟着合并更新。

## 要做什么
- 项目配置 `.autome/config.toml` 新增 `[setup]`：`command`（用 `sh -c` 在 worktree 根目录执行，例如 `npm ci --prefix apps/web`）与 `watch`（触发重跑的文件列表，例如 `["apps/web/package-lock.json"]`）。
- 宿主在三个时机跑它：建 worktree 之后、第一个会话之前；每次门禁之前，若 `watch` 里任一文件的内容哈希与上次成功准备时不同；合并到默认分支之后在主检出里同样判断一次。
- 准备命令失败：任务进入失败，原因写明命令、退出码与日志路径；不当作门禁失败退回实现（会话无法修复环境）。
- 准备走与门禁相同的后台作业机制（日志、超时、进程组），写 `setup.started` / `setup.result` 事件，`autome run` 打印。

## 不做什么
- 不给会话开网络，不改会话沙箱。
- 不自动猜测包管理器；没配 `[setup]` 时行为与今天完全一样。

## 验收标准
- 没配 `[setup]` 时所有现有测试不改照常通过。
- e2e：配置一个会在 worktree 写标记文件的准备命令，任务第一个会话之前标记文件已存在。
- e2e：会话修改 `watch` 中的文件后，下一次门禁之前准备命令重跑一次；未修改时不重跑。
- e2e：准备命令失败时任务失败，原因含命令与退出码，且没有启动门禁。
- 合并后主检出的准备同样按 `watch` 判断，有测试。
- 本仓库的 `.autome/config.toml` 不在本任务里修改（由用户配置）。

## 参考
- T-14 的门禁日志 `.autome/output/gates/T-14/`（「Cannot find module 'ws'」）
- `crates/automed/src/gate.rs`（门禁后台作业）
