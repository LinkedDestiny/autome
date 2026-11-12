# M-02 · 实现 #2

范围：A-02、A-05；基线 ecb7229。上一轮审计 M-01 通过，无退回缺陷、无须逐字搬入的审计检查。本轮只推进 CLI，不改网关或协议。

## 实现与修复

- 新增 `task convert`，用现有任务解析器接受 id/slug 前缀，调用 `task.convert`，固定 kind=feature，仅带 --start 时传 true。
- JSON 保留完整 RPC 返回；普通输出三行，包含新任务、原任务取消说明和草稿/Loop 提示。
- 更新两种 help 入口共用的 USAGE、not_a_bug 下一步和 README。
- 新增 1 个 render 单测及 2 个真实二进制集成测试；原有 status 测试更新为新提示，未删除用例。
- 首次验收 2 通过、1 失败：--start 会启动新会话，复用 fake 序列导致第二个原任务未进入 not_a_bug。拆成独立 Sandbox 后通过；属本轮测试夹具修正，非上一轮审计退回。红输出 `.autome/output/impl2-cli-convert.txt`，绿输出 `.autome/output/impl2-cli-convert-final.txt`。

## 命令与数量

- 改动前 `cargo test -p autome-cli`：38 个，38 通过（25 单元 + 13 集成），0 失败/忽略；日志 `.autome/output/impl2-cli-baseline.txt`。
- 验收 `cargo test -p autome-cli task_convert`：3 个，3 通过，0 失败/忽略；相对基线过滤 0 个增加 3 个，来自上述三个测试函数，参数矩阵不重复计数。
- 格式检查、Clippy（`cargo clippy --all-targets -- -D warnings`）与 `git diff --check` 通过；输出 `.autome/output/impl2-fmt-final.txt`、`impl2-clippy-final.txt`。

## 标待审前自审清单（逐项执行）

### 1. 情形表逐行

- 提示行：render 单测断言 convert/cancel/reply 均在，旧 task new --brief 提示不在；真实 status 同样检查。通过。
- --start 白名单及请求：集成用例分别执行缺省与 --start，缺省为 draft，--start 非 draft。通过。
- USAGE：真实 `autome --help`、`autome help` 都包含 task convert。通过。
- JSON：断言 task.kind、task.state、converted_from、started 和 queue_position，保证并非只返回 task。通过。
- 普通输出：默认、--start 都断言恰好三行、原 id 取消说明及正确下一步。通过。
- README：检查新增 turn a not-a-bug task into a feature，与设计一致。

### 2. 失败分支

- 已取消原任务重复 convert：退出 1，stderr 包含「不是 Bug」。通过。
- 需求草稿 convert：退出 1，stderr 包含「只有 Bug」，原任务仍 draft。通过。
- 缺少任务参数：退出 2。不存在任务：退出 1。通过。
- 拒绝授权、读盘失败、超时、外部服务不可达：不适用，M-02 未新增这些分支；内核预检与事务失败属于已关闭的 M-01，CLI 沿用现有错误传播。

### 3. 全部迁移

- active/not_a_bug + 默认 convert → 新 feature/draft，原 cancelled。JSON 和普通输出均执行。
- active/not_a_bug + --start → 新 feature 非 draft（调度后 queued/active），原 cancelled。JSON 和普通输出均执行。
- cancelled + convert → 拒绝；feature/draft + convert → 拒绝且状态不变。
- 其它内核状态拒绝由 M-01 的 task_convert 矩阵覆盖，本轮全量 Rust 回归复验；CLI 不新增状态机。

### 4. 输入域边界

- 完整任务 id、非完整 slug 前缀分别成功；空串沿用解析器语义（当前仅有一个需求草稿，解析后被 kind 检查拒绝），不存在的 T-999999 退出 1；省略参数退出 2。
- --start 缺省与出现两种输入均执行；没有 CLI 布尔值参数域。
- 前导零、闰日、跨年、数字最大值：不适用，任务标识按既有字符串解析器处理，不解析数字/日期；附件、brief 边界属于 M-01。

## 宿主门禁与环境限制

`sh scripts/ci-local.sh` 已完整执行，退出 1，日志 `.autome/output/impl2-ci.txt`。Format、Clippy、Contract、Web lint/typecheck/build 通过。

- 首跑 Rust 阶段遇到上述测试夹具失败，停在 CLI 测试目标；修正后的全量另跑，以下方最终记录为准，不能把首跑当成全量通过。
- Desktop 总数 105，104 通过、1 失败、0 跳过；与上一实现轮总数 105 差 0，本轮没有改桌面测试。失败为 Electron DOM harness 子进程 status=null，无 stderr，具体退出原因未核实。
- Web 总数 301，271 通过、11 失败、19 跳过；与上一实现轮总数 301 差 0，本轮没有改 Web 测试。日志含 `listen EPERM: operation not permitted 127.0.0.1` 与 `EMFILE: too many open files, watch`，live/server 启动失败导致整组跳过。未删除、跳过或放宽任何断言。
- 最终源码验收再跑 `cargo test -p autome-cli task_convert`：3/3 通过，日志 `.autome/output/impl2-cli-acceptance.txt`。

门禁未全绿是明确保留的环境阻塞；M-02 独立验收已通过，可交审计，不能声称整个任务或宿主门禁已完成。H-01 真实项目人工验收未取证，仍保留在设计文档。

## 最终结果

- `cargo test --workspace --all-targets`：退出 0，9 个测试目标，1206 通过、0 失败、1 忽略，总数 1207。较上一轮 Rust 基线 1204 增加 3，全部来自本轮新增 CLI 测试；CLI 单元 25→26、集成 13→15。目标数与上一轮 `cargo test --workspace` 的 11 不同，是 `--all-targets` 不运行两个零用例 doctest 目标，实际用例未减少。日志 `.autome/output/impl2-workspace-final.txt`。
- 状态字段、implementation-round=2/15、M-02 待审/M-03 开放及证据路径检查通过；四项自审已在标待审前完成。设计文档仅新增本里程碑的一行证据指针，retro 仅追加一行。
- 本轮结论：M-02 待审；宿主 Desktop/Web 门禁仍需有权限的环境复跑。未标已完成，未推进 M-03。
