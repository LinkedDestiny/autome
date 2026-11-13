# 旧内核不能推进新账本：内核带构建标识，比账本旧就只读；面板找内核的顺序与安装方式对齐

## 背景

2026-11-13 12:20，在 main 上按计划 §7.3 的说法用 `npm --prefix apps/web start` 起面板看 W6 的效果。面板起的内核是 `target/debug/automed`——主检出里 11 小时前 cargo 留下的旧构建（早于 T-61、T-67、T-73、T-77）。它和 launchd 里的驱动轮流拿驱动锁，5 分钟内造成：

- T-59、T-54 被判「失败（实现）：配置问题：prompts/impl.md 里有 Autome 不认识的占位符 `{gate_line}`」——任务固定的协议是 v4 / v6，旧内核只认识到 v3；
- 新建的 T-81 被它固定在 `protocol/v3` 上（只能取消重提）；
- 它不认识 `[gate] run = "before_merge"`、快检、合并不占槽位这些后来的行为，如果没被及时发现，还会按旧规则跑门禁、推进状态。

原因有两层：

1. `apps/web/server/core/binary.ts` 的查找顺序是 `AUTOMED_BIN` → 同包的 `automed` → `~/.local/bin/automed` → cargo 构建目录。但 `scripts/install-cli.sh` 只装 `autome` 一个文件（内核是链接进 CLI 的库），`~/.local/bin/automed` 从来不存在，于是在源码检出里起面板一定落到 cargo 目录，那里是什么版本全凭运气。
2. 内核没有任何机制发现「我比这份账本旧」。任何旧二进制（旧的桌面端、没重装的 CLI、另一个 worktree 里的构建）接上同一个 `AUTOME_HOME` 都会照常推进。

## 要做什么

1. **构建标识。** `automed`（库与二进制）在构建时嵌入一个可比较的标识：建议用构建所在提交的提交时间戳 + 短 sha（`build.rs` 读 git；拿不到 git 时退回到包版本与构建时间，并标记为「未知来源」）。`autome --version`、`automed --version`、`env.get` / `core` 握手信息里都带上它。口径由设计轮定准，要求是「同一个提交构建出来的 CLI 与内核标识相同，后来的提交标识更大」。
2. **账本记下推进过它的最新内核。** 每次真正推进（`scheduler.tick` 拿到驱动锁、以及会改任务状态的写命令）时，把自己的标识与账本里记的比较：
   - 自己更新或相同：照常推进，并把账本里的值更新为自己的；
   - 自己更旧：**不推进**。`scheduler.tick` 返回 `skipped`，原因写明「这个内核（<标识>）比推进过账本的内核（<标识>）旧，已停止推进；请重新安装或设置 AUTOMED_BIN」；会改任务状态的写命令（创建、开始、批准、合并、rerun 等）同样拒绝并给这句话；只读方法照常工作。
   - 不能因此把任何任务判失败，也不能给任务固定协议。
3. **逃生口。** 确实要用旧内核接管（回滚）时：`AUTOME_ALLOW_OLDER_KERNEL=1` 或一条显式命令（`autome doctor --adopt-kernel` 之类，设计轮定名）把账本里的值降回当前内核，记一条事件。README 写明。
4. **面板找内核与安装对齐。**
   - `scripts/install-cli.sh` 与 `scripts/package-cli.sh` 把 `automed` 也装到 `$PREFIX`（或者 `autome` 提供一个等价的子命令让面板以 stdio 方式拉起内核——两种做法设计轮选一，要求是「装了 CLI 就一定有一个与 CLI 同版本、面板找得到的内核」）；
   - `binary.ts` 落到 cargo 构建目录时在启动日志里明确打一行「正在使用源码构建的内核：<路径>（<标识>）」；不管来源是哪一种，启动时都打出内核路径与标识；
   - 面板收到第 2 条的「内核比账本旧」时，顶部横幅显示这句话，写操作置灰（沿用内核离线时的样式），不再每 3 秒重试 tick 刷日志。
5. **CLI 侧同样受保护。** `autome run` 用的是链接进去的内核，走同一段比较；被拒绝时 `autome run` 打一行原因后退出码非零（launchd 会重启它——设计轮确认不要形成每 10 秒一次的刷屏，例如退避或保持进程但不推进）。
6. 文档：README「安装 / 升级」一节与 `docs/development/technical-design.md` 写明构建标识、谁能推进账本、逃生口；计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3 里「用 `npm --prefix apps/web start` 起面板」一句补上「先重装 CLI」的前提。

## 不做什么

- 不做账本格式的版本迁移框架；这里只管「谁能推进」。
- 不做多台机器之间的协调。
- 不改协议正文。
- 不处理 W10 的发布包结构（W10 沿用这里定下的安装位置即可）。

## 验收标准

1. 单测：构建标识的解析与比较（含「未知来源」）；同一提交的 CLI 与内核标识相同。
2. e2e：先用「新」标识的内核推进一次，再用「旧」标识的内核（测试里注入标识）接同一个 `AUTOME_HOME`：`scheduler.tick` 返回 `skipped` 且带原因；`task.create`、`task.approve`、`task.merge`、`task.rerun_from` 被拒绝并给同一句话；`task.get`、`dashboard.get`、`events.since` 正常；账本里没有新增任何 `task.updated`，也没有任务被固定协议。
3. e2e：旧内核带逃生口启动后可以推进，事件里有记录；之后新内核再接上，账本的值回到新的。
4. e2e：排队中的任务在旧内核接上期间不被判失败（对应 T-59 / T-54 的现场：协议副本里有旧内核不认识的占位符）。
5. `install-cli.sh` 在空 `PREFIX` 下装完后，面板的 `resolveAutomedBinary` 找到的内核与 `autome --version` 标识一致（测试）；`package-cli.sh` 的 tar 包里有对应文件。
6. 面板：内核返回「比账本旧」时横幅与置灰有测试；启动日志含内核路径与标识。
7. `autome run` 被拒绝时不刷屏（测试断言一段时间内的输出行数上限）。
8. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 参考

- `apps/web/server/core/binary.ts`、`apps/web/server/core/supervisor.ts`、`apps/web/server/ticker.ts`。
- `scripts/install-cli.sh`、`scripts/package-cli.sh`；`crates/autome-cli/Cargo.toml`（CLI 把 `automed` 当库链接）。
- `crates/automed/src/scheduler.rs`：驱动锁与 `tick`；`freeze_protocol`（旧内核给新任务固定旧协议的地方）；`crates/automed/src/dispatch.rs` 的写方法清单。
- 现场：账本事件 7452–7460（2026-11-13T04:20:58Z 起），`.autome/output/web-panel-main.log`。
