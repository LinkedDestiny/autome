# M-09 · 实现轮 17/65 · 快检修复

本轮只修复宿主在 e5fd381668 上发现的 `driver_lock::tests::released_registration_is_removed` 失败；M-09 保持待审，不推进新里程碑。上一份审计为 M-08 通过，没有退回检查需要迁移；宿主失败检查本来就在项目测试体系内，本轮保留其名称和全部原断言，增加稳定触发条件。

## 根因与修复

Registration 仅关闭 File，没有显式解锁。并行测试启动子进程时，fork 到 exec 之间可继承同一个 open file description；O_CLOEXEC 要到 exec 才生效。登记对象虽已销毁，子进程短暂持有的描述符仍使 flock 存续，drivers 查询误报活跃登记。DriverLock 已有显式 LOCK_UN，Registration 缺少同样的生命周期处理。检查本模块两个持锁对象后，仅补 Registration::drop 的显式解锁，不改查询断言、不加重试、不串行化测试。

回归用例用 File::try_clone 模拟共享 open file description，不真正 fork 多线程测试进程：先断言登记可见，再销毁登记对象，在共享描述符仍打开时原样断言查询为空及目录为空，最后关闭克隆。该触发条件排除了目录串扰和随机调度的依赖，旧实现稳定失败。

## 命令与结果

原始输出位于 `.autome/output/`。

- 原样单跑 `cargo test -p automed --lib driver_lock::tests::released_registration_is_removed`：1 通过（`r17-original.txt`），单跑没有碰到 fork 窗口，不能据此否定宿主失败。
- 加入共享描述符触发条件、修复前同一命令：1 失败，仍是 `assert!(drivers(&db).unwrap().is_empty())`（`r17-red.txt`）。这是本轮确定性红证据；原始随机失败的红证据来自用户提供的宿主输出。
- 修复后 `cargo test -p automed --lib driver_lock`：11 通过，0 失败/忽略（`r17-green.txt`），包含上述回归。
- 指定的原样快检：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`：退出 0，5 步通过（`r17-quick.txt`）。脚本明确提示“未提供本轮起点，未跑改动相关的测试”，因此另行原样运行宿主失败的库测试命令，不能把本地快检等同于宿主提交后复验。
- `cargo test -p automed --lib --no-fail-fast`：退出 0，1024 通过，0 失败/忽略（`r17-lib.txt`）；原宿主失败用例在并发库测试中通过。
- `git diff --check`：通过。未运行 `sh scripts/ci-local.sh`，未运行下一里程碑测试。

计数：driver_lock 仍为 11 例，相对 M-08 基线 11 的差为 0；只增强已有用例，没有新增或删除用例。宿主库测试基线为 1024（1023 通过、1 失败），本轮库测试总数仍为 1024，差为 0，全部通过。定向测试是库测试子集，不重复相加。上一轮 M-09 选中的 1145 是库 + 真实清理 + 关联 Web 的不同范围，本轮不重跑未改动的 M-09 真实流程/Web，不能把范围差当作用例删除；其历史证据见 M-09-r16-impl.md。

## 自审清单

1. **情形表逐行**：本轮不改 F 情形表，M-09 仍待独立审计。针对修复涉及的 E-3 登记：登记持锁时可见、共享描述符存续时释放后不可见且陈旧文件删除、PID 复用陈旧登记不存活、隐藏未发布文件不读取、内存账本无文件，均由 driver_lock 11 例执行通过。DriverLock 已有继承描述符用例同样通过。
2. **失败分支**：非法间隔 NaN/Infinity/负数拒绝、登记目录为普通文件时读盘报错、竞争锁到期返回 None、长竞争通知一次，既有用例均通过。拒绝授权/外部服务不可达不适用：本修复没有权限接口和网络调用；没有声称注入 LOCK_UN 系统调用失败。
3. **全部迁移**：未登记→登记可见→touch 更新→过期不推进→touch 恢复；登记→drop→查询删除陈旧记录；持锁→第二持有者拒绝→释放→可重取；有共享描述符时释放→查询不可见，以上均通过。内存账本 drop 无文件分支也执行通过。
4. **输入域边界**：空串/:memory:、单登记、Unicode 命令 121 截至 120 字符、间隔 0/负数/NaN/Infinity/f64::MAX、推进阈值 30/30.001 和 60/60.001 秒，既有 11 例通过。日期/闰日/跨年不适用，本修复不解析日期输入。

## 交接

未接触真实账本或 launchd，回归只操作测试临时目录及自有文件描述符，不启动服务、不向宿主进程发信号。最终快检结论以 Autome 提交后复验为准。
