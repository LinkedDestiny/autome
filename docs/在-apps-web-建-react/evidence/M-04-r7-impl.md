# M-04 · 实现 #7 · 宿主门禁专项修复

本轮 implementation-round 为 7/20，只处理用户指定的门禁失败，不推进新里程碑。M-01..M-04 在本轮开始前已由审计关闭，保持原状态；没有待审项。本轮未自行关闭里程碑。上一轮审计无退回检查；本次失败检查原已在项目测试体系内。

## 根因、红绿检查与修复

1. 原样执行 `sh scripts/ci-local.sh`：exit=1，75s，日志 `.autome/output/r7-ci-red.txt`。Rust Test 中 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 失败，断言原文 `the wrapper removes its pid on the way out`。535 个 automed 单测为 534 通过、1 失败。用户给出的宿主摘要没有具体断言，不能证明宿主与本次失败是同一个用例。
2. 包装脚本先原子发布退出标记，再删除 PID；`poll()` 看到标记即返回 Done，原测试错误地把 Done 当作包装进程退出。临时边界实验让真实 WRAPPER 在发布标记后立即退出，原断言确定性失败（exit=101，`.autome/output/r7-regression-red.txt`）。该实验用于区分契约，未作为永久用例保留：这个中间状态本来就允许存在。交换两步会让长任务在 PID 已删除、标记未发布时被判 Vanished，因此没有修改生产包装脚本。
3. 修复 `gate.rs` 原用例：在保留原 PID 删除断言之前，以 5 秒截止时间、每次检查 PID 是否仍存在的方式等待清理；退出码、日志断言不变，不跳过、不重试整个测试。`cargo test -p automed --lib gate::tests` exit=0，5/5（`.autome/output/r7-gate-green.txt`）；原失败用例以 `--exact` 连续执行 30 次，全部通过（`.autome/output/r7-gate-repeat.txt`）。
4. 第二次原样全量门禁（`.autome/output/r7-ci-final.txt`）暴露另一原有用例 `git::tests::a_command_that_finishes_quickly_is_not_held_by_the_poll`：轮询最快 19.436375ms、直接等待最快 7.543584ms，差超过 10ms。两个独立进程样本组受并发调度影响，差值不是纯退避成本。
5. 修复 `git.rs`：等待实现提取可传入 sleep 函数的内部入口，生产入口仍直接使用 `std::thread::sleep`，轮询、截止时间、杀进程及读管道逻辑不变。原测试用 stdin 阻塞真实子进程，记录实际等待循环请求的 1、2、4、8、16、20、20ms，在第七次放行子进程，验证成功退出、stdout 和此后 20ms 上限。取消依赖机器负载的两组耗时相减，不放宽阈值、不隐藏失败。
6. `cargo test -p automed --lib git::tests::a_command_` exit=0，2/2，含超时杀进程（`.autome/output/r7-git-green.txt`）。临时把初始间隔改成 20ms，原用例名下新断言 exit=101（`.autome/output/r7-git-negative.txt`）；已恢复 1ms。该负向对照证明固定 20ms 的真实退化仍会被拦截。
7. 第三次全量（`.autome/output/r7-ci-verified.txt`）中 Rust 单测 535/535；Format 指出新增断言换行问题，已修正，独立 `cargo fmt --all -- --check` exit=0（`.autome/output/r7-fmt-green.txt`）。最终全量结果见下。

## 四项自审清单

1. **情形表逐行**：M-04 门禁步骤结构未改动，仍每步运行并汇总。正常门禁命令→退出码 0、日志保留、最终 PID 删除；命令失败→退出码 7 与 stderr 保留；快速子进程→按退避间隔轮询并返回输出；超过截止时间→杀死并回报。门禁 5 个用例与 Git 2 个针对性用例均通过。
2. **失败分支**：失败命令、超时、无进程无标记的 Vanished 均由原门禁用例覆盖；Git 超时断言同时检查进程确实死亡。授权拒绝、外部服务不可达、读盘失败不适用：本轮不新增这些分支。Web 监听权限拒绝如实记录，不以跳过认作通过。
3. **全部迁移**：门禁启动→运行→发布标记/Done→PID 清理；失败命令→Done(7)；超时→Done(124)；无标记且进程消失→Vanished；Git 阻塞等待→退避升至上限→stdin 放行→正常退出，及超时→杀进程。前述用例覆盖，未改变生产状态机。
4. **输入域边界**：退避首个 1ms、倍增、16→20ms 截断、20ms 连续上限逐项断言；退出码 0/7、截止超时、中文日志截断边界由现有测试覆盖。日期、前导零、空业务输入不适用，本轮只修改门禁相关测试的时序同步和可测入口。

同类检查限于两个失败模块的轮询、等待和完成标记边界（搜索记录 `.autome/output/r7-timing-sites.txt`）。不扩大到无失败证据的业务模块。对 crates 的修改依据本轮用户专项修复门禁指令，已在设计范围例外中注明。

## 交付边界

未修改 Autome 配置/规则、未切分支或推送、未启动后续会话。H-01、H-02 人工验收未取证。宿主门禁最终结论以 Autome 在本轮提交上重跑为准。

## 最终同命令结果与用例基线

- 最终执行 `sh scripts/ci-local.sh`：exit=1，105s，`.autome/output/r7-ci-last.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/build 全部通过；仅 Web test 失败，原因是沙箱 `listen EPERM 127.0.0.1` 与监听 `EMFILE`。当前权限模式禁止提权，未绕过沙箱、未弱化 Web 断言，无法在本会话取得全门禁通过证据。
- Rust 执行槽位 920，Desktop 总数 104（103 通过、既有 1 跳过），Web 总数 38（31 通过，server beforeAll 失败导致 7 跳过）。合计 1062，与上一轮 r6 最终基线 1062 差 0；automed 单元测试 535、Web 38、独立 End to end 38 均与上一轮相同。本轮不新增、不删除用例，只修复两项原有用例的同步与测量方法。
- 对本轮首次红色门禁的 1007，最终执行槽位增加 55：Rust 单测失败不再提前中断 cargo，恢复了后续 end_to_end 38、prompt_rendering 10、real_cli 7。不是新增测试，也没有通过减少测试使门禁变绿。
- 已核对状态块字段完整、implementation-round 为 7/20、里程碑表与轮初逐行相同，`git diff --check` 通过。全门禁仍需由 Autome 在提交后独立复验，不能将本地 Rust 成功写作全量成功。
