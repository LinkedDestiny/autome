# M-02 · 实现轮 3/20

结论：验收通过，M-02 待审。本轮仅推进 M-02。上一轮审计 M-01 通过，没有退回缺陷或需要逐字迁入测试体系的判别检查。

## 实现

- 新增宿主 setup 入口：按检出识别作业，先收在途结果，再计算最新 watch 指纹；成功只保存启动指纹，失败不更新记录，收尾只保留日志。
- 懒建检出与任务输入移到会话准备之前；记录匹配直接启动，不匹配借用持久化待重试队列等待。准备失败触发 SetupFailed，不作为门禁失败回流。
- 自审修复固定作业路径复用的收尾竞态：保持原 wrapper 发布顺序；setup 收到 Done 时，若旧 wrapper PID 仍存活则返回 Pending，等其完成最后一次 PID 删除再复用路径。避免下一作业的 PID 被旧 wrapper 删除，不阻塞 tick。最终实现重新验收 lib/e2e。
- 保留 gate::poll 两参数接口和门禁文案，新增 poll_as 与只读 running；停止/取消清理 setup 前缀作业。CLI 打印三种准备事件及检出标签。
- M-03 门禁前调用点、M-04 主检出与工作区跳过事件未在本轮推进。

## 验收命令与基线

原始输出全部位于 .autome/output/。所有命令在本 worktree 执行。

| 命令 | 输出 | 结果 |
|---|---|---|
| cargo test -p automed --lib | m02-lib-final.txt | 614 通过，较上一轮 611 增加 3（L1/L2/L4） |
| cargo test -p automed --test end_to_end | m02-e2e-final.txt | 88 通过，较上一轮 80 增加 8（E1–E5/E14/E15/E16），0 失败、0 忽略 |
| cargo test -p autome-cli | m02-cli.txt | 20 + 11 通过；bin 较上一轮 19 增加 1（R1），集成目标 +0 |
| cargo test -p automed --test end_to_end setup | m02-setup-final.txt | 8 通过，80 filtered out；为上述 e2e 子集 |
| cargo test -p autome-domain | m02-domain.txt | 355 通过，较上一轮 +0；复核既有配置/指纹/失败状态边界 |
| cargo fmt --check | m02-fmt-check.txt | 通过 |
| cargo clippy --all-targets -- -D warnings | m02-clippy-final.txt | 通过 |
| git diff --check | 无输出 | 通过 |

M-02 受影响目标上一轮基线共 721 个（611 + 80 + 19 + 11）；本轮实测 733 个（614 + 88 + 20 + 11），净增 12。设计原列 11 个，额外 1 个综合 e2e 用例补齐自审中的暂停、取消、作业消失、旁记缺失和宿主读取边界，未删改现有用例。过滤 setup 的 8 个是这 88 个的子集，不重复计入总数。后续里程碑的预期总数同步增加 1。

首次编译发现拆分初始化后 launch_session 仍需读取任务卡，已修复。首次新用例运行 6/8 通过、2 失败：E1 使用了错误节点名 await_design；E14 夹具把 lock 的换行带入运行记录，误计为 3 次。更正为实际节点 await_design_approval，并将记录中的文件换行折为同一行。未削弱准备次序、指纹归属或运行次数断言。

第二次全量为 86/88 通过、2 失败：E1 的 shell PWD 是 macOS canonical 路径（/private/var），与临时目录别名 /var 不同；E4 的测试仅等 PID 文件存在，读到了重定向创建后尚未写入的空文件。分别改为 canonicalize 比较、等待可解析 PID；停止后的 PID 消失也采用有界条件等待；同类检查发现新增 L1 也读取 PID，统一改为等待可解析内容。此处为测试夹具修复，产品实现未变。

收尾竞态修复后的全量首次为 87/88：综合用例在底层 exit 标记出现后立即要求 Ready，忽略了新明确的 wrapper 收尾 Pending。测试改为有界轮询准备入口直到 Ready，再检查记录，最终重新全量验收。

## 标待审前自审清单

### 1. 情形表逐行

