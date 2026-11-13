# M-02 · 实现 #2 · 待审

## 范围与实现

- 完成方案 §2、§3：启动时保存上限、SQLite v6 可空列及旧行兼容、纯函数边界判定、TimedOut 生命周期、整个进程组 TERM/轮询/KILL/确认。
- 超时事件先落账再发信号；无 PID 不收割并去重记录 unconfirmed；晚到 PID 单次读盘补写；终止失败/未确认保持 Running。
- Exited/Vanished 优先正常收割；已有 timeout 事件时必须重新终止并确认残留组，随后按 TimedOut 收割。普通/ Bug 结果均提供中文超时原因。
- 契约交叉已明确：M-02 用例⑨要求 next_round 不计超时行，原安排却在 M-03；本轮提前实现该查询条件，M-03 仍验证完整重派预算、限流跳过及注入语义。本轮不实现 M-03 注入或 M-04 展示。
- 上一轮审计 M-01 通过，没有退回缺陷或需逐字迁入的审计检查。

## 验收命令与结果

原契约完整执行，退出码 0：

```sh
cargo test -p autome-domain session_timeout && cargo test -p automed --lib session_timeout && cargo test -p automed --test end_to_end session_timeout && cargo test -p automed --test end_to_end -- stopping_a_task a_non_zero_exit model_not_found a_rate_limited_round && test "$(git diff "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs | grep -c '^-[^-]')" = 0
```

分段原始输出在：
- `.autome/output/m02-accept-domain.txt`：2 个通过。
- `.autome/output/m02-accept-lib.txt`：4 个通过，6.79 秒。
- `.autome/output/m02-accept-e2e.txt`：4 个通过，5.43 秒。
- `.autome/output/m02-accept-regression.txt`：5 个通过，7.05 秒。
- e2e 相对 main 分叉点删除/修改原行数为 0，原有 e2e 未改。

直接相关的额外回归：
- `cargo test -p automed --lib store::tests`：53 个通过（账本迁移、读写和轮号受本轮修改影响）；`.autome/output/m02-store-regression.txt`。
- `cargo test -p autome-domain session::tests`：15 个通过（既有 classify/生命周期回归）；`.autome/output/m02-session-regression.txt`。
- `cargo test -p automed --lib stopping_an_implausible_pid`：1 个通过；`.autome/output/m02-invalid-pid.txt`。
- `git diff --check` 通过。执行了 `cargo fmt --all` 格式化；未运行宿主快检或全量门禁。

**数量核对：**契约总数 15，基线 5 个旧回归 + 0 个 timeout 用例，本轮新增 10 个，净增 10。上一实现轮 M-01 实跑 8 个，本轮契约比其多 7 个，但两轮选集不同：移出上轮配置 8 个，纳入本轮新增 10 个与旧回归 5 个。加额外回归共执行 84 个用例实例，去除 store 重复 1 个、domain 重复 2 个后是 **81 个不同用例**（10 新增、71 既有）；比上一轮实跑 8 个多 73，来自选集变化而非声称新增 73 个测试。最终全部通过，0 失败、0 ignored；过滤项不算执行。

## 修复与验证过程

- 首次编译发现错误类型转换、事件返回值类型及测试字符串/宏位置错误，已修正后通过。
- 恢复测试最初让 shell 孤儿自行回收，terminate_group 不能及时确认组消失，正确返回 false；测试改为组长与残留成员分别持有 Child 并 wait，仍保留真实“组长已消失、组仍存在、残留成员忽略 TERM”的输入。未放宽生产确认条件或断言。
- 扩展账本回归初跑 52 过、1 失败：既有 v4 迁移测试硬编码期望版本 5。本轮已增加 v6，故调整回退夹具去掉新列、断言当前 SCHEMA_VERSION；复跑 53/53。旧行保留及所有旧任务状态断言未删除。
- 本轮无审计退回检查；上述实现调试不冒充上一轮审计回归。

## 标“待审”之前的四项自审

### 1. 情形表逐行

