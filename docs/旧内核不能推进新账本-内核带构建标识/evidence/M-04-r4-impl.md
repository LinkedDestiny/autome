# M-04 实现证据 · 实现 #4

## 范围与修复

- 本轮只推进最小开放里程碑 M-04；上一轮 M-03 审计通过，无退回缺陷，因此没有须逐字搬入的审计检查。
- 将原 automed 帧循环移入 `entry::serve_stdio() -> i32`，两个入口共用；`autome core` 在 Client 打开之前分发，stdout 只输出帧。相应调整“仅入口解析 home”的既有架构检查，允许共享入口 entry.rs。
- 常驻 run 按拒绝消息去重，仍保持进程；消息变化重新提示，恢复时提示一次。once/until-idle 被拒返回 1；JSON 输出独立 older_kernel 对象，恢复为 null。普通持锁提示与旧内核拒绝分开。
- doctor 文本/JSON 显示当前与账本标识，旧内核拒绝不改变环境探测决定的退出码；写命令继续使用内核统一拒绝。
- 全部进程验证显式设置临时 AUTOME_HOME 与 AUTOMED_DB_PATH，未触及真实账本、安装目录或其它 worktree。

## 命令与结果

| 命令 | 结果 | 原始输出 |
|---|---|---|
| `cargo test -p autome-cli --test cli kernel_guard` | 最终 8/8 通过，0 失败、0 忽略 | `.autome/output/m04-restored.txt` |
| `cargo test -p autome-cli --test cli run_json` | 5/5 通过 | `.autome/output/m04-run-regression.txt` |
| `cargo test -p autome-cli --test cli help_and_version_need_no_ledger` | 1/1 通过 | `.autome/output/m04-help.txt` |
| `cargo test -p automed --lib only_the_entrypoint_resolves_the_real_home` | 1/1 通过 | `.autome/output/m04-home.txt` |
| `git diff --check` | 退出 0 | 无输出 |

本轮唯一用例总数 **15**（验收 8 + 相关回归 7），不重复累计重跑。上一轮 M-03 验收基线 4，本轮比其 **+11**：换出 M-03 的 4 个用例，换入 M-04 新增 8 个和既有回归 7 个；并非全量套件增长 11。相同选择范围在本轮改动前是 7，新增 8；CLI 文件注册总数由 32 变为 40，无删除或忽略。未运行全量门禁、宿主快检或整个项目测试。

首跑 7 通过、1 失败（`.autome/output/m04-accept.txt`）：新增帧测试错把 payload 当作顶层；实际协议为 outcome.payload。修正断言路径后 8/8 通过，未改产品协议。

负向对照：临时将 `if older_message.as_deref() != Some(message)` 改为 `if true`，原样运行 `cargo test -p autome-cli --test cli kernel_guard_run_says_it_once_and_stays_up`，1 个用例失败（`.autome/output/m04-negative.txt`），证明能抓重复日志。随后恢复原代码，最终验收 8/8 通过。临时变异未保留。

## 标待审之前的自审清单

1. **情形表逐行**（方案 11、12）：常驻文本与 JSON 均在 0.05 秒间隔下观察约 2 秒，进程仍在，拒绝只出现一次；文本总行数不超过 3，无“另一个驱动”。once/until-idle × 文本/JSON 四组合均返回 1 且单条拒绝；恢复文本/JSON 各提示一次。task new、approve、merge、rerun 均返回 1、stderr 逐字为统一拒绝，前后任务列表完全一致。core 当前/旧身份与 version 一致；doctor 空账本、当前、被拒三种情况均检查。
2. **失败分支**：旧内核拒绝由上述四个写命令及三种 run 模式覆盖；core 的数据库路径指向目录时退出 1、stdout 为空；无效命令帧返回 error；空 stdin 正常退出、无帧污染。拒绝授权、外部服务不可达、超时不适用：本里程碑没有新增授权、网络或超时分支（已有环境探测行为未修改）。所有新常驻测试的等待均有截止时间与子进程清理。
3. **全部迁移**：文本/JSON 两通道都执行：首次拒绝→重复同一拒绝不再输出→账本换为另一标识后重新输出→另一进程显式接管后恢复→新内核抬高记录后再次拒绝→SIGINT 正常退出。once/until-idle 的拒绝退出亦覆盖。正常 JSON 驱动、错误输出、收会话、管道关闭与中断由 5 个既有 run_json 用例回归。stdio 入口正常响应、错误帧、EOF、打开失败均执行。
4. **域边界**：测试覆盖低位零填充旧标识 `0000001-20200101T000000Z-000000000000`；覆盖空串、畸形覆盖值及最大七位计数候选 `9999999-29991231T000000Z-ffffffffffff` 被忽略，真实身份不变；空账本记录为 null，单帧与空输入均执行。闰日/跨年不适用：本轮不修改 M-01 的日期解析/比较逻辑。

## 交接

M-04 待审；实现轮计数固定为 4/35。下一步由审计轮独立复验 M-04，通过后才可推进 M-05。提交在当前分支尝试；不启动下个会话。
