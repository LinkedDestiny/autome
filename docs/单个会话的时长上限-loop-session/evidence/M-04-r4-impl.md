# M-04 · 实现 #4

范围：R-5、方案 §6，当前 worktree 内完成查询字段、CLI 展示与技术设计同步。上一轮审计 M-03 通过，没有退回缺陷或需要逐字迁入的审计检查。

## 实现

- `session_json` 统一接收上下文和项目；`task.get`、`session.log`、`dashboard.get` 的运行会话均使用同一路径。`timed_out` 从生命周期派生，`limit_minutes` 只调用已有 `effective_limit`；运行时长复用内核 `secs_since`，结束会话的时长字段为 null。
- CLI 只打印内核字段：运行分钟数/上限、超时行和 `session.timeout` 事件。失败面板保持已有 `session_crashed.detail`，新增测试钉住超时原因。
- 技术设计 §7.3、§17 同步生命周期、终止确认、重试、轮号、睡眠时间及 Onboarding 范围。
- CLI 测试复用真实二进制与独立账本，新增 rusqlite 开发依赖用于回拨测试会话时间，无生产开关。

## 验收命令与结果

- `cargo test -p automed --lib timeout_visible && cargo test -p autome-cli timeout_visible`：6 个用例全部通过（automed 3、CLI render 1、CLI 集成 2）。原始输出：`.autome/output/m04-lib.txt`、`m04-cli.txt`；最终恢复后的输出见 `m04-final.txt`。
- 本轮过滤器的上一轮基线为 0，本轮新增 6，差 +6，分别对应设计①～⑥。
- 直接相关回归：`cargo test -p autome-cli --test cli status_shows_latest_five_sessions_in_chronological_order`，1 个既有用例。合计唯一用例 7；相对上轮 M-03 的 18 为 -11，原因是按协议切换验收范围：上轮 13 个重试专项及 5 个 e2e 本轮未重跑，本轮改跑 6 个可见性专项及 1 个既有会话展示回归，没有删除或跳过测试。
- 最初测试编译因 `DispatchError` 不实现 Debug 以及一次测试调用替换位置错误失败；仅修正测试错误处理后通过，不改生产错误类型。
- 未跑全量门禁、宿主快检或人工验收 H-01/H-02，交由宿主/用户执行。

## 标 `待审` 之前的自审清单

1. **情形表逐行**：设计 §6 / M-04 ①～⑥逐条执行。① task.get/session.log 超时 true、上限 5，正常退出 false；② 三入口运行 37/120，当前配置 30 不覆盖启动快照，旧会话 None 回退 30；③ dashboard.waiting 失败理由原样含超时和 5 分钟；④事件行逐字断言；⑤真实 CLI 输出 37/120；⑥真实 CLI 输出超时上限 5。均通过，技术设计两处已同步。
2. **失败分支**：日志文件不存在时 session.log 仍返回空日志及正确会话字段；旧会话配置解析失败回退默认 120；超时失败任务在 waiting 展示明确 detail，均通过。拒绝授权、外部服务不可达不适用：本轮没有授权或外部服务调用，也未改变这些路径。
3. **全部迁移**：本轮是只读投影，不新增状态迁移。执行 Running → TimedOut 后检查 running=false、timed_out=true、elapsed=null；正常 Exited 对照 timed_out=false；运行行 running=true、timed_out=false。三个查询通道均验证运行字段，两个历史查询通道验证结束字段；CLI 运行与超时两分支均执行，正常历史列表由既有回归覆盖。重试状态迁移属于已关闭的 M-03，本轮未改。
4. **域边界**：最小合法上限 5、默认 120、最大 u32、旧会话 None；空时间返回 null、未来时间返回 0；无日志返回空串；CLI 单条会话及既有七条历史只显示最新五条。均执行通过。前导零、闰日、跨年不适用：本轮无新增日期或配置解析器，沿用既有内核解析。

## 负向对照

临时修改生产代码，每次执行后恢复，均由新增项目测试抓住（不是只验证编译错误）：

- session.log 只返回 id、遗漏字段：`cargo test -p automed --lib timeout_visible_finished`，失败，日志 `m04-negative-log.txt`。
- 上限直接返回可空的 `s.limit_minutes`、跳过有效配置回退：`cargo test -p automed --lib timeout_visible_running`，失败，日志 `m04-negative-limit.txt`。
- CLI 忽略 elapsed_minutes、固定打印 0：`cargo test -p autome-cli timeout_visible_status_running`，失败，日志 `m04-negative-cli.txt`。

所有日志位于 `.autome/output/`；恢复后重新执行完整 M-04 验收与既有展示回归。负向对照及重跑不计为新增唯一用例。

最终复验：`m04-final.txt` 中 3 + 1 + 2 + 1 = 7 个唯一用例全部通过，0 失败、0 忽略；三项负向对照均为断言失败（退出码 101），恢复后全绿。`git diff --check` 通过。M-04 标为待审，implementation-round 为 4/20。
