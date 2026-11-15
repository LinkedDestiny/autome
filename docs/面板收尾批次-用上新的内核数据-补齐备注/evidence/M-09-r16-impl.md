# M-09 · 实现轮 16/65

结论：待审。完成 F 的进程清理、两处调度接入、两类事件与时间线文案；验收命令 45 例通过。未改 schema / 协议，未运行项目全量门禁或宿主快检。

## 实现与验收口径

- `residue.rs`：libproc 枚举、BSD 身份及控制终端、exe/cwd 组件归属、原路径/canonical 根；当前用户、非自身、非僵尸；每次信号与存活检查复核起始时间、归属、终端。TERM 共用 3 秒窗口，KILL 共用 1 秒窗口；拒绝信号仍存活进 survivors。失去归属或变成 tty/unknown 的进程不冒充已结束。
- 合并 `Node::Cleanup` 在删除前调用；取消 `discard_checkout` 在文档归档后、删除前调用。改建共用该入口；dispatch 先停止会话后取消/改建，不增加运行中扫描。
- 事件命令按 Unicode 字符截到 120；未知路径不按进程年龄豁免；TTY 带终端名。时间线按 kind 区分原因，旧/未知 kind 归 unknown，同时呈现混合原因和 survivors。
- 修正验收内部冲突：F.1 要求全机本用户路径未知进程告警，原 F.4/M-09 又写“无残留不写事件”。真实测试发现一个宿主 Chrome 扩展可执行文件被替换，exe 为 ENOENT，按 F.1 必须告警。保留 F.1，明确“无残留不写 stopped；也无全机未知进程才不写 unchecked”。真实空残留测试严格禁止 stopped、要求存在的 unchecked 均为 unknown 且枚举成功；另用空替身表和真实内存 Store 验证两类事件均不落库。没有按应用名、进程年龄豁免，也没有删除失败检查或增加环境跳过。设计契约同步这一口径，供审计独立判断。
- 上一轮审计 M-08 为通过，未退回 M-09；没有应逐字迁移的审计缺陷检查。

## 命令与结果

原始输出均在 `.autome/output/`，以下只写判据与结果。

| 命令 | 结果 / 输出 |
|---|---|
| `cargo test -p automed --lib residue && cargo test -p automed --test end_to_end residue_ && (cd apps/web && npx vitest run test/task-timeline.test.ts)` | 退出 0；18 + 3 + 24 = **45**，无失败/跳过；`m09-acceptance-final.txt` |
| `cargo test -p automed --lib` | 退出 0；最终 **1024** 通过，无失败/跳过；`m09-lib-final.txt`。前次 1022 通过后补 2 个失败/空表用例，再跑最终版本 |
| `cargo build -p automed -p autome-cli` | 退出 0；为关联测试对齐 CLI/内核构建版本；`m09-build.txt` |
| `(cd apps/web && npx vitest related --run app/components/task-timeline-model.ts)` | 17 文件 **118** 例，108 通过、4 失败、6 未执行；`m09-related-final.txt`。失败均是既有进程收尾检查 `spawnSync ps EPERM`；两个 live 文件 beforeAll 的 `listen EPERM` 导致 6 例未执行，不是新增 skip |
| `cargo fmt --all`；`cargo clippy -p automed --lib --tests -- -D warnings`；`git diff --check` | 均退出 0；定向检查 automed，不跑全项目 gate；`m09-fmt.txt`、`m09-clippy-final.txt` |
| `cargo test -p automed --lib residue`（最后一次变异还原后） | 18 通过；`m09-restored.txt` |

计数基线：改前同验收范围 residue 0、residue_ 0、timeline 22，共 **22 → 45（+23）**：新增 Rust 18、真实流程 3、Web 2。Rust 原库 1006 → 1024（+18），其中 13 项设计点名用例和额外 5 项边界/失败用例。改前 residue 过滤输出与 timeline 实跑分别见 `m09-baseline-rust.txt`、`m09-baseline-web.txt`，原 HEAD end_to_end 无 `fn residue_`。关联 Web 范围 118 含 timeline 24，不重复相加；其范围相对原代码只增加 timeline 2 例。最终选中用例去重为 **1024 + 3 + 118 = 1145**（1135 通过、4 环境失败、6 环境未执行），同范围改前 1122，增量仍为 23。上一轮 M-08 验收的 115 例是另一组命令，不能把切换里程碑造成的范围差当成用例删除。

首次真实测试另发现夹具目标 output 目录未创建，已补创建；首次关联 Web 的 1 个功能失败是 CLI 构建号落后于 automed，对齐构建后功能断言通过，仅上述沙箱限制保留。未把这些初次失败算进最终通过数。

## 标待审前自审清单（四项已执行）

### 1. 情形表逐行

