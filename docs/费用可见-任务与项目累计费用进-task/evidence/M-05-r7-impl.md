# M-05 实现轮 #7

结论：CLI 实现与专项验收通过；全量门禁因环境失败，M-05 保持开放，不标待审。implementation-round 为 7/25，reopen 保持 0。

## 改动与修复

- `render::usd` 固定两位小数；`cost_cell` 在有无计费数据会话时加星号，旧内核缺字段时返回空串。
- `status <任务>` 在状态下一行显示累计费用、已结束会话数与无计费数据会话数；无会话显示零值，缺 cost 时不打印。
- `ls` 每行追加费用，仅存在星号时打印脚注；`status` 运行中行在状态之后、会话与重试片段之前插入费用。四个 JSON 通道沿用内核原样输出。
- USAGE 补充 `[budget]` 配置说明。
- B-07：既有 `bug_loop_task_get_reports_the_fix_round_for_bugs_only` 的整对象键集合仅补 `cost_usd`、`cost_sessions_without_cost`。符合设计 §5、任务约束 7；其余断言不变。
- CLI 夹具通过 `insert_session` 后调用 `set_session_metrics` 写入费用，避免误以为 insert 会保存 metrics；未改生产持久化逻辑。

## 审计检查原样复现与回归

上一轮发现 B-07 的检查本来就在项目测试体系中，因此保留原测试，不复制第二份。

命令：`cargo test -p automed --test end_to_end bug_loop_task_get_reports_the_fix_round_for_bugs_only`

- 修改前原样运行：1 个用例，0 通过 / 1 失败，键集合多出费用字段；日志 `.autome/output/m05-baseline-red.txt`。
- 只补上述两个期望键后：同一个用例通过，日志 `.autome/output/m05-baseline-green.txt`。新增用例数 0。

## 验收命令与用例总数

1. `cargo test -p autome-cli cost_cli_`：5 个用例全通过（render 2、真实二进制 CLI 3），对设计过滤前缀基线 0 增加 5。日志 `.autome/output/m05-cli.txt`；负向对照还原后同命令仍 5 个通过，日志 `.autome/output/m05-cli-final.txt`。
2. `sh scripts/ci-local.sh > .autome/output/m05-ci.txt 2>&1`：退出 1。Format、Clippy、Rust 全量、Contract、Web lint/typecheck/build 通过；Desktop、Web test 失败。
3. `npm test --prefix apps/desktop` 独立复跑：退出 1，同样在 Electron real-DOM harness 启动时 SIGABRT，105 个用例中 104 通过、1 失败；日志 `.autome/output/m05-desktop-retry.txt`。
4. `git diff --check` 通过；负向注入已全部还原，格式检查再次通过。

与上一轮 `.autome/output/m04-ci.txt` 的全量基线对比（总数包含失败、ignored/skipped，不重复计入专项复跑）：

| 层 | 上一轮总数 | 本轮总数 | 差与来源 | 本轮结果 |
|---|---:|---:|---|---|
| Rust | 1195 | 1223 | +28：CLI 新增 5；B-07 解除后继续执行原有 prompt_rendering 16 与 real_cli 7 | 1222 通过、0 失败、1 ignored |
| Desktop | 166 | 105 | -61：Electron 启动中止，未执行内部 DOM 断言；不是删用例 | 104 通过、1 失败 |
| Web | 302 | 302 | 0：未新增或删除用例 | 272 通过、11 失败、19 skipped |
| 合计 | 1663 | 1630 | -33 = +5 新增 +23 原有后续套件 -61 未执行 DOM | 1598 通过、12 失败、20 ignored/skipped |

总数下降意味着验收不完整，不能以 CLI 全绿代替宿主门禁全绿。桌面 SIGABRT 的具体系统原因未核实；日志确认进程启动失败。Web 三个 suite 的初始化失败，日志含 `listen EPERM: operation not permitted 127.0.0.1` 与 `File watcher error: EMFILE: too many open files, watch`；gateway-http 的 11 条因 listen EPERM 失败。未修改测试、跳过条件或监听配置来绕过环境限制。

## 标待审之前的自审清单（已执行，门禁未通过所以不标待审）

### 1. 情形表逐行

- 设计 §2 第 1 行：三个已结束会话 1.25 / 2.5 / None，另有运行中会话 99；状态下一行精确为 `  费用      $3.75（3 个会话，其中 1 个无计费数据）`，通过。
- 第 2 行：空会话任务精确输出 `  费用      $0.00（0 个会话，其中 0 个无计费数据）`，通过。
- 第 3 行：两个任务列表分别以 `  $3.75*`、`  $0.00` 结尾，通过。
- 第 4 行：含星号时末行精确等于规定脚注；空列表与单个零会话任务没有星号、没有脚注，通过。
- 第 5 行：运行中行包含 `运行中 · 实现  · $3.75*  · claude #4`，费用位置和原会话片段通过；原重试片段追加逻辑未改。
- 补充规则：`cost_cell({})` 返回空串的单测通过；缺顶层 cost 不打印费用行由 `got.get("cost")` 条件分支静态核对。四条 JSON 命令的费用/计数断言均通过。

### 2. 失败分支

- 本里程碑未设计授权、读盘、超时、外部服务调用的新失败分支：不适用，CLI 沿用现有 RPC 错误处理。
- 空输入/缺字段适用部分：空任务列表、零会话任务、缺费用字段均按上述测试检查通过。
- 全量遇到的 Electron 与 Web 启动失败已如实记录，不将其归为产品通过证据。

### 3. 全部迁移

- 状态迁移不适用：本里程碑仅渲染，不增加或修改任务状态机。
- 多通道：`status <任务>` 文本/JSON、`ls` 文本/JSON、`status` 文本/JSON、`project ls --json` 全部运行并断言费用字段。`project ls` 文本不加费用是明确非目标。
- Draft 零会话、有历史费用、Active/Implement 运行中三种展示夹具均已覆盖；运行中会话即使夹具携带 99 美元也不进入累计。

### 4. 输入域边界

- 金额 0、3.75、1234.5 的两位小数格式；缺字段；无计费计数 0/1；空集合、单任务、两个任务、混合已结束与运行中会话均通过。
- 日期、前导零、闰年与最大预算不适用：本里程碑不解析这些输入，预算域已属于 M-03；CLI 金额展示没有新增最大值约束。

## 负向对照

`.autome/output/m05-mutations.sh` 顺序注入并恢复四种错误，每次均为实际测试断言失败（退出 101），不是编译错误：

- `{v:.2}` 改为 `{v}`：两个 render 用例失败，日志 `m05-negative-format.txt`。
- 无计费会话数错用总会话数：status 精确行失败，日志 `m05-negative-count.txt`。
- 禁止打印脚注：列表末行断言失败，日志 `m05-negative-no-footnote.txt`。
- 无星号的非空列表也打印脚注：无星号控制断言失败，日志 `m05-negative-extra-footnote.txt`。

上述日志均位于 `.autome/output/`。还原后专项 5 个全绿；完整门禁的产品源码与还原后的源码一致。

## 剩余工作

在支持 Electron 启动、本地 TCP 监听和文件监听的环境重新执行 M-05 全量验收，确认 Desktop 恢复 166 个、Web 302 个全部通过后再标待审。人工验收 H-01–H-03 本轮未执行，仍保留原清单。未启动下一个会话。
