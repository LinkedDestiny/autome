# 实现 #10：合并前门禁修复（M-03 / M-06）

本轮遵循修复轮指令，不推进新里程碑。开场六个里程碑均已由审计关闭，保持原状态；implementation-round = 10/30。审计 #9 没有退回项，本次依据宿主在 0e2e20549c28 上的实际失败修复。未运行完整门禁或宿主快检组合。

## 失败、红绿证据与修复

原始输出均在 `.autome/output/r10/`；宿主日志只读核对自 `/Users/dannie/project/autome-v2/.autome/output/gates/T-97/merge_gate-0e2e20549c28.log`，摘取索引保存为 `host-failures.txt`。

1. Rust 原用例原样执行：`cargo test -p automed --test end_to_end setup_run_in_flight_is_collected_under_its_own_fingerprint_when_the_watched_file_changes -- --exact --nocapture`，退出 101，1 例失败（`rust-before.txt`）。事件、指纹与顺序断言均通过，末尾目录断言发现额外 `setup-worktree.passes`。根因：M-03 的共享 `gate::start` 增加 passes/report 文件，setup 手工收尾只删除 started/pid/exit，遗漏新增文件。补齐两者删除，保留诊断 log。检查 `gate::Job::clear`、`discard_except`、scheduler 的清理调用均已覆盖新增文件，setup 是漏项处。追溯设计 B.3 的作业文件生命周期与 X-3、V-7。
2. Scripts 原样运行 `node --test scripts/test/*.test.mjs` 在本会话 NO_COLOR=1 下 44/44 通过（`scripts-before.txt`），不能据此排除宿主故障。宿主失败的是 `web-groups` 的 project 标签断言，实际 3 文件 / 28 Web 用例全绿。继续执行 `env -u NO_COLOR FORCE_COLOR=1 node --test scripts/test/web-groups.test.mjs`，退出 1，1 例失败（`groups-color-red.txt`），复现同一断言。仅 FORCE_COLOR=1 而继承本会话 NO_COLOR=1 仍通过（`groups-red.txt` 实为绿色探索记录）。根因：Vitest 无色标签是 `|unit|`，彩色模式使用背景色标签；剥离 ANSI 后兼容两种标签，并将项目名与对应文件关联断言，额外钉住 28 例总数。原有项目测试现参数化为无色/彩色两例，持续覆盖宿主触发条件。追溯 M-06 三 project 同次启动验收、D-R4。
3. 原 Rust 回归已在项目体系中，未删改原指纹/事件顺序/目录断言；成功、失败两条路径都让真实 setup 命令写 report，失败路径补断言只保留 log、没有 passes/report/started/pid/exit。成功路径仍检查目录恰好一个 log。

## 最终验证与用例总数

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test -p automed --test end_to_end setup_ -- --nocapture` | 18/18，通过，退出 0 | `setup-final.txt` |
| `cargo test -p automed --lib setup -- --nocapture` | 23/23，通过，退出 0 | `setup-lib.txt` |
| `cargo test -p automed --lib gate_report` | 6/6，通过，退出 0 | `gate-report.txt` |
| `node --test scripts/test/web-groups.test.mjs` | 2/2，通过，退出 0；每例真实运行 3 文件 / 28 Web 用例 | `groups-green.txt` |
| `node --test scripts/test/*.test.mjs` | 45/45，通过，退出 0 | `scripts-final.txt` |
| `git diff --check` | 退出 0 | 无输出 |

同口径顶层用例基线：18 setup e2e + 23 setup 单元 + 6 gate_report + 44 Scripts = 91；本轮实际 92/92 通过，失败 0、跳过 0，差 +1，仅来自 web-groups 从一个环境变为两种颜色环境。Rust 数量未变，只加强两例的文件清理断言；单跑 web-groups 的 2 例已包含在 Scripts 中，不重复加总。Scripts 内部还运行 65 次 Web 用例（两种颜色各 28，ci-quick related 9），涉及 28 个不同 Web 用例，独立列明而不混进顶层总数。上一审计轮的 Web 子集是 33 例、全量是 843 例；本轮只修复相关路径，没有重跑其全量与性能基准，不能拿 92 与 843 比较宣称覆盖增加。

## 标待审之前的四项自审（修复轮同样执行）

1. **情形表逐行**：M-06 的 unit/kernel/exclusive 三行在两种颜色环境下逐行关联 paths/ci-config/load-context 文件，并验证每次 3 文件、28 用例；均通过。B.3 作业文件收尾：成功保留 log、删除其它作业文件；失败保留 log、删除其它文件且不保存成功指纹；两条真实 setup 回归通过。无需推进或重新关闭里程碑。
2. **失败分支**：setup 非零退出及运行中输入改变后失败，e2e 均通过；setup 超时（124 与准备命令提示）、各 gate 节点 success/failure/timeout 分支由 setup 单元子集通过；读盘缺失、目录不可作文件、空输入由 setup 边界例通过；gate_report 缺失、错误格式、超限与 partial 非全集通过的 6 例通过。拒绝授权、外部服务不可达不适用：本次仅作业本地收尾与测试输出识别，不新增授权或外部请求。
3. **全部迁移**：setup Pending→成功收取→输入变化重跑→Ready、Pending→失败、暂停→恢复同作业、停止→恢复新作业、取消、Vanished→重启、缺失 running 归属→重启、exit 已发布但 PID 未退→继续等待，均在 setup_ e2e 子集通过；未配置与 workspace 跳过路径也通过。三 checkout 的成功/失败与 setup→gate 接续由 e2e/lib 子集通过。Web 无状态机，unit/kernel/exclusive 三通道均按真实 runner 检查。
4. **输入域边界**：watch 不存在/目录/空文件/空列表，由 setup 边界例通过；report 缺失/非法/超限，由 gate_report 通过；颜色域显式 FORCE_COLOR=0 + NO_COLOR=1、FORCE_COLOR=1 + 删除 NO_COLOR 两端通过，未用条件跳过。日期、闰日、跨年、数值最大值不适用：本轮未改变相应输入域。

## 限制与交接

快检与完整门禁均由 Autome 在提交后/合并前重跑，最终结论以宿主结果为准。未重跑 Web 全量、性能基准、人工 H-01/H-02；原结论没有更新。尝试只读 `ps` 被沙箱拒绝，故不声明本轮进程盘点通过。只写本 worktree；没有改配置、协议或真实账本，没有启动下一会话。
