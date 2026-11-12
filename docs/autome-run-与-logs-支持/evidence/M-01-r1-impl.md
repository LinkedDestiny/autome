# M-01 · 实现 #1

结论：M-01 验收通过，待独立审计。实现轮 `1/10`，没有上一轮审计退回或需逐字迁入的回归检查。

## 改动

- Store 新增 `StoredEvent` 和只读 `events_since_with_at`；旧三元组、四元组接口不动。RPC `events.since` 仅增加 `at`。
- `run --json` 输出紧凑事件行，保留 payload、subject，以真实任务查询判断 task_id，time 原样使用 at；不经过人类事件过滤。
- tick errors 逐条输出 JSON；仅因 until-idle 停止时输出 unfinished 任务投影；once、SIGINT 不输出 idle。三条驱动提示在 JSON 模式禁用。
- 继续使用项目 println!/print_stdout，包含 flush 和 BrokenPipe 退出处理；未修改 logs、人类渲染、现有 14 条集成用例。

## 命令与结果

- `cargo test -p automed --lib events_since && cargo test -p autome-cli`：通过。分别保存为 `.autome/output/m01-events-final.txt` 与 `m01-cli-final.txt`。过滤后 automed 4 例（基线 3，增加 RPC 时间字段检查 1）；CLI 45 例全过（单元 28、集成 17），相较基线 39 增加 6：事件组装 3、完整 JSON 驱动 1、断管/驱动占用 1、tick 错误/SIGINT 1。
- 另新增 Store 时间与查询边界测试 1 例，不匹配 `events_since` 过滤器，由 workspace 全量执行。这解释了 automed 全量相较基线应增加 2 而过滤验收仅增加 1。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`：通过，见 `m01-format-final.txt`、`m01-clippy-final.txt`；最终宿主门禁再次执行这两项。
- `git diff --check`：通过。
- 初版异常测试的夹具未产生 tick error，断言失败；修正为 Active/Gate 任务的非法项目配置后 `cargo test -p autome-cli run_json_tick_error` 通过。属于本轮测试夹具修正，无上一轮审计红绿要求。原始记录 `m01-branches.txt`、`m01-error.txt`。

## 标待审之前的自审清单

### 1. 情形表逐行

- 任务主体：`run_event_task_identity` 用非 T- 前缀真实任务验证；项目主体：`run_event_project_preserves_subject` 验证 null 与 subject 保留。
- 时间：`run_event_time_is_stored_at`、Store 与 RPC 用例验证原时间，集成逐事件与 RPC at 相等。
- 全部事件/顺序/payload：集成从驱动前 seq 起抓取，以事件列表长度、逐项 seq、payload 对照，包含人类过滤掉的 task.head。
- tick 错误：非法配置引发真实 scheduler report，第一行含 error 且关联任务，stderr 空。
- idle：完整任务到 await_design_approval，收尾 state 与 status 一致；空账本输出 tasks=[]。
- once：空账本同时给 once 和 until-idle，无收尾。SIGINT：收到首条 JSON 后发信号，5 秒有界等待成功退出且无 idle、stderr 空。
- skipped：持有真实 driver lock 时 run --json --once 成功退出，stdout/stderr 均空。
- 管道关闭：先关闭唯一读端，再执行 run --json --until-idle，退出 0 且 stderr 空。
- verbose：源码确认 JSON 分支在 render/verbose 判断前 continue，verbose 不影响事件输出；未单独增加重复进程用例。

### 2. 失败分支

- 参数错误：非法 interval，退出 2、stdout 空。
- 配置读取/解析失败：真实非法 TOML 触发 tick error，以上异常集成通过。
- 输出失败：真实 BrokenPipe 检查通过。
- 拒绝授权、外部服务不可达、业务超时：不适用，M-01 是进程内内核读取/驱动，设计没有新增授权、网络或超时分支。
- 空输入：空账本 until-idle 与 once 均已验证。M-02 的日志文件不存在、未知 runtime 分支不属于本轮。

### 3. 全部迁移

- 启动 → 多次 tick/分页事件 → idle：完整沙箱工作流检查。
- 启动 → once 停止：空账本检查（同时传 until-idle，证明 once 优先）。
- tick error → JSON 行 → SIGINT 停止：异常集成检查，保留已输出完整行，无 idle。
- 驱动锁占用 → skipped → once 停止：真实锁检查。
- 输出 → BrokenPipe → 安静成功退出：真实管道检查。
- 参数解析 → 命令失败：退出 2 无 stdout。
- 不新增业务状态机；原 idle 状态迁移单元用例及现有端到端回归保持通过。

### 4. 输入域边界

- 0 事件、单事件、limit=0、after_seq 等于末事件：Store 单元检查。
- 历史闰日时间 `2000-02-29T23:59:59Z`：原样保留；无需做日期运算，跨年/前导零属于原字符串保留。
- payload 内换行：纯函数测试确认 JSON 序列化只有一行，payload 无损。
- 起点 `u64::MAX`：驱动集成经过现有 latest_seq 初始化路径，保留既有查询语义。
- 任务 id 不按前缀猜测；项目 id 不误作 task_id。

## 验证限制

人工清单的真实 CLI/真实模型会话未执行；本轮用真实 autome 二进制与项目既有 fake-cli 沙箱验收。设计负向对照对应的断言已核对，未额外注入四种生产代码变异。未修改范围外的 Desktop/Web 测试或环境配置。

## 全量回归补充

`cargo test --workspace --all-targets` 完成：总计 1209 例，1208 通过、0 失败、1 ignored。分组为 CLI 28+17、domain 384+1 ignored、automed 650、end_to_end 106、real_cli 16、工具用例 7。日志 `.autome/output/m01-workspace-final.txt`。上一轮未提供全 workspace 的实跑总数，不能声称独立复验过旧基线；按本轮净新增 8 例推算原总数为 1201（差来自 CLI 6 + Store/RPC 2），既有用例未删除或修改。

状态块字段唯一性、`1/10`、M-01 待审/M-02 开放已通过本轮格式检查。

## 宿主门禁结果

`sh scripts/ci-local.sh` 最终退出 1（168 秒），日志 `.autome/output/m01-gate-final.txt`：Format、Clippy、Rust 全量、Contract、Web lint/typecheck/build 均通过；Desktop 105 例（104 通过、1 失败），失败为 Electron DOM harness 子进程退出码 null；Web 301 例（271 通过、11 失败、19 跳过），4 个测试文件失败，含 `listen EPERM ... 127.0.0.1` 与监听器 `EMFILE`。没有上一轮 Desktop/Web 完整基线，本轮未改这两层测试，源代码用例数增量为 0；不能把跳过当作通过。宿主限制下未取得这两层通过证据，需要可启动 Electron/监听本地端口的环境复验。首次门禁另有尚未修正的本轮异常测试失败，已在最终门禁消除。

全量后最后只为异常测试显式加入 fake Claude/Codex 二进制环境，避免测试依赖宿主安装；重新跑 fmt、clippy 和 CLI 全套 45 例均通过，未重复未改动的全 workspace/前端测试。
