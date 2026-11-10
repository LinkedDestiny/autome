# M-04 · 实现 #6 · 宿主门禁专项修复

本轮为用户指定的修复轮，implementation-round 为 6/20，不推进新里程碑。M-01..M-04 已由前轮审计关闭，本轮保持原状态，不自行关闭里程碑。上一轮审计无退回缺陷或待搬运检查；本轮发现失败的检查本来就在项目测试体系中，原样保留。

## 复现、修复与验收

- 修复前原样执行 `sh scripts/ci-local.sh`，exit=1，154s；原始日志 `.autome/output/r6-ci-red.txt`。Test 步骤中的 `a_tick_skips_while_another_driver_holds_the_ledger` 在 `end_to_end.rs:2720` 失败：`assertion failed: !automed::scheduler::tick(&mut ctx).skipped`，38 个用例中 37 通过、1 失败；后续独立 End to end 步骤 38/38 通过，说明是时序相关失败。用户提供的宿主摘要没有失败断言，不能声称已核对宿主失败的具体用例与这里相同。
- 机制：DriverLock 只靠 File 关闭释放 flock；其他线程 fork 时，子进程暂时共享 open file description，即使 Rust 打开文件设置了 close-on-exec，也要到 exec 才关闭副本。原持有者 drop 后，副本可继续持锁，导致下一个 tick 被跳过。用 File::try_clone 复现相同的共享文件描述符生命周期，无需依赖真实 fork/exec 的随机窗口。
- 新增永久回归 `driver_lock::tests::dropping_the_guard_releases_the_lock_with_an_inherited_descriptor`。修复前执行 `cargo test -p automed --lib driver_lock::tests::dropping_the_guard_releases_the_lock_with_an_inherited_descriptor -- --exact`，exit=101，1 个失败、534 个过滤，断言 `an inherited descriptor outlived the guard`；日志 `.autome/output/r6-lock-red.txt`。
- 最小修复：`DriverLock::drop` 在关闭自己拥有的描述符之前显式 `LOCK_UN`。专项修复仅修改 `crates/automed/src/driver_lock.rs` 及其同文件测试，依据本轮最新的“先复现、修好 End to end”指令，作为 W1 原始禁止修改 crates 的专项例外；未改业务调度、未串行化或跳过测试、未延长超时。
- 修复后 `cargo test -p automed --lib driver_lock::tests`，exit=0，4/4 通过（原有 3 个 + 新增 1 个）；日志 `.autome/output/r6-lock-green.txt`。
- 第一次修复后完整门禁：`.autome/output/r6-ci-green.txt`，exit=1，136s；Test 与独立 End to end 均通过（后者 38/38）。Format 指出新增断言应换行，已按要求修正；`cargo fmt --all -- --check` 随后 exit=0（`.autome/output/r6-fmt.txt`）。Web test 仍因沙箱 `listen EPERM 127.0.0.1` / 文件监听 EMFILE 阻塞。
- 最终原样门禁：`sh scripts/ci-local.sh`，exit=1，135s，日志 `.autome/output/r6-ci-final.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/build 全部通过；仅 Web test 因 `listen EPERM 127.0.0.1` / EMFILE 失败。Test 内和独立 End to end 均为 38/38 通过。当前权限模式禁止提升权限，无法在本会话取得完整门禁全绿证据，交 Autome 在宿主复验；未降低断言或把失败转成成功。

## 用例总数与基线差

- Web 用例总数 38，与上一轮审计基线 38 相差 0；当前沙箱实际执行 31 个，server 套件 beforeAll 失败导致 7 个跳过，不能视为 38 个通过。
- automed 单元测试 535，与修复前本轮实跑基线 534 相差 +1，全部来自新增的描述符继承回归；锁模块 4 对 3，同样 +1。
- End to end 用例总数 38，对修复前基线 38 相差 0，原失败用例未修改。Desktop 总数 104 未改动，仍有既有的 1 个跳过。
- 最终全量按各步骤累计的用例执行槽位总数为 1062（Rust 920，Desktop 104，Web 38，包含门禁原有的 End to end/Adapter 重复执行），对本轮修复前 1044 增加 18：新增锁回归 +1，另外 17 是修复前 cargo 遇到端到端失败提前退出，未跑到的后续两个测试目标（10 + 7）；没有新增这 17 个用例。上一轮审计没有记录 Rust/Desktop 全量数字，不能虚构上一轮全量基线；本轮修复前的门禁日志作为可核对的补充基线。日志摘要分别为 `.autome/output/r6-counts-red.txt` 与 `r6-counts-final.txt`。

## 四项自审清单

1. **情形表逐行**：本轮仅修复 R-03 门禁的锁行为。无竞争→获得锁；已有持有者→第二持有者被拒绝；释放→可重新取得；描述符副本仍存在→释放后仍可重新取得；关闭旧副本→新持有者仍保持互斥。前两类由原有锁单测，后三类由新回归逐项断言，4 个锁用例均通过。M-04 的 CI 配置六个原有守卫保持通过，Web 四步仍独立运行，未改门禁结构。
2. **失败分支**：竞争拒绝由 `a_second_holder_is_refused_until_the_first_lets_go` 与新增回归覆盖；等待超时由 `waiting_gives_up_at_the_deadline` 覆盖（120ms 截止），均通过。授权拒绝、读盘失败、外部服务不可达：不适用，本轮没有新增这些路径，设计也没有为锁模块声明它们；Web 监听被拒绝是已如实记录的环境阻塞，没有绕过。
3. **全部迁移**：未持有→持有→竞争被拒→释放→新持有者取得→旧副本关闭但新锁仍有效→新持有者释放→再次取得，回归逐项断言通过。内存数据库不使用实际账本锁，由原有 `an_in_memory_store_has_nothing_to_lock` 覆盖。End to end 原有检查验证持锁 tick skipped、释放后 tick 恢复，检查保持原样。
4. **输入域边界**：`:memory:` 的无账本路径由原有单测覆盖；零个副本由原有互斥测试、单个仍存活副本由新增回归覆盖。日期、数值最大值、前导零：不适用，本轮没有增加该类输入域。

同类检查：搜索 `crates/automed/src` 中 `flock|LOCK_UN|LOCK_EX`，直接 flock 操作仅见 driver_lock.rs，结果在 `.autome/output/r6-all-lock-sites.txt`。无需扩大修改范围。

## 交付边界

H-01、H-02 仍为既有人工验收项，本轮未取证。最终门禁是否通过，以 Autome 在本轮提交上重跑为准；不以这里的本地回归结果替代宿主结果。未启动后续会话、未切换分支、未推送。
