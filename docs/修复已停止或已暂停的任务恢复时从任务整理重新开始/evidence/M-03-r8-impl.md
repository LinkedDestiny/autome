# M-03 · 实现 #8 · 门禁修复轮

本轮 implementation-round 为 8/25，M-03 保持待审，不推进 M-04。输入 HEAD 为 6e1c34d；宿主报告失败的提交为 8caab15c6e。上一审计通过 M-02，没有审计退回用例需要搬入。

## 复现与根因

- 原样运行 `sh scripts/ci-local.sh` 两次，均退出 0，分别 103 秒和 93 秒；日志 `.autome/output/r8-ci-red.txt`（文件名不代表结果）、`r8-ci-reproduce-2.txt`。宿主只提供汇总，未给出 Test 的失败名，因此不能确认宿主那一次的具体失败原因。
- 工作树保留的 `.autome/output/r3-ci-after.txt` 记录了现有 `driver_lock::tests::a_second_holder_is_refused_until_the_first_lets_go` 在释放后再次获取锁失败。检查所有调用：scheduler tick 与 dispatch 写命令均通过销毁 DriverLock 释放锁；对象原来只关闭文件，没有显式 LOCK_UN。
- 共享 open file description 的描述符仍存在时，close 不会立即释放 flock。并行启动子进程的 fork/exec 窗口可以保留这种引用。用 File::try_clone 确定性模拟该引用，避免测试依赖实际 fork 时机；这证明了机制，但不把它冒充为宿主失败的精确归因。
- 新增项目回归 `dropping_the_holder_releases_the_lock_even_with_an_inherited_descriptor`，保留原测试的持有时拒绝、drop 后立即再次获取断言，旧用例不改。`cargo test -p automed --lib dropping_the_holder` 在修复前为 1 个失败，断言为 `assert!(try_acquire(&db).unwrap().is_some())`，日志 `.autome/output/r8-lock-red.txt`。
- 新增回归后原样全量门禁也在同一断言报红，日志 `.autome/output/r8-ci-regression-red.txt`。命令退出 1，用时 60 秒；本轮没有删除测试、延长等待或跳过竞争。修复是在 DriverLock 生命周期结束时显式 LOCK_UN，再由 File 正常关闭描述符，所有调用共享这一处修复。

## 验收结果

修复后 `cargo test -p automed --lib driver_lock` 退出 0：4/4，通过数相对旧专项 3 增加 1；日志 `.autome/output/r8-lock-green.txt`。

修复后原样运行 `sh scripts/ci-local.sh` 退出 0，用时 117 秒；Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract 全部通过，日志 `.autome/output/r8-ci-green.txt`。未执行可选 package。

- Rust 工作区总数 912（CLI 8+4、domain 288、automed lib 545、e2e 50、prompt 10、real_cli 7），全部通过、忽略 0。对上一轮 911 差 +1，来源仅为新增驱动锁回归。
- Desktop 总数 104，103 通过、0 失败、1 跳过，较宿主基线差 0；保留原有跳过，未增删桌面用例。
- 门禁独立用例共 1016，较完整基线 1015 差 +1。脚本重复执行 e2e 50 与 Adapter flags 1，因此实际执行 1067 次，不把重复执行算成新增覆盖。
- 回归红灯门禁执行的独立用例为 1000：Rust 896（CLI 12、domain 288、lib 545、单独 e2e 50、单独 adapter 1）+ Desktop 104。最终绿灯比它多 16，是 lib 失败不再阻止 Test 步骤后续 prompt 10 和其余 real_cli 6，并非跳过数变化。
- 以下四项自审均在最终全量及 4 个锁专项中执行通过；没有修改恢复状态机或原有测试断言。

## 四项自审清单

1. **情形表逐行**：本轮不改变恢复情形表；最终全量已覆盖 §3 R1 原会话继续、R2 暂缓交付、R3 重起，§4 Paused/Requeued 暂缓及暂停后停止，§5 同 kind 手动复盘排除与恢复互斥。驱动锁修复检查正常持有拒绝、正常释放、共享描述符仍在时释放三种情形。
2. **失败分支**：锁被占用时立即拒绝、等待到期返回 None；恢复侧崩溃暂缓、手动轮拒绝、槽满等待由原有测试检查。授权、网络、空输入不适用：本轮未新增这些接口或分支；没有声称执行真实授权/网络故障注入。
3. **全部迁移**：锁未持有→获取→拒绝第二持有者→释放→再次获取；共享描述符存在时同样执行。暂停侧 Active→Paused→会话结束→Requeued→交付、提前恢复等待原会话、槽满 Requeued 暂缓后交付、Paused→Stopped→恢复、崩溃交付后再次暂停恢复、门禁暂停恢复、手动复盘结束恢复均由原有全量测试检查。
4. **输入域边界**：零共享描述符与一个共享描述符；内存账本无锁；超时边界。恢复侧单槽满/释放、零/一个未交付结论、已交付结论、同 kind 不同来源保持原测试。日期、前导零、闰日、跨年、最大数值不适用：无新增这类输入域。

## 边界

代码只改 crates/automed/src/driver_lock.rs，原有测试断言不改，不修改脚本或其它里程碑。H-01～H-03 人工验收未运行。最终门禁以 Autome 在本轮提交后的提交上重跑为准。
