# M-08 · 实现 #14

结论：驱动登记、查询和展示已实现，可运行的业务与模型验收通过；依任务总-6、§6 约束 7，标 **待审**，进程表/监听相关项交审计及宿主复验。完整验收命令因沙箱限制非零，不宣称全绿。implementation-round：14/65。

## 实现范围

- `driver_lock.rs` 增加进程生命周期登记：隐藏临时文件写完、持排他 flock 后 rename 发布；文件描述符 CLOEXEC；查询用非阻塞共享 flock 判陈旧并删除，不靠 PID 存活。内存账本不写文件，命令按 Unicode 字符截到 120。
- mtime 为最近 tick；`touch()` 使用标准库 `File::set_modified`（文件描述符更新时间，不重写 JSON）；推进阈值 `max(30 秒, 3 × interval)`。CLI run 进入循环登记，stdio 入口首条 scheduler.tick 才登记；每次完成更新心跳。非法非有限/负 interval 拒绝。
- 新只读 `driver.list`，内核和 Web 白名单一致；API instance 标记本面板当前内核 PID。status 排除 self，环境页列全部登记和时间；未知方法才退回让路推断，其余异常明确显示暂不可用。
- 环境页复用原有 env-component 样式与语义标题，无新增交互；README、技术设计同步。schema、协议版本均不变。
- 上轮审计 M-07 已通过，未退回 M-08，故没有需要逐字搬运的上轮缺陷检查。

## 验收命令与用例总数

原始日志均在 `.autome/output/`，此处只记结果与判据。

正式命令（逐段执行并保留失败）：

```sh
cargo test -p automed --lib driver_lock && cargo test -p autome-cli --test cli driver_ && cargo build -p automed -p autome-cli && (cd apps/web && npx vitest run test/read-gate.test.ts test/write-gate.test.ts test/web-launcher.test.ts test/env-page.test.ts test/web-status-driver-real.test.ts test/test-groups.test.ts)
```

- 最终日志 `m08-acceptance.txt`：Rust driver_lock **11/11**；CLI driver_ **4/4**；构建成功；Web **100 例，88 通过、12 失败、0 跳过**。12 例均在进程枚举预检报 `spawnSync ps EPERM`。
- 本里程碑总数 **115**；改前同口径基线 **99**（driver_lock 5、CLI 匹配既有锁竞争测试 1、Web 93），净增 **16**：Rust 6、CLI 3、Web 7（读白名单 1、status 4、环境卡 1、真实集成 1）。Web 改前实跑为 82 通过、11 个 ps EPERM，见 `m08-baseline-web.txt`；Rust 改前 5/5，见 `m08-baseline-rust.txt`。设计预测 CLI 3 忽略了既有名称包含 driver_ 的锁竞争测试，实际为 4。
- 上一轮 M-07 审计范围是 84 例（10+67+7），本轮 115，相差 **+31**；其中 15 来自更换里程碑后的验收集合差异，16 来自本轮新增，不能将 31 全称为新增回归测试。截图生成次数不计为用例。
- 无环境依赖的 Web 四文件 **83/83**，`m08-web-green.txt`；四条 status 新测试 **4/4**，`m08-status-green.txt`。后者使用 `-t`，其余 12 条被名称过滤，非条件跳过；完整命令仍执行全部 100 例。

直接相关检查：

| 命令 | 结果 / 日志 |
|---|---|
| `cargo test -p automed --lib` | 1006 例（原 1000 + 新 6），1005 通过、1 失败、0 跳过；`m08-lib.txt` |
| `cargo test -p automed --lib merge_after_past_its_timeout` | 原样单独复跑该失败例，1/1；`m08-lib-recheck.txt` |
| `cargo test -p autome-cli --bin autome` | 67/67，与上一轮相同；`m08-cli-bin.txt` |
| `cargo fmt --all` | 成功；`m08-fmt.txt` |
| `cargo clippy -p automed -p autome-cli --all-targets -- -D warnings` | 成功；`m08-clippy.txt` |
| `(cd apps/web && npx tsc --noEmit)` | 成功；`m08-tsc-final.txt` |
| 改动的 app/server/test 文件定向 `npx eslint` | 成功；`m08-eslint-final.txt` |
| `git diff --check` | 成功 |

Rust 全库失败来自未修改的 `scheduler.rs:7816`：测试只等 hook-child 文件存在便读数字，读到了创建后尚未写入的空串（ParseIntError Empty）。单独复跑通过，不宣称首次全库已全绿，也不修改范围外调度测试。

相关 Web 命令：

```sh
(cd apps/web && npx vitest related --run app/routes/env.tsx app/components/env-model.ts app/load-context.ts server/main.ts server/http.ts server/instance.ts server/launcher.ts server/drivers.ts server/gates/read-gate.ts)
```

`m08-related.txt`：66 文件、**489 例，398 通过、67 失败、24 因套件前置失败未执行**；无新增 skip。失败原文为 ps EPERM、listen EPERM 或服务因 listen EPERM 提前退出；无 AssertionError/TypeError。该相关集合未在改前运行，不虚构其上一轮基线；不与 115 例重复相加。相关测试使用的既有生产构建准备机制照常执行。

