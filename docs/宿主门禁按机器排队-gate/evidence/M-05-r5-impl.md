# M-05 · 实现 #5

范围：只推进 M-05；新增 `end_to_end.rs` 的 T1–T6 和共用夹具，不修改产品实现、旧测试、配置、协议或其它里程碑。上轮 M-04 审计通过，没有审计退回检查需要搬入回归体系。

## 命令与结果

- `cargo test -p automed --test end_to_end`：首次全量 112/112 通过，0 失败、0 忽略，136.22 秒；原始输出 `.autome/output/m05-full.txt`。最终版本复跑结果见下。
- `cargo test -p automed --test end_to_end gate_queue_`：6/6 通过（106 filtered out），首次修正后 14.74 秒，加强入队时间后 39.26 秒；输出 `.autome/output/m05-focused2.txt`、`m05-focused-final.txt`。最终版本复跑见下。
- `cargo fmt --all -- --check`、`git diff --check`：通过；格式检查输出 `.autome/output/m05-final-fmt.txt`。
- 用 `git show HEAD:crates/automed/tests/end_to_end.rs` 保存原文件，再以 Perl `index(new, old) == 0` 核对：原文件全部字节保留，只在末尾追加；原有 106 个运行用例（包含既有 gate/setup 用例）未删、未改、未改名。源文件里的 `#[test]` 字符串还包含脚本夹具，不拿文本匹配数量当运行数量。

用例总数与上轮基线之差：112 − 106 = **+6**，全部来自本轮 T1–T6，无替换、删除或跳过。workspace、clippy、桌面端与 README 验收属于 M-06，本轮未执行，不作为通过证据。

## 修复与负向对照

- 初次夹具误写节点名 `await_approval`，6 个等待均失败（92.57 秒，`.autome/output/m05-focused.txt`）。改成已有真实节点 `await_design_approval`；同时按真实 dashboard 形状从 `running[].task.id` 查任务。修正后 6/6 绿；这是新增测试夹具修正，不是产品缺陷。
- 临时将 domain 准入条件 `position == 1 && held < limit as usize` 改为 `position >= 1 && held <= usize::MAX && limit >= 1`，原样运行 T1：**红**，断言 `2 live jobs in one ledger`，0/1 通过，16.39 秒；输出 `.autome/output/m05-negative-admission.txt`。随后从备份原样还原 domain 文件，git diff 确认产品文件无改动，再运行最终正向验收。
- T3 使用磁盘测试账本，只改 B 的真实 `gate.queued.at` 为 120 秒前；回拨 A 的 `.started` 为 61 秒前，不改变 production 超时实现。A 退出 124、有内核终止行，B 的 `.started` 不早于 A `gate.result.at`，B 退出 0；A 下一会话提示含「门禁超时」。这落实设计 B-07 的加强建议，不真等一分钟。
- 尚未逐个注入全机计数、纯内存队列与提前写时钟的错误实现；对应正向断言已实跑，不把这些未做的变异实验记成通过。
- 新门禁屏障在临时目录消失时退出，避免断言失败后无限等 release。负向实验发生在该收尾补强之前；尝试 `ps` 核对残留进程被沙箱拒绝，未扩大进程清理范围，不声明该次失败实验的后台进程已核验清空。

## 标「待审」之前的自审清单

### 1. 情形表逐行

| 情形 | 实际检查与结果 |
|---|---|
| T1 缺省并发 1、FIFO、预算/API | 两个真实任务、真实会话与门禁；B 停 active/gate，事件 position=1，task.get 与 dashboard 名次=1，项目槽位 queue_position=null；B 无作业文件、无 started；三次预算快照相等；A result.seq < B started.seq，B 退出 0。 |
| T2 Bug ReproCheck | 普通任务占位，真实 Bug Locate 后 B 在 repro_check 排队；fix_round=0、fix_round_started、retry 等三次快照不变；A result.seq < B started.seq。 |
| T3 超时 | 真实入队事件与运行时钟回拨；A 124 与终止行、B 新时钟和退出 0、A 提示「门禁超时」均断言。 |
| T4 账本隔离 | 两个 World 的真实门禁同时启动，两者各有 started、均无 queued。 |
| T5 准备命令 | A 门禁占位时 B 会话前准备排队；job=setup、checkout=worktree，无 setup.started，retry attempts=0；放行后 setup.started.seq > A result.seq。 |
| T6 重启 | A 运行、B/C 名次 1/2；转移同一个 Store 到全新 Ctx，recover 后 tick，名次不变，A started 仍一条且无 result；放行 A、B 后 B 先于 C 启动。 |

每次新增夹具的显式 tick 都核对错误为空，按账本任务的真实作业文件检查活作业 ≤1。

### 2. 失败分支

- 门禁超时：T3 实跑，124、终止行、反馈和后继释放均检查。
- 测试失败：T2 的真实复现命令为失败命令；既有全量 e2e 的门禁失败、准备失败路径保持原样运行。
- 授权拒绝、读盘失败、外部服务不可达：不适用本里程碑六行情形；本轮不新增这些接口或 I/O 分支，也不把模型替身当外部服务验收。
- 排队准入错误：负向实验明确被 T1 拦截，还原后正向复验。

### 3. 全部迁移

- 空闲 → 启动：每个 T 的 A；忙 → 排队：T1/T2/T5/T6；排队 → 持续等待且预算不动：T1/T2 三次快照。
- 运行 → 正常结束 → 下一项启动：T1/T5/T6，用 seq 断言；运行 → 超时 → 释放：T3。
- 排队 → 门禁启动：T1/T2/T3/T6；排队 → 准备启动：T5。
- 有在途与队列 → 新 Ctx 恢复 → 顺序启动：T6；独立账本之间互不占位：T4。
- 既有暂停、停止、取消、恢复等迁移由未改动的全量 e2e 执行；本轮没有新 TaskState/Node 或其它通道。

### 4. 输入域边界

- concurrency 缺省（不写全局/项目键）即 1；单个运行者、单个等待者名次 1、两个等待者名次 1/2；无目录/空作业文件；空队列启动：六个用例覆盖。
- timeout_minutes 最小合法值 1；运行时钟 61 秒前、入队事件 120 秒前；不等待真实一分钟。
- concurrency 的 0/负数/最大值等解析域属于已审 M-01，不在本轮新增输入；日期字符串、前导零、闰日等不适用本轮接口，不增加无关测试。

## 最终版本复验

- `cargo test -p automed --test end_to_end gate_queue_`：6/6 通过，0 失败、0 忽略、106 filtered out，**26.65 秒 < 60 秒**；`.autome/output/m05-final-six.txt`。
- `cargo test -p automed --test end_to_end -- --list`：**112 tests, 0 benchmarks**；`.autome/output/m05-final-list.txt`。
- 最终 `cargo test -p automed --test end_to_end`：**112/112 通过，0 失败、0 忽略，113.65 秒**；`.autome/output/m05-final-full.txt`。相对基线仍是 +6，最终全部新增断言与收尾屏障均在此版本内。

结论：M-05 验收通过，四项自审已执行，提交审计；不自行关闭里程碑。无实现阻塞；失败变异实验的进程清理核验受沙箱限制已如上记录。
