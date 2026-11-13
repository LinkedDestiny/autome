# M-05 · 实现 #7

## 范围与修复

- 增加 task new --after（重复开关、逗号列表、id/slug 前缀）、after 列表/--none、queue move 的三种位置参数。依赖在同一次 task.create 写入。
- ls 将 Queued/Requeued 按项目和 queue_position 放入末尾排队段；status 输出依赖当前状态和 dashboard 排队组；补齐三个事件文案及帮助。
- 初次 deps_cli_queue 红：移动成功但 ls 仍输出 T-2、T-3、T-4。定位到 project.get 缺少设计 §6 已规定的 queue_position；本轮补齐每项目一次 queued_tasks 查询和字段，CLI 同时断言 JSON 位置及文本顺序。这是完成 M-05 所需的只读输出修复，没有推进 M-06。
- 首次编译测试误用了 set_state，已改用现有 set_task_state。
- 上轮审计 M-04 通过，没有退回缺陷，因此没有需要逐字迁移的审计检查。

## 验收命令与计数

- cargo test -p autome-cli deps_cli_：7 通过，0 失败，0 跳过（bin 3 + CLI 集成 4）。
- cargo test -p autome-cli --test cli：28 通过，0 失败，0 跳过。
- 本轮验收去重总数 31；两条命令执行 35 次，其中新增 4 个 CLI 用例重复执行。M-05 设计基线为 CLI 集成 24 个，本轮增加 4 到 28；新增前缀基线 0，本轮增加 7 到 7。旧测试断言未修改。
- 上轮 M-04 的验收范围为 9 个用例；本轮去重 31，表面差 +22 来自换成 CLI 验收范围（24 个既有 CLI +7 个新增），不是声称在上轮那 9 个用例上新增 22 个。
- 原始输出：.autome/output/m05-deps.txt、m05-cli.txt。未运行 ci-local.sh 或宿主快检命令；只用 rustfmt 格式化本轮变动的 Rust 文件。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §7 / M-05 情形 | 检查与结果 |
|---|---|
| 创建时用 id 和 slug 前缀声明依赖 | deps_cli_create 两种入口均保持 queued；ls JSON after/blocked_by 正确，账本没有 task.after_set |
| 重复 --after 与逗号分隔 | deps_cli_create 重复开关和 8 项列表成功，去重后单依赖 |
| after 修改及清空 | deps_cli_after 先清空再按 slug 设置，读回 [] / [T-1] |
| queue move top/before/after | deps_cli_queue 逐次验证三种方向的 JSON queue_position、ls 顺序及等候标注 |
| 非排队原顺序；排队末尾；费用脚注 | 原有 CLI 24 个回归通过，含 cost_cli_ls_appends_cost_and_a_footnote；新测试校验排队段及全部队列行 |
| ls --json | deps_cli_create/queue 校验数组对象及新增字段，不改 JSON 外层形状 |
| status 单任务依赖 | deps_cli_status 覆盖无依赖、Draft、Cancelled、Done 的依赖行；Done 用隔离 store 状态夹具检查显示，不声称真实合并验收 |
| status 总览 | deps_cli_status 覆盖空队列不出段、有队列位置及阻塞标注；JSON 与文本一致 |
| 短状态及三个新事件 | deps_cli_events 检查九种短状态、等待批准、取消/失败阻塞、三种移动、设置/清空依赖 |
| 帮助和参数解析 | deps_cli_usage / deps_cli_args 检查帮助、等号形式、重复参数、缺少值 |

### 2. 失败分支

- deps_cli_create：创建自引用（预测下一 id）和不存在 id 均退出 1、中文 stderr、任务数量不变；自引用在 resolve_task 阶段即拒绝尚不存在的预测 id。
- deps_cli_after：列表与 --none 同给或都不给、额外位置参数均退出 2；自引用/环退出 1，互斥拒绝后原依赖保持。
- deps_cli_queue：位置参数 0 个/2 个、重复位置参数、未知子命令退出 2；自己作参照退出 1。
- 空输入：空列表、开头/末尾空项退出 2；值型开关缺少值解析失败。
- 拒绝授权、读盘失败、超时、外部服务不可达：不适用；M-05 未新增这些协议分支，CLI 调用进程内已有 Client/RPC，已有错误传播不变。

### 3. 全部迁移

- CLI 不新增状态机；本轮边的迁移为无依赖→有依赖→清空、Draft 带依赖→Queued 阻塞→清空后离开 Queued，deps_cli_after 逐一执行。
- 排序三个方向 top/before/after 全部实际执行并断言结果；均不改变阻塞条件。
- 显示状态从依赖 Draft→Cancelled→Done 逐项检查；Done 为测试夹具，不触发调度。
- JSON 与文本通道均覆盖。底层调度的完整状态矩阵属已关闭 M-01～M-04，本轮未更改调度状态迁移。

### 4. 输入域边界

- 空串、前后逗号空项、单依赖、8 项重复输入、9 项重复输入、重复 --after 均由 deps_cli_create 执行；8 接受、9 拒绝（按输入项数）。
- id/slug 唯一前缀两种解析方式通过；位置参数零个、一个、两个和同名参数重复均执行。
- 日期、闰日、跨年、数值前导零：不适用；此入口接受任务名称/前缀，不解析日期或数值。

## 负向对照

临时修改生产代码后逐条运行项目内既有新增测试，随即恢复原文件；未保留注入代码。

1. ls 使用 created_at 而非 queue_position：cargo test -p autome-cli --test cli deps_cli_queue，1 失败（顺序断言）；原始输出 m05-negative-order.txt。
2. after 允许列表与 --none 同给：cargo test -p autome-cli --test cli deps_cli_after，1 失败（退出码断言）；原始输出 m05-negative-none.txt。
3. 创建先去掉 after，再调用 task.set_after：cargo test -p autome-cli --test cli deps_cli_create；检查启动状态和无 task.after_set 契约。1 失败（实际提前进入 active，预期 queued）；原始输出 m05-negative-atomic.txt。

恢复后三项生产改动全部撤回，再跑 cargo test -p autome-cli deps_cli_：7 通过、0 失败、0 跳过（m05-restored.txt）。git diff --check 通过。负向对照及恢复复验不增加独立用例数。

结论：M-05 待审；没有阻塞。宿主快检、全量门禁以及 H-01/H-02 人工验收未在本轮运行。