| F.1–F.4 情形 | 检查与结果 |
|---|---|
| sweep 不命中 sweep-2；主检出不命中 | `residue_components_not_prefixes`；真实合并/取消均保留两处同名 sleep，通过 |
| /tmp 与 /private/tmp、根本身及根内路径 | `residue_canonical_tmp_alias` 在 /tmp 建根并用 canonical 路径匹配；cwd=根、exe=根内，通过 |
| exe 命中；cwd 命中（exe ENOENT 仍能确认） | `residue_executable_matches`、`residue_cwd_matches_even_if_executable_unknown`；真实两个进程事件分别 matched=exe/cwd，通过 |
| ESRCH、僵尸、自身、他人 uid | `residue_esrch_zombie_and_self_skipped`、`residue_other_uid_skipped_without_warning`，无信号/无他人告警，通过 |
| 路径未知，起始时间早/晚 | `residue_unknown_old_process_also_warns`、`residue_unknown_new_process_warns`，均只告警，通过 |
| 初始有终端；TERM 后取得终端 | `residue_tty_never_signalled`、`residue_tty_acquired_before_kill`；真实 script 伪终端仍活且只在 unchecked，通过 |
| PID 微秒起始时间变化；cwd 离开 | `residue_pid_reused_no_kill_or_survivor`、`residue_cwd_leaves_before_kill`，无后续 KILL/无 survivors，通过 |
| 合并；取消（改建共用取消入口） | 三个 residue_ 真仓库测试通过；调用点静态核对 dispatch 的 stop 在 discard 前 |
| 空残留/全机未知告警 | 真实无残留不写 stopped，ambient unknown 不隐藏；`residue_empty_table_writes_no_events` 两类事件均无，通过 |
| 时间线两种原因、混合、旧 kind | 新增 2 个 Web 用例逐项断言，通过 |
| 非 macOS | 平台分支返回单条 unknown、不发信号；当前 macOS 未做跨平台编译，产品只支持 macOS |

### 2. 失败分支

- 枚举拒绝、初始身份 EPERM、信号前身份查询 EPERM：`residue_query_failures_do_not_signal` 三组全部通过；初始 EPERM 跳过，另两组只告警。
- 路径 ENOENT、cwd 复核 EPERM：新/旧进程 unknown 和 `residue_term_exits_and_unknown_recheck` 通过；不对未知归属发信号。
- TERM 超时升级 KILL：`residue_term_then_kill` 通过，60 次等待后 KILL，再一次等待确认消失。
- 信号 EPERM 与 KILL 后仍活：`residue_permission_denied_survivor_and_command_boundary` 通过，80 次共享等待、无虚假 stopped、survivor 保留 EPERM。
- 空根/空进程表：各有独立用例通过。
- 外部服务不可达：不适用，F 不调用网络服务。事件落库错误仅 tracing 告警、继续清理（源码核对）；未声称做过磁盘损坏注入。

### 3. 全部迁移

已用上述替身及真实测试执行：扫描→忽略；扫描→unknown；扫描→tty；候选→TERM；TERM→退出；TERM→仍活→KILL→退出；TERM/KILL 拒绝→survivor；复核→PID 复用；复核→离开根；复核→取得终端；复核→无法归属/身份失败。取消和合并两条清理通道均真实执行。去重按 pid 的 BTreeMap 固定，复核退出保护不会被 kill(pid,0) 替代。

### 4. 输入域边界

已跑：空根列表与空路径、单进程、根路径自身、同前缀不同组件、canonical 别名、起始秒数 0/u64::MAX、同秒不同微秒、Unicode 命令 121→120 字符、未知/缺省 kind、混合 kind。日期、闰日、前导零不适用：本接口不解析日期或文本数值。真实 test process 的 stdout/stdin/终端由夹具显式设置，使用 setsid 避免继承开发终端。

## 负向对照（全已还原）

| 变异 | 检查结果 |
|---|---|
| 删除 tty 保护 | 单测失败；真实合并失败，stopped 错误出现第三个（伪终端）进程 |
| 按字符串前缀比较 | 单测失败；真实合并 `children.alive(sibling_process)` 失败 |
| 删除未知路径告警 | unknown 单测失败 |
| 把未知路径当命中（允许发信号） | 两个 unknown 单测失败 |
| 起始时间为 0 时豁免告警 | old_process 单测失败 |
| 去掉起始时间身份核对 | pid_reused 单测失败 |
| 时间线将 tty 统一作 unknown | 新 Web 文案用例失败 |

命令：单测分别运行 `cargo test -p automed --lib <上述测试名>`；真实对照运行 `cargo test -p automed --test end_to_end residue_merge_preserves`；Web 对照运行 `npx vitest run test/task-timeline.test.ts`。原始红证据为 `m09-negative-*.txt`、`m09-negative-e2e-*.txt`、`m09-negative-unknown-kill.txt`；最终还原绿证据见验收输出与 `m09-restored.txt`。所有变异仅作用于临时测试仓库中自行创建的进程。

## 隔离、限制与交接

- 本轮所有真实任务为 World 临时仓库/内存账本，复制的内核设临时 AUTOME_HOME/HOME，未访问真实账本或 launchd。
- 新测试 Drop guard 对自建 Child kill/wait；伪终端先结束 sleep，等 script 回收，再回收父进程。成功路径额外断言 Child 均退出、PTY pid 不再存活。失败变异路径同样经过 guard。
- 既有 Web teardown 在 ps 失败时仍 stop task/core；另用只读 libproc+sysctl 工具检查本轮创建的 **12 个 w7 临时根**（按本轮开始时间筛选），匹配存活进程 **0**，见 `m09-process-check.txt`。未按进程名杀任何宿主进程；未手动启动服务。
- 交宿主复验：`(cd apps/web && npx vitest related --run app/components/task-timeline-model.ts)`，具体受阻文件为 task-detail-real、task-docs-real、task-merge-real、task-timeline-real 的收尾检查，task-live-real、task-log-live 的 beforeAll/收尾。未改弱这些断言。H-06 的真实 iTerm/编辑器集成终端人工体验仍由用户确认。
- 提交：本轮按要求尝试 git add / git commit；若权限拒绝则提交由 Autome 兜底。