| 设计条款/情形 | 检查及结果 |
|---|---|
| 新会话冻结上限 | e2e frozen 配置 5→120，事件仍 limit=5，终止成功；通过 |
| 旧行无上限 | store 真 v5 行迁移后 None；scheduler effective_limit 当前配置 30、坏配置默认120；通过 |
| 活跃任务会话 | e2e Intake 超时，原节点不变、pending_retry 存在；通过 |
| Bug 会话 | read_bug_outcome(TimedOut) 断言中文超时；后续完整重派归 M-03；通过 |
| Paused / Requeued | 恢复单测两种输入分别使用 Paused / Requeued，真实终止后完成收割；通过 |
| 终态/手动复盘 | 不适用新增分支：超时确认在公共入口完成，既有 detached_retro 路径未改；本轮仅代码核对，不声称专门实跑复盘 e2e |
| Onboarding | 不适用：设计明确排除，真实启动路径不登记账本 |
| Exited 优先 | 正常文档+退出标记，回拨到6分钟前，仍 Exited 且无 timeout；通过 |
| 超时中断恢复 | 130 标记、无标记且日志11分钟前两种输入，后者先断言 classify=Vanished；残留组必须消失、生命周期 TimedOut、轮号不占用；通过 |
| TERM / KILL | 带子进程组 TERM <1秒；忽略TERM至少3秒后KILL且全组消失；不存在组立即成功；通过 |

### 2. 失败分支

- 超时：4 个 e2e/恢复测试均验证，事件含 session_id/minutes/limit。
- 缺失 PID：连续两次 tick 无 finish/retry/timeout，unconfirmed 只写一次；恢复 PID 后补写并终止，真实三个进程均消失。
- 终止出错：恢复测试传 pid=1 触发拒绝，验证会话仍 Running、无 retry、真实残留组仍存在；然后合法 PID 完成收割。
- 终止返回 false：按设计⑨明确允许代码审查；两条恢复/超限路径共用 match，Ok(false) 直接 return，finish/sweep/retry 均在其后。未添加生产测试后门。
- 配置读盘/解析失败：坏 TOML 下旧会话回退120，冻结上限仍优先；通过。
- 空/无法解析 started_at：e2e 空串保持 Running、不记录超时；通过。
- 拒绝授权、外部服务不可达：不适用，本里程碑无授权或网络依赖；系统信号错误通过 fail-closed 分支处理。

### 3. 全部迁移

- Running + 未达上限 → Running：e2e 正常 tick。
- Running + 不可解析时间 → Running：空串检查。
- Running + 超限 + 无PID → Running/unconfirmed：两次 tick 去重，随后晚到PID。
- Running + 超限 + 确认终止 → TimedOut → 原节点 pending_retry：三个阻塞 e2e。
- 已退出 + 超限 + 无timeout事件 → Exited：正常结束 e2e。
- 非Running(Exited/Vanished) + timeout事件 + 确认终止 → TimedOut：恢复单测两种输入。
- 有timeout事件 + 终止错误/未确认 → Running：错误实跑，false分支代码审查（见上）。
- 普通无事件 Vanished、正常 Exited、Running 心跳保持：domain session::tests 既有分类用例通过。
- 人工 stop → Killed：两个 stopping_a_task 旧 e2e 通过；原停止调用方不改。
- TimedOut → 普通/Bug Crashed：两个 read_outcome 断言；读文档标志 false。
- TimedOut 不占 next_round；非超时原轮号计数：恢复测试及 store 旧轮号测试通过。
- 完整重试用尽、包装注入、恢复后重派：属于 M-03，不在本轮声称通过的范围。

### 4. 输入域边界

- over_limit：0、5、120、u32::MAX；limit*60、前1秒（非零上限）、u64::MAX；均通过，乘法在 u64 无溢出。
- PID：i32::MIN、-1、0、1拒绝；合法组、已消失组；通过。
- limit_minutes：旧NULL、u32::MAX写读；缺省120、项目30、冻结5；通过。
- 生命周期 JSON 精确匹配并往返、is_running/should_parse_document=false；通过。
- 时间空串与6分钟回拨、日志11分钟静默；通过。
- 前导零、闰日、跨年：不适用新增输入解析，本轮消费现有RFC3339时间解析函数，未新增日期字符串接口。

## 交付边界

M-02 标待审，implementation-round=2/20。M-03/M-04 保持开放；全量门禁和宿主快检由 Autome 执行，本轮未运行。无人工验收阻塞。
