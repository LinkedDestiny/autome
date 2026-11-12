# M-05 r11 实现轮：门禁失败定位，内核修改边界阻塞

## 范围与结论

- implementation-round：11/25；起点 HEAD 309a910，宿主失败提交 b44b4449a404。本轮不推进新里程碑；M-01..M-05 的审计关闭状态原样保留。
- **未修复、未取得同命令绿灯。** 实际失败位于 crates/automed/src/gate.rs，任务文件 §4.1 明确规定 crates 只能读或运行；本轮不把泛化的门禁修复要求解释为撤销该点名禁令。源码没有修改。候选仅保存在允许的临时输出目录，不是已交付修复。
- 上一份审计 #6 为通过，没有待搬迁的审计缺陷检查。本次宿主失败检查已在项目测试体系中；新增确定性探针尚未纳入项目体系，须由获得内核改动授权的任务完成，不能以临时检查代替回归。

## 原检查、根因与候选

- 宿主原始日志：/Users/dannie/project/autome-v2/.autome/output/gates/T-35/merge_gate-b44b4449a404.log（只读）。摘录保存在 .autome/output/r11-host-errors.txt。
- 原检查逐字名称：gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files。gate.rs:548 断言 old wrapper must be dead before paths can be reused 失败。宿主 Rust lib 647 通过、1 失败；后续 Web 364 全通过。
- 先启动原命令 sh scripts/ci-local.sh，日志 .autome/output/r11-ci-before.txt。未修改源码。原检查本轮在全量 Rust lib 中通过；单独编译源码模块后原样检查也通过（.autome/output/r11-original-test.txt，1 条，4.27 秒）。因此不能声称原检查在本轮稳定复现。
- 排查假设：①PID 文件存在但尚未写入数字；②SIGKILL 后未等待退出；③高负载启动超过 3 秒。只验证出①，未声称排除②③或证明宿主此次必由①导致。
- 确定性反例：shell 的重定向先创建/截断 PID 文件、echo 随后写入。discard_node 在 gate.rs:268–279 仅等待 exists；读到空串时 parse 失败，跳过 kill_group 并清理文件。旧进程仍存活，稍后能重建 PID 或覆盖同 stem 的文件。该行为违反函数自身“after stopping any surviving wrapper”契约，也与宿主断言吻合。
- 红：.autome/output/r11-pid-probe.rs 用 #[path] 直接编译本 worktree 的 gate.rs，创建自有进程组与空 PID 文件，延迟 100ms 发布数字，再检查 discard_node 返回后的进程状态及 PID 文件。运行 .autome/output/r11-pid-probe empty_pid_publication_window --nocapture，退出 101；1 条失败，wrapper exited=false, stale pid recreated=true。探针结束前终止并回收自有进程，删除夹具目录。
- 探针限定：launcher::pid_alive 与生产实现同为 kill(pid,0)，只为编译提供 strip_git_env 空实现和固定 now_iso；空 PID 探针不调用这两个替身。它不是完整 Rust workspace 验收。
- 绿候选：将 gate.rs 复制到 .autome/output/r11-gate-candidate.rs，仅将等待条件改成循环读取并解析 PID；读到 PID、started 消失、exit 出现或原 3 秒期限到达才退出。临时差异在 r11-gate-candidate.patch。不增加等待上限、不改变原检查断言。
- 运行 .autome/output/r11-pid-candidate --nocapture：退出 0，12/12，4.33 秒；包括 gate 模块原有 11 条和新增 1 条。总数较模块基线 11 为 +1，来自空 PID 发布窗口。日志 r11-candidate-tests.txt。此结果仅验证候选副本，项目源码仍有缺陷。

## 标待审之前的自审清单

1. **情形表逐行**：本轮不新标待审。临时候选执行 gate 模块全部 11 项现有检查：日志窗口、超时与 setup 标记、marker/liveness 竞态、setup/gate 前缀隔离、running 生命周期、延迟 PID 清理、成功/失败退出、超时终止、进程消失、日志截断；另加空 PID 发布窗口。12 项通过，仅为候选证据。
2. **失败分支**：候选原检查覆盖命令退出 7、超时 124、PID 缺失等待到期、目录不存在、进程消失；空 PID 原代码红、候选绿。授权、读盘失败、外部服务不可达不适用：本轮未改相关产品路径，也未取得这些分支的新证据。
3. **全部迁移**：针对定位范围，缺少 PID→数字 PID→终止→删除→复用由原检查覆盖；空 PID→数字 PID 由新增探针覆盖；缺少 PID→期限到达由原检查覆盖。候选均通过。未宣称验证所有内核状态迁移或解决 SIGKILL 后退出等待问题。
4. **输入域边界**：空 PID 是新增边界；缺少 PID、有效 PID、不可存活 PID、缺少任务目录由原检查覆盖。前导零、日期、跨年、业务金额不适用，本轮不改变相关输入域。

## 门禁与用例基线

- Rust 全量 1198 条：1197 通过、1 原有 ignored、0 失败。相对上一实现轮 1194 为 **+4**，均为本轮开始前已有提交带入：CLI 驱动锁等待 1、driver_lock 通知 1、端到端调度等待回归 2；比对 5b394e1..HEAD 的新增函数清单见 .autome/output/r11-inherited-tests.txt。本轮无项目测试数量变化。宿主失败时 Rust 在 lib 失败后未继续，不能拿那个截断总数作完整基线。
- Desktop 105 条：104 通过、1 失败、0 skipped；相对上一实现轮 105 差 **0**，相对宿主完整基线 165 差 **-60**，原因是 Electron dom-harness 启动 SIGABRT，内部子检查未展开，并非删除测试。
- sh scripts/ci-local.sh 退出 **1**，用时 **674 秒**。Format、Clippy、Rust Test、Contract、Web lint、Web typecheck、Web build 通过；Desktop 与 Web test 失败。未进行第二次无源码变化的重复全量，也没有将临时候选替换到门禁中。
- Web **364 条 / 43 文件**：333 通过、12 失败、19 skipped；相对上一实现轮及宿主基线 364 差 **0**。失败涉及 shell-live/gateway-http 监听及 eventbus-live/notifier-live/server 前置，日志有 listen EPERM 与 watcher EMFILE。没有删除用例或增加 skip；19 条是前置失败导致未执行。
- 本轮原宿主门禁断言未在全量重现；确定性空 PID 探针复现的是兼容该症状的真实竞态。项目源码没有修复，候选绿灯不能作为提交后的门禁结论。最终仍由 Autome 对提交复跑。

## 交接与协议核对

- 下一责任动作：内核维护任务处理 gate.rs 的 PID 发布窗口；将确定性空 PID 检查纳入现有 gate 测试模块并完成真实项目红绿验证，另核对 SIGKILL 后退出确认。W5 不应通过改 Web 用例或删除门禁检查来绕过此问题。
- git diff --check 通过；状态字段各出现一次、implementation-round 11/25、repos 原样、里程碑表与 HEAD 一致、retro 五段且小于 200 字均通过。不标协议失败，不启动下一个会话。