| 设计条款/情形 | 检查与结果 |
|---|---|
| §3/§4.1 首次、无成功记录 | E1：真实 git worktree、wrapper、sh 写入 deps/stamp；模型替身检查标记；成功事件早于 session.started |
| §2/§4.1 后续会话指纹相同 | E1 跑到设计批准停顿点，多次会话仅一次准备；记录匹配不产生新准备事件 |
| §3 运行中 watch 变化、成功 | E14：两次启动与两次成功；结果各归属于对应启动指纹；第二次完成后才启动会话；只余一组 log |
| §3 运行中 watch 变化、失败 | E15：只启动一次，exit 3 失败，未写成功指纹、未启动会话 |
| §6 非零退出及人工修复后重跑 | E2：exit 7、kind=setup、命令与真实日志匹配，无 session/gate；修好脚本后 rerun_from intake 第二次成功 |
| §7 重启接续 | E3：在准备仍运行时 recover，准备只运行一次，随后启动会话 |
| §7 停止后恢复 | E4：脚本 PID 被杀、setup-* 全清，恢复后第二次启动成功 |
| §7 暂停/取消 | 补充综合用例：暂停保留原作业，恢复不重启；取消停止并清作业 |
| §3 收尾窗口 | 综合用例给真实存活 wrapper 发布完成标记，下一 tick 仍无 result/session，不提前复用路径 |
| §3 Vanished/旁记丢失 | 补充综合用例：死 PID 无 exit、完成但无 running 旁记，都不写虚假成功，重新准备 |
| §3 启动空窗 | L4：仅 started 为真；10 秒边界无 PID 为假；活 PID 为真，exit 或死 PID 为假；无 started 为假 |
| §3 作业隔离、超时 | L2 只删 setup 前缀保留 gate；L1 两种文案与 124，杀进程 |
| §8 打印 | R1：started、成功/失败 result、skipped，worktree/verify/main 三标签、日志路径 |
| §9 未配置零差异 | E5 配 gate 跑到 done，三种 setup 事件均为 0；其余原有测试保持原文 |

### 2. 失败分支

- 非零退出：E2/E15 已执行，断言任务失败、退出码、日志、无会话/门禁、无成功记录。
- 超时：L1 以 poll_as(job, 0, "准备命令") 实跑真实 sleep，退出 124，中文说明正确，PID 死亡；同用例原两参数 poll 仍为门禁文案。
- 读盘失败：综合用例把不存在的路径和目录按 absent 处理，与预置指纹一致则 Ready；创建空文件后 Pending，成功记录改变。
- 空输入：综合用例空 watch、M-01 既有 domain/config 用例覆盖空 command、空路径和空文件。本轮复跑 domain 全量及 automed lib 全量。
- 拒绝授权/外部服务不可达：不适用独立的内核分支。准备由用户配置的 sh 执行，不新增授权或网络 API；脚本返回非零统一走上述失败路径。本轮未访问外部包源，不声称真实 npm 网络验收。

### 3. 全部迁移

- 无记录 → Pending → Running → 成功记录 → Ready → 会话：E1。
- 指纹相同 → Ready（不新增事件）：E1、综合用例。
- Running + 输入变化 → Pending → 旧指纹成功 → 新作业 Pending → Ready：E14。
- Running + 输入变化 → 非零失败 → Task Failed：E15。
- Pending → 非零失败 → 清标记 → rerun → Pending → Ready：E2。
- Pending → 超时 124：L1；失败状态迁移由 D6/D9 全节点覆盖，D7 拒绝非运行态（复跑 domain）。
- Pending → 暂停（作业继续）→ 恢复（原作业）：综合用例；Pending → 停止 → 清理 → 恢复新作业：E4；Pending → 取消 → 清理：综合用例。
- Pending → core recover → 收原作业：E3。
- 作业消失/旁记丢失 → 丢弃无归属结果 → 重启：综合用例。
- 门禁前、verify、main 的迁移不适用本轮调用点，分别属于 M-03/M-04；未用本轮证据宣称完成。

### 4. 输入域边界

- 空 watch：综合用例实跑 Ready；单元素 watch：E1/E2/E14/E15；缺失/目录/空文件：综合用例。
- 去重/排序、命令改变、相同/不同记录、缺失不等于空文件：既有 domain D1–D5，355 全量通过。
- timeout=0 是直接 poll 的测试边界，验证即时超时；生产配置拒绝 0，沿用 M-01 C3（本轮 lib 全量）。
- started 的无文件、新写入、10 秒、活 PID、死 PID、exit 标记：L4。
- 日期、闰日、前导零不适用：本里程碑没有日期或数字文本输入解析；timeout 的配置解析属于已关闭的 M-01。

## 范围与限制

真实网络安装与人工 UI 验收仍按设计 H-01/H-02 留待用户确认。Desktop/Web 全量由 M-04 执行，本轮没有声称它们通过。未改项目配置、协议文本或其他 worktree，未推送远端。

最终受影响目标 733/733 通过，0 失败、0 忽略；另复跑 domain 355/355。最终 e2e 196.14s、lib 66.44s、setup 专项 33.66s，日志以表中 final 文件为准。
