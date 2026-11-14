# 设计裁决记录

## 第 1 轮（裁决轮）

**评审结论：** 评审 #1 提出 6 条属于协议六类、可追溯的问题（R1-P01…P06）和 4 条非阻塞建议（N-01…N-04）。裁决轮逐条核对源码与设计文本后，**6 条全部采纳**，设计文档已按下表修改，`design-round` 由 0 增至 1。修改后本轮未发现剩余的六类问题，设计定稿：状态块 `status` 改为 `实现中`，九个里程碑均为 `开放`。

核对到的源码证据：

- `crates/automed/src/dispatch.rs::events_since`：带 `subject` 时调用 `events_by_subject`，回包 `latest_seq` 恒为全账本最大序号（`store.latest_seq()`）。
- `crates/automed/src/store.rs::events_by_subject`：SQL 先按 `subject_id` 过滤再 `LIMIT`。
- `crates/automed/src/entry.rs::serve_stdio`：阻塞读帧、收到请求才 `handle_command`，没有周期 tick。
- `apps/web/app/components/task-timeline.tsx`：只渲染 loader 返回的条目；`session-log.tsx`：展开 / 日志 / 滚动状态在组件内部，卸载即清除。
- `apps/web/server/onboarding-watch.ts` 与 `settings-data.ts`：页面加载时读退出标记 / pid。

### 逐条裁决

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01（R1-P01） | 按 subject 过滤后分页拿全账本 `latest_seq` 当终止 / 前进判据，其它任务更新或空任务时会误报「分页未前进」 | A-3；M-01 契约 | 采纳 | `events_since` 的 `latest_seq` 不随 subject 过滤；T-1 末事件 seq=10、T-2 有 seq=20 时，读 T-1 下一页为空而水位 20，按原设计报错。设计内部也矛盾（A-3 要求读到 `latest_seq`，而该值不属于筛选后的流） | A-3 重写终止条件：按原始页 `events.length < limit`（含空页）结束，游标在过滤噪声前推进，仅「非空页却不前进」报错；M-01 task-history 5→6 个用例（其它任务水位更大、空任务、恰好整页后空页、整页噪声），总数 55→56，加负向对照 | 0 |
| D1-P02（R1-P02） | 缺省「最近 20 个会话」窗口在新会话到来时滑动，会把已展开日志所在会话移出渲染树，key 稳定不能保住已被删除的组件 | C-Q1、C-Q3；M-05 | 采纳 | 条目 key 只保证节点仍在时不重挂；窗口 41–60 变 42–61 后条目 41 不再渲染，违反任务 C-2「新增条目时日志不折叠、滚动位置不丢」 | C-Q1：首次响应限 20 个，窗口起点由客户端用 `?from=<会话 id>`（`replace`、`preventScrollReset`）钉住，之后只增不缩；`from` 非法回退缺省窗口；C-Q3 保证理由改为「起点钉住 + key 稳定」两层，C-2 浏览器用例改为展开窗口里**最早**会话的日志后让新会话到来再加载更早；M-05 task-detail-data 6→7、总数 36→37、payload 夹具加「钉住后又到 5 个新会话」，加负向对照（不钉住则用例失败） | 0 |
| D1-P03（R1-P03） | 事件覆盖不了「账本里仍是运行中却无人收割」的情形，设计却把读盘兜底整个删掉，违反任务 A-5「覆盖不了的如实保留并写明」 | A-5、R-5；M-02 | 采纳 | 旧 `settings-data` 在页面加载时读退出标记，只读内核 / 无驱动时账本状态停在运行中，新方案刷新也看不到结束，属可证的页面加载退化。（评审已声明不主张旧版有实时推送，裁决照此，R-5 文字也据此改写） | A-5：删除轮询 `watch` / `onTick` / `onboarding.ended`，`onboarding-watch.ts` 改名瘦身为 `onboarding-disk.ts`，只在 `onboarding_session` 为运行中时页面加载读一次；账本已结束时不读盘；R-5 重写；M-02 增 `onboarding-disk.test.ts`（4）、settings-onboarding 兜底用例，9 个文件 74 个用例，文件检查与 grep 同步修改 | 0 |
| D1-P04（R1-P04） | stdio 内核存活被登记为「正在循环驱动」，但 `serve_stdio` 没有自己的 tick，status 会给出错误的确定答案 | E-3；M-08 | 采纳 | `entry.rs::serve_stdio` 只在收到请求后处理；开着 stdin 不发 tick 的 `autome core` 会一直持锁登记。F.4 的阻塞 stdio 替身进程是同类场景 | E-3：stdio 不在启动时登记，处理首条 `scheduler.tick` 才登记；`last_tick_at` 取登记文件 mtime（`touch()`）；`driver.list` 增 `last_tick_at`、`driving`（≤ max(30 s, 3×interval)）；status / 环境页区分「在推进」「已登记但最近没有推进」「没有其他驱动」；发布顺序改为隐藏名写好 → 持锁 → rename（同时吸收 N-03）；M-08 用例数与负向对照同步（driver_lock 11、cli 3、web-launcher 16、vitest 100） | 0 |
| D1-P05（R1-P05） | TERM→KILL 之间只用 `kill(pid, 0)` 判活，pid 被复用后可能误杀主检出 / 其它任务进程 | F.1、F.2；M-09 | 采纳 | 扫描与 KILL 之间最长 3 s，期间 pid 可复用；设计已取得起始时间却未绑定候选身份，与任务 F-2「主检出、其它任务、用户进程一律不碰」冲突 | F.1 候选记身份 `(pid, 起始时间)`；F.2 每次发信号与每次「还在不在」判断前复核起始时间与 exe / cwd 归属，身份变化 / `ESRCH` / 僵尸即停止处理且不进 survivors，无法复核记入 `task.processes_unchecked`；M-09 residue 单测增「pid 复用」「cwd 离开 worktree」，加负向对照 | 0 |
| D1-P06（R1-P06） | 用「进程起始早于 worktree 创建」推断其 exe / cwd 不可能在 worktree 内，并免除告警，进程可后来 `exec` / `chdir` 进入 | F.1；M-09 预期 | 采纳 | 进程出生时间不是它访问目录的时间；`proc_pidpath` 在 exe 被删时返回 `ENOENT`（设计实测表已记），此时年龄推断是静默漏判，违反任务 F-2「判据取不到时不杀并记警告」 | F.1 删除年龄豁免：路径取不到且另一项未命中即不杀并告警（同次清理按 pid 去重）；M-09 用例改为「早于创建也告警」并加负向对照；代价（本机一个 exe 被替换的长期进程每次清理多一条警告，只进账本）写明并进 BL-12 | 0 |

### 非阻塞建议处置

| ID | 处置 | 落点 |
|---|---|---|
| N-01 | 采纳（顺手修正验收判别力） | M-04 负向对照改为「action 忽略 `top=yes`」，`before` / `after` 对调由 queue-deps 组装用例守 |
| N-02 | 进 Backlog | BL-10 |
| N-03 | 随 D1-P04 一并吸收 | E-3 发布顺序 |
| N-04 | 进 Backlog | BL-11 |

### 争议项

无。所有主张复提计数均为 0（首轮），没有冻结项。

### 设计是否定稿

是。六条问题均已在设计中给出可执行的修改并落到对应里程碑的验收契约（用例数与负向对照已同步重算），未发现剩余的六类问题；`status` 改为 `实现中`，里程碑表完整（M-01…M-09，均为 `开放`，reopen 0）。Autome 应据此停在「等待批准」。

### 提交

提交由本轮尝试 `git commit`；若被拒绝，由 Autome 兜底提交。
