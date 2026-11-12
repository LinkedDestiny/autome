# M-01 · 实现 #1

范围：只推进 M-01（配置、纯指纹、SetupFailed 迁移），预算 1/20。此前无审计轮，无需搬运审计回归用例。

## 实现说明

- domain 新增 SetupConfig、默认 30 分钟、ProjectConfig/ResolvedConfig.setup；resolve 保留项目配置。
- setup 模块提供 SHA-256 指纹（命令、排序去重的路径及内容摘要，缺失用 absent）、needs_run、路径校验和三个 Checkout 标签。
- SetupFailed 在通用控制触发器段处理：仅运行中的 Active 非等待节点进入 Failed，保留节点、命令、退出码与日志，不触发其它 Action。Bug 分发继续转入该通用处理。
- config_io 按设计解析 setup，空白命令禁用且忽略其余字段；watch 去首尾空白；超时与 gate 一致，正整数超过 u32::MAX 时饱和到 u32::MAX。
- scheduler 仅补 trigger_name 的穷举分支以保持编译；宿主作业与调度留给 M-02，不提前推进。

## 验收命令与结果

原始输出均在 `.autome/output/`，以下测试是恢复全部负向变异后的正确代码结果。

| 命令 | 用例总数 / 结果 | 与设计记录的上一基线差 | 输出 |
|---|---|---|---|
| `cargo test -p autome-domain` | 355，通过；另 0 doc tests | 345 → 355，+10（D1–D10） | m01-domain.txt |
| `cargo test -p automed --lib` | 611，通过 | 606 → 611，+5（C1–C5） | m01-lib.txt |
| `cargo test -p autome-domain setup` | 10，通过，过滤 345 | 新增子集 0 → 10 | m01-domain-setup.txt |
| `cargo test -p automed --lib setup_section` | 5，通过，过滤 606 | 新增子集 0 → 5 | m01-lib-setup.txt |
| `cargo fmt --check` | 退出 0 | 不计用例数 | m01-fmt-check.txt |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | 不计用例数 | m01-clippy.txt |

合计独立用例 966，较基线 951 增加 15；过滤运行是重复验证，不重复计数。现有用例未修改；各总量测试失败 0、忽略 0。基线来自设计记录的 2553923，本轮没有回退检出重跑旧版本。

## 标待审之前的自审清单

### 1. 情形表逐行

本里程碑对应设计 §1、§2 纯函数部分、§5；检出记录落盘表与门禁调度表属于后续里程碑。

| 情形 | 已执行检查与结果 |
|---|---|
| 未配置 setup | C4、D10：None |
| setup 不是表 | C3：拒绝 |
| command 缺失或非字符串 | C2：拒绝并包含需要 command |
| command 空串、空白串 | C2：None，watch/timeout 错误不再校验 |
| 完整 command/watch/timeout | C1：去空白后的命令和路径、12 分钟全部匹配 |
| watch 省略 | C4：空列表 |
| watch 非数组、元素非字符串、空路径、绝对路径、父路径段 | C3：逐项拒绝 |
| timeout 省略或非法 | C4：30；C3：零、负数、小数、字符串、布尔均拒绝 |
| save_project 保留 setup | C5：修改 parallel 后保存、重读，setup 完全相等 |
| 配置传递 | D10：有/无 setup 均按原值 resolve |
| 文件内容或路径变化 | D1：指纹不同 |
| 命令变化、watch 空列表 | D2：指纹不同 |
| 缺失与空文件 | D3：指纹不同 |
| 路径重排与重复 | D4：指纹相同 |
| 无记录、不同记录、相同记录 | D5：依次重跑、重跑、不重跑 |
| 三类 Checkout | D5：worktree、verify、main 标签一致 |
| 失败原因序列化 | D8：完整 JSON 字段和反序列化一致 |

### 2. 失败分支

- 配置拒绝：C2/C3 逐项覆盖上述无命令、错误类型、非法 watch、非法超时。
- 准备失败：D6 完整保留退出码 7；D9 完整保留超时码 124，均进入 Failed、Action::None。
- 非运行态拒绝：D7 逐项验证（详见迁移）。
- 读盘失败：本里程碑无 I/O，不适用；D3 已验证输入 None 的 absent 语义，宿主实际读盘留后续实现。
- 授权拒绝、外部服务不可达、实际命令超时：不适用，本里程碑没有进程、网络和授权入口；这里只验证超时失败码的状态传递。

### 3. 全部迁移

D6 遍历 Node::ALL + Node::BUG 的全部 22 个节点，其中 18 个非等待节点逐一执行 Active × SetupFailed → 同节点 Failed(Setup)，Action::None。

D7 对全部 22 节点分别执行 Paused、Stopped、Requeued × SetupFailed → 拒绝（66 组）；四个人工等待节点 AwaitDesignApproval、AwaitMerge、AwaitInfo、NotABug 的 Active × SetupFailed → 拒绝；另 Draft、Queued、Done、Cancelled、Failed 五态分别拒绝，共 75 组拒绝检查。

D9 经真实 Bug 分发入口，对 Locate、ReproCheck、FixGate 分别执行 SetupFailed → 同节点 Failed(Setup)，不被 Bug 节点拒绝逻辑截断。D5 覆盖 needs_run 的全部三种记录关系。

### 4. 输入域边界

- C2：空串、纯空白命令（包含无效其它字段时仍禁用）。
- C3：空/空白 watch 元素、单元素、父路径位于首/中/尾、绝对路径；普通文件、./lock、包含 ... 的合法路径与中文路径允许。
- C3：timeout 为 0、负数、错误类型拒绝；1、u32::MAX、i64::MAX 验证正值与饱和边界，沿用 gate 语义。
- D1–D5：零文件、单文件、重复文件、空内容、缺失内容、空记录；指纹 64 位十六进制输出长度。
- 日期、闰日、跨年不适用：本里程碑无日期输入域。

## 六个负向对照

逐个临时变异，执行指定用例后立即恢复原文件；六次均为实际断言失败（退出 101），不是编译错误。输出 `.autome/output/m01-negative-<编号>.txt`。恢复后再次执行上述全部验收，全部通过。

| 编号 | 临时错误 | 执行的测试（cargo test 对应 crate，automed 加 --lib） | 红 → 绿 |
|---|---|---|---|
| D2 | 指纹去掉命令输入 | setup_fingerprint_changes_when_the_command_changes | 1 失败 → 通过 |
| D3 | absent 使用空内容摘要 | setup_fingerprint_tells_a_missing_file_from_an_empty_one | 1 失败 → 通过 |
| D9 | 控制触发器前先拒绝 Bug 节点 | setup_failure_fails_a_bug_task_at_its_bug_node | 1 失败 → 通过 |
| D7 | 允许 Paused 接受 SetupFailed | setup_failure_is_rejected_unless_the_task_is_running | 1 失败 → 通过 |
| C2 | 空命令仍进入配置校验 | setup_section_without_a_command_is_refused_and_an_empty_command_means_none | 1 失败 → 通过 |
| C3 | 允许父目录路径段 | setup_section_refuses_absolute_parent_or_non_string_watch_entries | 1 失败 → 通过 |

## 边界与交接

未改项目 setup 配置或协议，未运行 e2e、Desktop/Web 和 ci-local：它们在 M-02–M-04 的验收范围，clippy 已编译所有 targets。无环境阻塞。M-01 提交独立审计，M-02–M-04 仍开放。
