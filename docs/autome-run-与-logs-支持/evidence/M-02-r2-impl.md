# M-02 · 实现 #2

## 范围与实现

实现 `logs --json [--follow]`：原始 `.jsonl` 按运行时复用 `stream_render`，输出 `{line,type,text,event}`；保留未换行字节直到补齐或会话结束；所有输出走已有断管处理。增加白名单和帮助文本。不修改旧测试、渲染器、包装脚本及人类模式输出。

源码核实发现设计假设有误：`Store::list_sessions` 按 `started_at DESC` 返回，`task.get` 不反转，旧 `logs.last()` 选的是最早会话。为满足 M-02 最新会话验收，JSON 分支默认取 `first()`，人类模式保留 `last()`；设计 §4 就地澄清。

## 命令与结果

- `cargo fmt --all`：通过。
- `cargo test -p autome-cli`：通过，日志 `.autome/output/m02-acceptance.txt`。总数 **50 = 32 单元 + 18 集成**，0 失败、0 跳过；上一轮基线 **45 = 28 + 17**，净增 **5**，来自 4 个行转换/行边界单元用例和 1 个真实二进制集成用例。既有测试未修改或删除。
- 开发中首次运行暴露白名单未放行 `--json`；修复后暴露默认会话选择与设计假设不符，修正 JSON 默认选择后上述验收全绿。中间一次既有 SIGINT 用例首个错误来自会话而非任务，断言失败；未改动它，最终 CLI 验收通过。
- 上一审计为 M-01 通过，未退回实现缺陷，故不存在需逐字搬入的审计回归检查。

## 标待审之前的自审清单

1. **情形表逐行**：
   - Claude assistant 原始对象保留、text 为 hi there；hook_started 的 text 为 null：`stream_claude` 与集成通过。
   - Codex agent_message 为 Hello!、turn.completed 用量含 in 36111、turn.started 为 null：`stream_codex` 与集成通过。
   - 非 JSON stderr 行 event/type 为 null、保留文本；空白行跳过但计行号：`stream_text_and_input_boundaries` 与集成通过。
   - 默认最新会话、显式 review 会话、前缀 live-：集成通过；已结束会话普通读取与 -f 结果完全一致。
   - 原始非空行数与输出数一致；每一行独立 `serde_json::from_str` 解析；断管退出 0 且 stderr 为空：集成通过。
2. **失败分支**：
   - 运行中 `.jsonl` 尚未创建：真实二进制成功且 stdout 为空。
   - 已结束 `.jsonl` 缺失：退出 1、stdout 为空、stderr 提示没有原始流。
   - 读盘失败：用同名目录替代文件，退出 1、stdout 为空，不吞成空日志。
   - 非法 UTF-8：lossy replacement；空输入：不输出，单元通过。
   - 未知 runtime：持久化 `Session.runtime` 为封闭 Runtime 枚举，合法账本及 task.get 无法产生未知字符串；JSON 入口仍以 Runtime::parse + CliError::Failed 拒绝，静态核对。此项没有伪造 RPC 绕过类型约束的实跑证据。
   - 拒绝授权、外部服务不可达、超时：不适用，本路径读取本地账本/文件，无授权或网络请求及超时协议。
3. **全部迁移**：
   - running + 文件未创建 → 零行；running + 一整行及半行 → 仅整行；running + 补换行 → 两整行；ended + 无换行尾部 → 两行：集成通过。
   - ended + 空白行/尾部 → 跳过空白但尾部 line=2：集成通过。
   - 字节消费位置 0→8→13（两行半，再结束补尾）：`stream_line_boundaries` 通过；单换行与空文件边界通过。
   - ended + follow → 输出后退出：集成通过。运行中持续 follow 与 Ctrl-C 的真实端到端取证仍属于设计 B-04/人工清单，本轮未声称实跑；代码检查中断走现有信号标志，退出不补运行中半行。
4. **输入域边界**：
   - 空串、空白、CRLF、单换行、无换行尾部、UTF-8 未完成字节、非法 UTF-8：单元通过。
   - JSON null/数字/数组、对象 type 非字符串：type 为 null；原始可解析值保留。集成检查默认/显式/前缀会话选择。
   - 日期、闰日、前导零、跨年、业务数值最大值：不适用，日志内容透明保留，不解析这些业务域。

## 尚未取证

人工清单中的真实 Claude/Codex 会话与 jq 管道未执行；自动验收使用既有 fake CLI 包装脚本及 `stream_render` 中同形样例，并用逐行 serde_json 解析验证 JSONL 契约。

## 全量回归与格式核验

宿主门禁中的 Rust 全量总数 **1214**（1213 通过、0 失败、1 个既有 ignored），对比上一轮 1209 **净增 5**，全部来自本轮 CLI 用例；domain/automed/端到端/适配器用例未改变。Format、Clippy、Contract 已通过。Desktop **105** 例（104 通过、1 失败），相对上一轮总数差 **0**，失败为 `electron dom harness` 的子进程退出码 null，与上一轮同类，不能视作真实 Electron 验收通过。

状态字段唯一性、implementation-round 为 2/10、M-02 待审且 M-01 仍为已完成的格式检查通过；`git diff --check` 通过。

`sh scripts/ci-local.sh` 最终未通过（387 秒），完整输出 `.autome/output/m02-gate.txt`。Web **301** 例（271 通过、11 失败、19 跳过），4 文件失败；与上一轮总数差 **0**，失败含 `listen EPERM ... 127.0.0.1`、监听器 `EMFILE`。Web lint/typecheck/build 均通过。此类宿主限制未消除，不声称门禁全绿；需在允许 Electron/本地端口监听的环境独立复验。任务要求的 `cargo test -p autome-cli` 与 Rust 全量均通过，M-02 提交待审。
