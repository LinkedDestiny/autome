# 裁决记录

## 裁决轮 1（design-round 0 → 1）

评审结论：评审 #1 提出 3 条内部逻辑矛盾（P-01..P-03），均属六类问题，无非阻塞建议。此前无裁决记录，无冻结争议项，三条的复提计数均为 0。
三条都是确定性反例或框架/Git 实测，裁决轮对 P-02、P-03 重跑了实测（见各条证据）。

### D1-P01（评审 P-01）账本回退后旧游标继续过滤新账本事件

- 主张摘要：账本被换掉（`latest_seq < cursor`）时设计只重置总线 `cursor`，连接的 `sentSeq` 与客户端 `lastSeq` 仍是旧值；`reload` 只触发重验、不改去重游标，之后新序列里 11..100 的事件会被服务端 `seq > sentSeq` 或客户端 `seq <= lastSeq` 吞掉。在线发现与重连时发现两条路径同源。
- 设计位置：§3 游标与拉取（账本回退一条）、续传情形表 `after > latest_seq` 行；§4 `createLiveClient` 的 `hello`/`event`/`reload` 处理；M-02 ⑫⑰。
- 裁决：**采纳**。
- 证据：按设计逐步推演成立——`sentSeq=lastSeq=100`、换账本后 `latest_seq=10`，随后 `seq=11` 在服务端第 144 行条件与客户端 `seq <= lastSeq` 两处都被丢；这是设计内部不自洽，实现者无法靠「照设计写」得到正确行为，也无法用已写的 ⑫⑰ 发现（它们只观察到 `reload`）。
- 修改落点：
  - §3：账本回退时游标与每个连接的 `sentSeq` 一起切到 `latest_seq`、取消正在续传的连接并发 `reload`；新增「`reload` 统一语义」——服务端保证之后的事件都大于 `sentSeq` 基线，客户端收到即放弃检查点；`reload` 信封仍是 `{type:"reload"}`，不加字段。情形表 `after > latest_seq` 行补 `state = live` 与客户端据此接收小 `seq`；「`sentSeq > hello.seq` 且 `after < sentSeq`」行注明 `sentSeq` 不变。
  - §4：`reload` → `lastSeq = null` 再 `onReload`；`event` 在 `lastSeq` 为空时接受任意 `seq`；`hello` 在「`lastSeq` 为空且非首次连接」时按 `reload` 处理并以 `hello.seq` 重建检查点（堵住「reload 后、新事件到来前断线」的丢失窗口）。
  - M-02：⑫ 增加「回退后 `seq = latest_seq+1` 的新事件仍送达」；⑰ 增加「`sentSeq` 一起重置（含取消续传的连接）、小 `seq` 新事件恰好收到一次」；负向对照增加「只重置 `cursor` 不重置 `sentSeq`」。
  - M-03：`live-client.test.ts` 增加 ⑧（`reload` 清检查点后小 `seq` 被接受，覆盖在线与 `resume` 之后两条路径，及 reload 后断线重连）；用例数 7→8，Web 总数 218→219（M-03、M-04 契约同步）；负向对照增加「收到 `reload` 不清 `lastSeq`」。
- 已知边界（不另立问题）：账本被换掉、且在总线下一次拉取之前就已增长到超过旧游标，这种回退无法被检测；需要两者在 3 秒窗口内同时发生，不在 W3 范围。
- 复提计数：0。

### D1-P02（评审 P-02）用 URL 对象恒等判断纯重验