方法名/READ_METHODS 使用方检索见 `m08-consumers.txt`、`m08-other-consumers.txt`：命中的 CLI driver_、read-gate、write-gate 已纳入；scripts 仅检查常量名字存在。未运行 ci-quick 或 ci-local。

## 负向对照（变异均已还原）

| 注入与检查 | 红结果 | 还原后绿 |
|---|---|---|
| 查询把活 PID 当作登记仍有效；`cargo test -p automed --lib driver_lock::tests::pid_reuse` | 非空列表违反 empty 断言；`m08-negative-pid-reuse-final.txt` | driver_lock 11/11 |
| stdio 启动立即登记；`cargo test -p autome-cli --test cli driver_core_waits` | 首次查询应空实际有 autome core；`m08-negative-eager-core.txt` | CLI 4/4 |
| driving 固定 true；`cargo test -p automed --lib driver_lock::tests::touch_updates` | 过期仍 true；`m08-negative-heartbeat.txt` | driver_lock 11/11 |
| status 忽略 driving；web-launcher `-t 'status 已登记但'` | 停止推进被称在推进；`m08-negative-inactive.txt` | status 4/4 |
| status 不排除 self；web-launcher `-t 'status 只有本面板'` | 错把自己列为另一个驱动；`m08-negative-self.txt` | status 4/4 |
| Web read-gate 删除 driver.list，内核仍保留；write-gate 文件 | 清单一致性断言失败；`m08-negative-read-gate-final.txt` | write-gate 54/54 |

最后一项初次变异仍通过：原测试仅检查数量与写通道，未对比 Web 读清单。已在原用例增加「ALLOWED_READ_METHODS + WITHHELD_READ_METHODS 与内核 READ_METHODS 完全一致」，用例数不变。PID 对照首次粗略替换 syscall 得到残留 errno 错误，未用作有效判据；最终对照按活 PID 返回条目，明确失败于列表非空断言。

## 标待审之前的自审清单

1. **情形表逐行**：E-3 的内存账本、存活登记、失效删除、活 PID 对应陈旧 inode、隐藏发布文件、心跳过期及恢复分别由 6 条 Rust 新测试覆盖；CLI run 运行/退出和 core 首次 tick 登记由 3 条新集成测试覆盖。status 活跃/不活跃/只有 self/空列表/旧内核逐项检查通过；环境页本面板/外部/停止/旧内核/错误渲染检查通过。真实面板组合用例因 ps EPERM 未到启动步骤，明确待复验。
2. **失败分支**：readDrivers 对拒绝授权、读盘失败、请求超时、内核未连接逐项注入并检查错误文案；未知方法单独验证回退。Rust 查询不存在目录为空、路径为文件报错；无效 interval 拒绝。空列表输出没有其他驱动；端口/进程表拒绝已实际遇到，保留失败。无远程外部服务，本项外部服务不可达不适用（本地内核不可达已测）。
3. **全部迁移**：未登记→run 登记；stdio 未登记→只读请求仍未登记→首 tick 登记；持锁→过期（true→false）→touch（false→true）；持有者退出→锁释放→查询删除；隐藏未发布→查询忽略；本面板自身份→排除于 status、保留于环境页。前述均有测试。真实跨进程 CLI 已通过；HTTP 组合及浏览器视觉迁移受环境限制未实测，不宣称通过。
4. **输入域边界**：空路径与 :memory:；0 interval、None、较大 interval、NaN/Infinity/负数；Unicode 命令 121 截成 120 字符；30.0/30.001 秒、60.0/60.001 秒阈值；空登记集合、单个 self、多个驱动。测试通过。前导零/闰日/跨年不适用：不接收日期文本，时间来自文件 mtime 与内核格式化器。页面无新增表单输入域。

## 环境阻塞与复验交接

正式验收中需在允许进程表的环境按上述完整命令重跑的用例（未弱化）：

- web-status-driver-real：真实 run 与面板共存，status 明确报告驱动，退出后消失。
- web-launcher：无 NODE_ENV 无参数：生产服务、打开器、端口和页面一致；重复打开只复用一个服务；两个并发 start 只起一个服务；status 输出端口、内核路径来源与 autome 构建标识；status 使用登记而不是瞬时锁让路推断；没有门禁时 stop 清理进程和登记文件；直接 serve 与 start 并发多轮始终只登记一个活服务；stop 绑定旧 uuid 不停止新实例、不删文件；暂停期间 start/status/stop 保留登记，恢复后 reused；暂停后 stop --force 用 SIGKILL 兜底并清理登记；服务和内核的所有子孙无残留。

相关 Web 套件的完整复验命令已在上文列出，错误用例索引在 `m08-related-errors.txt`。环境页真实截图/浏览器视觉核对未运行，当前证据为 SSR 渲染及类型检查；H-04 真实 launchd 人工验收未动。

隔离：所有新增 CLI 测试使用 Sandbox 临时 home，子进程以 RAII kill/wait 回收；正式 Web harness 在 ps 预检即被阻止，未启动面板或浏览器。相关既有真实内核套件执行了各自 teardown，但最终 ps 核对被拒，不能宣称已核实全机无残留。没有接触真实账本，没有手工留下常驻服务，不启动下一会话。
