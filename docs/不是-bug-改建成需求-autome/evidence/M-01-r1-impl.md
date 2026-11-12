# M-01 实现 #1

## 范围与修复

实现 A-01、A-04：`task.convert` 的有序预检、默认草稿/显式排队、正文与附件/doc_refs/标题继承、单 SQLite 事务提交新任务与原任务取消/归档及三条事件。抽取任务记录构造函数和取消文件清理函数，保留 `task.create`、`task.cancel` 的既有路径；复用域层 Cancel 判定与指标汇总。附件缺失提示通过 TaskLayout 定位副本，兼容文档位于成员仓库的布局。未改 CLI、Web 网关、协议与其它里程碑。

没有上一轮审计，故没有需要逐字迁入并红绿复现的审计检查。本轮开发中修正了测试夹具没有初始 Git 提交、指标事件断言遗漏 `task.metrics`、测试多余借用及测试错误类型不实现 Debug 的编译问题；未削弱产品判据。

## 命令与结果

- `cargo test -p automed task_convert`：10 个用例，10 通过、0 失败、0 忽略。基线该过滤为 0，本轮 +10：dispatch 8、store 1、真实 Git/worktree 端到端 1。最终原始输出 `.autome/output/impl1-acceptance-final.txt`。
- `cargo clippy --all-targets -- -D warnings`：通过；`.autome/output/impl1-clippy-final.txt`。首次宿主门禁的多余借用警告已修复。
- `cargo fmt --all -- --check` 与 `git diff --check`：通过。
- 宿主门禁 `sh scripts/ci-local.sh`：执行记录 `.autome/output/impl1-ci.txt`。Rust 部分 1203 通过、1 忽略、0 失败，总数 1204；设计基线 1193 通过、1 忽略、总数 1194，本轮 +10，均来自以上新增用例，没有减少既有用例。
- 最终代码的 `cargo test --workspace`：结果见下方最终验证记录；输出 `.autome/output/impl1-workspace-final.txt`。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计情形/负向对照 | 执行检查与结果 |
|---|---|
| 默认/false 改建为草稿，保留字段与逐字正文 | dispatch 草稿用例：通过；默认缺 start 与 false 均覆盖 |
| start=true 排队或有空位启动 | dispatch start 用例逐一覆盖满额 queued、空位 active；端到端实际开工：通过 |
| 漏掉原任务取消 | 草稿与端到端均断言 cancelled、已归档：通过 |
| 两次提交留下半截任务 | 三种 ABORT 注入分别覆盖 brief 插入、tasks 更新、converted 事件；第二连接检查任务/brief/事件数量、旧状态与归档完全不变，重试成功：通过 |
| 归档标记提交后补置 | store 用例在 created 事件前触发器检查原任务归档已置位，另覆盖最后 updated 事件失败回滚：通过 |
| 停会话前已建任务 | gate 目录替换为普通文件导致 discard_node 失败，第二连接检查完全不变，移除故障重试成功：通过 |
| 仅检查 kind 不检查节点 | 状态矩阵含 await_info 等所有状态变体，均 TransitionRejected 且不变：通过 |
| brief 被 trim 或标题重拼 | 中文、尾随空格、换行、`{request}` 字节比较及标题/request 比较：通过 |
| 漏附件或取消后不清理 | 真附件源文件与新 worktree 副本字节一致；原 worktree/分支消失、文档归档内容保留：通过 |
| 错误列入 READ_METHODS | 草稿用例显式断言不在只读方法列表：通过 |
| 事件挂载/顺序/返回形状 | created(new)、converted(old)、updated(old) 的顺序和完整载荷、返回四字段：通过；随后既有指标汇总可追加 metrics 事件 |

以上为项目测试内的可区分断言；本轮未额外逐项篡改生产代码做 mutation campaign。

### 2. 失败分支

- 非 Bug、非 not_a_bug、已归档、项目移除：均执行，错误类别 TransitionRejected，保留状态/数量/事件。
- 参数缺失/错误类型/错误 kind、空 task_id、任务不存在：均执行，分别 InvalidParams / NotFound。
- 附件源丢失：有副本时提示恢复位置，无副本时不报虚假位置；两种均执行并检查无写入。
- 停会话读目录失败：普通文件占用 gate 目录，执行失败与修复后重试。
- 数据库提交失败：brief、原任务 UPDATE、converted、最后 updated 四个位置的 ABORT 均执行；失败时保留原 worktree/分支，移除注入后成功。
- 拒绝授权：不适用，本方法沿用既有写方法驱动锁与 IPC 边界，M-01 不新增授权流程。
- 超时、外部服务不可达：不适用，改建没有新增网络调用或外部服务；tick 沿用调度器报告行为。

### 3. 全部迁移

- bug active/not_a_bug → cancelled+archived，同时新 feature → draft（start 缺省与 false）：通过。
- bug active/not_a_bug → cancelled+archived，同时新 feature → queued（满额）或 active（有空位）：分别通过。
- draft、queued、requeued、active/locate、active/await_info、paused、stopped、failed、cancelled、done、已归档：拒绝且原状态不变，分别执行。
- feature → 拒绝；项目移除 → 拒绝；已改建的原 Bug 再次调用 → 拒绝：通过。
- 提交/预提交失败 → 无新任务、原 not_a_bug 且未归档、事件不变；修复故障 → 成功：通过。
- 既有取消、reply 确认 Bug、pause/stop/extend_budget 拒绝等用例保留，纳入全量 Rust 回归。

### 4. 输入域边界

- start：缺省、false、true 均执行；字符串、数字、null 拒绝。
- kind：feature 接受；缺省、bug、null 拒绝。task_id 缺省/非字符串拒绝，空串/不存在返回 NotFound。
- brief：中文及尾随空白、`{request}`、None、历史空串、65536 字节均逐字保留；改建不重新施加创建时校验。
- 附件/doc_refs：单元素真实路径及引用已执行；无附件/无正文由 store 用例覆盖。
- 前导零、闰日、跨年：不适用，接口没有数字或日期输入域，task_id 为不解释语义的字符串。

## 最终验证记录

宿主门禁最终未全绿：首次 Clippy 警告已修复并单独复验通过；Desktop 总计 105，104 通过、1 失败，失败为 Electron DOM harness 子进程 status=null，具体退出原因未核实。Web 总计 301，271 通过、11 失败、19 跳过；日志包含 `listen EPERM: operation not permitted 127.0.0.1`、`EMFILE: too many open files, watch`，live 测试启动 hook 超时。没有绕过或删除失败测试。设计轮未给 Desktop/Web 全量基线总数，无法声称其执行总数差为零；本轮未改这两套测试，文件层面新增用例为 0。Web lint、typecheck、build 与 Contract 通过。需要宿主允许 Electron 启动与本机监听的环境复跑完整门禁；这些阻塞不等于协议失败。

M-01 本身的 10 个验收用例及四项自审均已完成，标为待审，不标已完成；CLI/Web 产品行为留给 M-02/M-03。

最终工作区 `cargo test --workspace` 已完成：11 个测试目标，1203 通过、0 失败、1 忽略，总数 1204，较设计基线 1194 增加 10。最终 fmt、Clippy 与 diff whitespace 检查均通过。新增用例总数未因参数矩阵扩展改变，矩阵均在已有的 10 个新增测试函数内执行。