- 主张摘要：设计 §4 把「纯重验」写成 `currentUrl === nextUrl`；React Router 对两者分别 `createURL`，对象恒不相等，于是总是落到 `defaultShouldRevalidate`，匹配器不生效，M-03 ① 无法通过。
- 设计位置：§4 `liveShouldRevalidate` 一条；M-03 `live-routes.test.ts` ①。
- 裁决：**采纳**。
- 证据：裁决轮核对已安装的 react-router 8.5.1：`router.js:2065-2066` 为 `let currentUrl = history.createURL(state.location); let nextUrl = history.createURL(location);`，两次调用各返回新 `URL`，对象 `===` 必为 false；评审的 `createMemoryRouter` 实验（`.autome/output/review-router-result.json`）：对象比较时项目与任务 loader 各跑 1 次，改 href 比较后项目 0 次、任务 1 次。
- 修改落点：§4 改为 `currentUrl.href === nextUrl.href`（按值比较），并写明原因与「同址链接点击在存在待处理批的短窗口内会走匹配器」的接受理由；M-03 负向对照增加「写回对象比较时 ① 失败」。
- 复提计数：0。

### D1-P03（评审 P-03）只替换 NUL 无法得到 M-04 写定的历史 numstat 结果

- 主张摘要：设计 M-04 预期 `git diff --numstat 30e5d51 -- apps/web/test/write-gate.test.ts` 输出 `2	2`，但基线 blob 含 NUL，Git 对含二进制一侧整体按二进制，输出仍是 `-	-`；任务 R-06 的该条判据与 Git 行为冲突。
- 设计位置：M-04 说明与 M-04 验收契约；任务 R-06 的验收句。
- 裁决：**采纳**。冲突来自任务文件本身，裁决轮不改任务文件，也不授权 `.gitattributes`、改基线等越界手段；在设计里显式记录冲突并换成可达的等价判据。
- 证据：裁决轮独立重做：取 `git show 30e5d51:apps/web/test/write-gate.test.ts` 为旧副本（NUL 数 2），副本里把两个 0x00 换成 `\u0000` 为新副本（NUL 数 0）。Git 2.53.0：`git diff --no-index --numstat old new` → `-	-`；`--numstat --text` 仍 `-	-`；`git diff --no-index --text -U0 old new` 给出恰好 2 处替换（第 117、398 行）；`git diff --no-index --numstat /dev/null new` → `562	0`（新文件按文本），`/dev/null old` → `-	-`。
- 修改落点：M-04 说明增加「R-06 验收判据与 Git 行为的冲突」一段；M-04 验收契约改为：NUL=0；相对基线的 numstat 预期仍为 `-	-`（只记录，不算失败）；`git diff --text -U0 30e5d51 -- <文件> | grep -cE '^[-+][^-+]'` 输出 `4` 且两处内容符合；`git diff --no-index --numstat /dev/null <文件>` 以行数开头而不是 `-	-`；35 个用例不变、`-t` 过滤 2 过 33 跳；负向对照同步（只改一处时 NUL=1、补丁行数 2）。
- 复提计数：0。

## 设计定稿判断

三条均已按采纳修入设计，并用评审或裁决轮的实测支撑。逐类复核：

1. 关键事实错误：无（P-02 的框架行为、P-03 的 Git 行为已改为实测值）。
2. 内部逻辑矛盾：三条已消除；复核修改后的 `sentSeq`/`lastSeq`/`reload` 规则——在线回退、重连回退、续传缺口超 1000、`sentSeq > hello.seq` 的 `reload`、reload 后断线重连五种路径都能达到「新事件被收到且同一连接严格递增」。
3. 违反项目规则、4. 违反任务目标：无；`reload` 信封未改，`hello`/`event`/`core`/`resume` 与计划 §5.2 一致，R-06 的偏差已作为任务验收句与 Git 行为的冲突如实记录。
5. 里程碑不可执行：M-01..M-04 均有三行齐全的验收契约，用例计数连续（M-01 188、M-02 203、M-03 219、M-04 219）。
6. 必须提前验证的重大风险：监听受限、真实 Agent 误拉起、HMR 共存均已有验证安排，无需前置。

结论：**设计定稿**。`status` 由 `设计中` 改为 `实现中`，`design-round` 为 1/15，`current-milestone` 为 M-01，里程碑表 M-01..M-04 完整（均为开放）。

提交：见 git log；若提交被拒则由 Autome 兜底。
