# 实现 #12：M-05 rebase 冲突处理

## 范围与修复

本轮简报没有开放里程碑；M-01 至 M-07 已被审计关闭。按本轮用户明确要求处理 rebase，不新增里程碑、不改写已审状态、不标记新的已完成或待审。上一审计 #8 无退回缺陷，因此没有需要移植、先红后绿的审计检查。

默认分支经 origin/HEAD 确认为 main，目标提交 219d891c。执行 `git rebase main`，在提交 5b58717e 的 apps/web/server/main.ts 出现两处冲突。第一处同时保留 main 的 userHome、terminal 注入及本分支的 binaryArgs、kernelEnv、describeKernel；第二处同时保留 canTick/isSlow 限频与 attached.onTick、admin.onTick(report) 两个回调。git add 后以 `GIT_EDITOR=true git rebase --continue` 完成全部 52 个重放步骤。解决结果已进入重放后的功能提交。

`git merge-base --is-ancestor main HEAD` 返回 0。main 的 SCHEMA_VERSION 为 7，本分支为 8，未占用同一迁移号，无须顺延。未修改协议、真实账本、安装位置或其它 worktree，未推送。

## 命令与结果

- `cargo build -p automed -p autome-cli && npm run test --prefix apps/web -- test/core-binary.test.ts test/kernel-identity.test.ts test/older-kernel-live.test.ts test/older-kernel-core.test.ts test/older-kernel-real.test.ts test/ticker.test.ts test/core-supervisor.test.ts test/live-banner.test.ts`：构建通过；51 例，49 通过、2 失败、0 跳过，退出 1。两条 HTTP/WebSocket 服务测试都在监听 127.0.0.1 时收到 EPERM；日志另有 watcher EMFILE。不是断言证明的产品缺陷，宿主须复验。原始输出：`.autome/output/r12-build.txt`、`r12-m05.txt`。
- `npm run test --prefix apps/web -- test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/env-real.test.ts test/core-restart.test.ts`：4 文件、20 例全部通过，退出 0。覆盖 main 管理功能相关逻辑及真实 stdio；原始输出 `.autome/output/r12-main-regression.txt`。
- rebase 日志：`.autome/output/r12-rebase.txt`、`r12-rebase-continue.txt`。`git diff --check` 通过。

**用例总数与差**：本轮 71 个唯一用例，69 通过、2 环境失败、0 跳过，无新增测试。M-05 上次审计基线 51，本轮同子集仍 51，差 0；额外 20 来自 main 管理与重启回归。上一实现 #11 执行 29，本轮数字为 +42，原因是测试范围更换，不能解释成新增 42 个测试或全量覆盖增长。未运行全量门禁及宿主快检。

## 四项自审清单

1. **情形表逐行**：查找顺序 9 例通过；日志来源与标识获取 7 例通过；outdated 映射、hello、横幅、写闸门、reload 5 例通过；supervisor/限频 5 例通过；真实内核 4 例中 stdio 拒写/接管与退出回收 2 例通过，HTTP hello/409 与服务启动日志 2 例受 EPERM 阻塞；原有 ticker/supervisor/banner 21 例通过。冲突中 userHome、terminal、两个 onTick 回调经对 main 差异逐项检查均保留，额外 20 例通过。
2. **失败分支**：文件不存在、version 非零、无标识、version 超时由 identity 用例通过；旧内核拒写由真实 stdio 用例通过；HTTP 409 本轮未取证，因本地监听被拒。没有新增外部服务或授权分支。
3. **全部迁移**：starting→outdated、outdated→connected、connected→outdated、outdated restart 先停止回收再启动、重复状态不重复日志，由 older-kernel-core 全部通过；ticker 正常/慢速与 busy 分支由对应测试通过。既有其它 supervisor 生命周期 10 例通过。数据库迁移不适用，本轮未修改迁移，仅核对 main v7 与本分支 v8。
4. **域边界**：空覆盖、缺省 profile、多 PATH 候选及无候选由 binary 测试通过；限频 29999ms 边界和累计 60000ms 的两次 tick 通过；标识缺失/超时通过。没有新日期或数值输入域。

## 后续

状态轮数为 12/35，保持审计已关闭状态及原 M-05 审计证据指针；本文件记录合并修复，不覆盖独立审计结论。宿主重跑快检、M-05 受阻测试与合并前门禁；H-01 至 H-03 仍由用户合并前验收。本轮结束，不启动下一会话。
