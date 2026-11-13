# 旧内核不能推进新账本：裁决记录

## 第 1 轮（design-round 0 → 1）

评审结论：第 1 轮评审提出 4 项六类范围内的问题（R1-P01～R1-P04）和 2 项非阻塞建议（S-01、S-02）。裁决：4 项均采纳并已修改设计；2 项建议是设计内部的小错或实现注意事项，一并就地处理，不进 Backlog。修改后没有剩余的六类问题，设计定稿，`status` 改为 `实现中`，等待用户批准。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01（R1-P01） | 以提交者时间为次序键，同秒父子提交被双向放行，时钟倒退时更旧的提交被判「更新」，违反任务 §3.1/约束 4「后来的提交标识更大」 | 方案 1 比较表；M-01 单测第 5、6 项；风险「提交时间不单调」 | 采纳 | 评审在临时仓库造出同秒父子提交（`.autome/output/review-1-probes.log` 的 `git-same-second-parent-child`）；设计原文的比较表对该情形明确写「可推进、不写」，反例成立。任务要求的是严格递增，风险段只写「新内核被拒」漏掉了反向放行 | 次序键改为 `git rev-list --count HEAD` 的祖先提交数（子孙严格更大，与时钟无关）；文本形如 `0004213-20261113T042058Z-7715d8ed0a1b`，时间只给人读不参与比较；祖先数相同且 sha 不同判「不可比，拒绝」；浅克隆按未知来源；风险段改写为「提交图被改写」；M-01 单测第 5、6 项与测试用的标识格式随之更新，新增 `build_id_orders_same_second_parent_and_child`（真实 git） | 0 |
| D1-P02（R1-P02） | `claim` 事务只保护比较与更新，取驱动锁出 I/O 错误后仍不带锁继续，O 与 N 交替时旧内核可在新内核推进之后继续写，设计「无锁也成立」的说法不对 | 方案 3；方案 4 两个检查点 | 采纳 | 核对源码：`scheduler.rs` 的 `tick` 与 `dispatch.rs` 的 `handle_command` 的 `Err` 分支都是 warn 后继续；评审的双连接模型按「O 提交标记 → N 抬高并推进 → O 推进」顺序执行，事件顺序为 new 后 old（`claim-then-advance-without-driver-lock`）。内存库的 `try_acquire` 恒返回 `Some`，不受影响 | 方案 3 删去「无锁也成立」，改写为互斥靠驱动锁覆盖到最后一个副作用；方案 4：文件账本取锁出错时 tick 返回 `skipped: true` 加 `errors`，写命令返回 `internal`，都不调 `claim`；M-02 新增第 7 号用例 `older_kernel_lock_error_stops_all_advancing`（用例数 9→10） | 0 |
| D1-P03（R1-P03） | 只监视当前存在的 git 文件，`pack-refs` 加 `core.logAllRefUpdates=false` 时下一次提交新建松散引用，HEAD 与 packed-refs 都不变，build.rs 不重跑，标识停在旧提交，违反约束 5 | 方案 2；M-01 重跑路径用例 | 采纳 | 评审用独立无依赖 cargo 项目复现：`firstMatchesA=true, secondMatchesB=false, secondStillA=true`（`packed-ref-no-reflog-cache`）。设计只用 `watch_paths` 返回哪些现存文件来证明，证明不了「标识跟上 HEAD」 | 方案 2：松散引用不存在且无 `logs/HEAD` 时，仍把这个不存在的路径发给 cargo（cargo 视为每次都变，build.rs 每次重跑，宁慢勿陈旧）；取值与路径选择放进 `build_support.rs` 供测试共用；新增 `tests/build_rerun.rs` 3 个用例，用真实 git 与两次真实 cargo 构建覆盖 worktree、packed-ref 无 reflog、同秒父子（M-01 用例数 17→20，验收命令加 `--test build_rerun`） | 0 |
| D1-P04（R1-P04） | 上线顺序先装 CLI 迁移、再 kickstart、最后才处理旧面板，而 schema 检查只挡新连接，已打开账本的旧进程可在此期间与新驱动轮流拿锁推进，正是要阻断的事故路径 | 方案 3「本任务之前构建的内核」；方案 14；风险 | 采纳 | 评审的双连接实验：新连接升到 v8 后，先前已打开的旧连接仍成功写入一条事件（`migration-existing-connection`）；`store.rs` 的版本检查只在 `migrate`（`Store::open` 调用）里。设计原文自己也承认「旧驱动仍持着已打开的连接照常运行」，却只排了 kickstart 而没有覆盖旧面板与桌面端 | 方案 3 补「这道门只挡新连接」；方案 14 上线顺序改为：合并 → 退出桌面端、停面板、`launchctl bootout` 驱动并确认无残留进程 → 重装 CLI 迁移 → `launchctl bootstrap` → 重启面板与桌面端；README 必须写明迁移不撤销已打开的旧连接；风险段、H-02 同步；M-06 验收命令加 `grep -q "launchctl bootout" README.md`（grep 5→6 个） | 0 |
| S-01 | 方案 8 与 M-05 写「共 8 条」，实际 5+1+3=9；26、47 的合计相应应为 27、48 | 方案 8；M-05 | 采纳（非阻塞，就地修正） | 清点方案 8 列出的条目即得 9；契约三项齐全，不单独触发轮次 | 方案 8、M-05 的条数与预期总数改为 9、27、48 | 0 |
| S-02 | `CoreSupervisor.restart()` 调 `start()` 会覆盖 `this.sidecar` 而不停原进程，`outdated` 下重启可能留下孤儿子进程 | 方案 10 恢复路径；M-05 `older-kernel-core` 第 5 号 | 采纳（非阻塞，写入实现要求） | 核对 `supervisor.ts`：现有 `restart()` 确实不先停原 sidecar；方案 10 原文并未排除，但没有写出先停后起 | 方案 10 写明先停止并回收原 sidecar 再启动；`older-kernel-core` 第 5 号断言 `stop` 先于新 `spawn`（用例数不变） | 0 |

## 争议项

无。复提计数均为 0。

## 备注

- 评审基线 `ca88a572`；本轮只改设计文档、本记录与 `retro.md`，不改产品代码，没有运行真实面板或接触真实账本。
- 里程碑表保持 M-01..M-06 全部 `开放`、reopen 0；状态块 `current-milestone` 保持 `无`，`next-action` 不变，`status` 改为 `实现中`，`design-round` 为 `1/15`。
- 本轮新增的验证项都用临时目录与临时 `AUTOME_HOME`（`build_rerun` 在临时 cargo 项目里构建，不碰仓库 `target/`），符合约束 6。
- 提交由 Autome 兜底（若本轮提交被拒）。
