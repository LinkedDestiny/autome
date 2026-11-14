# M-01 实现轮 #2

结论：审计指出的测试竞态与清理缺陷已修复；完整验收仍受沙箱 `spawnSync ps EPERM` 阻塞，M-01 保持开放，reopen 维持 1，不标待审。implementation-round: 2/65。

## 修复与回归

- setup 在确认 gates 包含 T-1 后，继续有界轮询 gate.sh 进程出现，成功才记录 pid；不再把任务状态当作进程已启动的证据。等待上限 20 秒，不使用固定 8 秒 sleep。force 用例也等待相同 setup，初始化失败不会拿未赋值 pid 继续执行。
- fixture cleanup 改成普通有界轮询（3 秒、50 ms 间隔），保留进程树无残留断言，允许 afterAll 调用；不是忽略失败或跳过清理。
- 审计判别检查已经在项目 test/web-stop-gate.test.ts 中，原样保留其四条行为断言。修复前逐字执行审计命令，40 条中 21 条因 ps EPERM 提前失败，未到原竞态位置，不能称为已复现该竞态；修复后同命令结果相同，尚无该竞态的真实红绿证据。
- 增加 test/web-launcher-cleanup.test.ts：用进程表替身隔离 ps 限制，实际从 afterAll 调用 cleanup；不创建或发送信号给任何子进程。临时恢复审计指出的原句 `await expect.poll(() => processes(root), { timeout: 3000 }).toEqual([]);` 后运行该文件，套件失败，错误逐字为 `expect.poll() must be called inside a test`；恢复修复后套件通过。此检查只证明 hook 清理缺陷修复，不代替真实进程无残留验收。
- BL-09 按用户决定纳入独立 M-13，已写入设计表、方案、需求对照与验收契约。按“一轮只推进最小开放里程碑”，本轮不实现 M-13；之后按编号实现。

## 命令与结果

所有原始输出位于 `.autome/output/impl2/`；下列为不同用例统计，重复红绿运行不重复计数。

1. 修复前原样执行审计命令：`cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-stop-gate.test.ts test/core-supervisor.test.ts`。退出 1；40 条，19 通过、21 失败、0 跳过，失败为 ps EPERM；`red.txt`。
2. 修复后重复上述命令：退出 1；40 条，19 通过、21 失败、0 跳过；`acceptance.txt`。web-instance 8/14，web-launcher 1/12，stop-gate 0/4，core-supervisor 10/10。阻塞发生在创建服务前的进程表预检。
3. `sh plugins/autome/tests/run.sh card panel`：退出 0，29/29；`card-panel.txt`。由于组合命令第一段失败，单独执行第二段以取得独立证据。
4. 原清理调用下 `cd apps/web && npx vitest run test/web-launcher-cleanup.test.ts`：退出 1，测试体 1 条通过但 afterAll 失败，不能算套件通过；`cleanup-red.txt`。修复后 `cd apps/web && npx vitest run test/web-launcher-cleanup.test.ts test/web-launcher-input.test.ts test/kernel-identity.test.ts`：退出 0，15/15；`related.txt`。
5. `cd apps/web && npx eslint test/web-stop-gate.test.ts test/fixtures/web-launcher.ts test/web-launcher-cleanup.test.ts`：退出 0；`lint.txt`。`apps/web/node_modules/.bin/tsc --noEmit --pretty false -p .autome/output/impl2/tsconfig.json`：退出 0；配置仅 include 三个修改文件、继承项目配置并明确 node 类型路径；`types.txt`。首次临时配置的类型查找路径错误已修正，不是项目类型错误。

**用例总数 84 = 40 + 29 + 15，63 通过、21 失败、0 跳过。** 对上个实现轮的 83 条基线增加 1，来自清理 hook 回归；其余集合相同。对上个审计轮 69 条验收基线，验收集合仍为 69，差 0；另外 15 条为输入域 7、内核身份 7、清理回归 1。审计宿主 65/69 通过与本轮 48/69 的差来自 ps 环境限制，不是减少了用例。

## 标待审之前的自审清单

以下四项均已逐项核对；未取得实跑证据的保留缺口，因此本轮不标待审。

### 1. 情形表逐行

| 设计行 | 本轮检查结果 |
|---|---|
| instance 1 原子单行登记 | 通过，含 0600 与 EEXIST 不覆盖 |
| instance 2 只删自己 | 通过 |
| instance 3 pid 已死 | 通过 |
| instance 4 无监听 | 注入 ECONNREFUSED，通过 |
| instance 5 端口换主人 | instance/pid 不符均通过 |
| instance 6 默认端口回退 | ps EPERM，未实测 |
| instance 7 显式占用拒绝 | ps EPERM，未实测 |
| instance 8 healthz 兼容 | ps EPERM；插件 29 条通过不替代 HTTP 实测 |
| instance 9 Host/Origin 拒绝 | ps EPERM，未实测 |
| instance 10 活实例 serve | ps EPERM，未实测 |
| instance 11 活锁不按年龄接管 | 通过 |
| instance 12 死锁接管 | 通过 |
| instance 13 三态判定表 | 全部注入行通过 |
| instance 14 暂停后 serve | ps EPERM，未发信号 |
| launcher 1 固定生产模式 | ps EPERM，未实测 |
| launcher 2 重复打开 | ps EPERM，未实测 |
| launcher 3 并发 start | ps EPERM，未实测 |
| launcher 4 内核身份 status | ps EPERM；身份子集通过不替代 status |
| launcher 5 驱动让路 | ps EPERM，未实测 |
| launcher 6 无门禁 stop | ps EPERM，未实测 |
| launcher 7 非 macOS 打开器 | 通过 |
| launcher 8 serve/start 交错 | ps EPERM，未实测 |
| launcher 9 instance 绑定 | ps EPERM，未实测 |
| launcher 10 暂停读者与恢复 | ps EPERM，未实测 |
| launcher 11 暂停强停 | ps EPERM，未实测 |
| launcher 12 无残留 | ps EPERM，未取得真实进程表证据 |
| stop-gate 1 等待超时 | ps EPERM，未实测 |
| stop-gate 2 强停门禁存活 | ps EPERM，未实测 |
| stop-gate 3 新驱动收取结果 | ps EPERM，未实测 |
| stop-gate 4 无残留 | ps EPERM，未实测 |

A5 无响应读者表：start/无参数/open、serve、status、stop 的真实用例均被预检阻塞；link 登记冲突后的三态重试仍无实跑证据。不能以替身的三态表推定这些调用链通过。

### 2. 设计点名的失败分支

- 授权失败：pid 探测 EPERM 的保守拒绝注入通过；Host/Origin 的真实拒绝未实测。交互授权不适用，本里程碑无授权弹窗。
- 读写盘失败：路径为目录、空串、损坏 JSON、非法 pid/port/instance、父路径为普通文件的检查通过。
- 超时：活锁超时、健康超时分类通过；门禁 --wait、启动 20 秒、锁等待 30 秒、停止 15 秒与 SIGKILL 兜底未实测。
- 空输入：非法参数与空参数值拒绝通过，无参数启动被预检阻塞。
- 外部不可达：ECONNREFUSED 与错误健康回包判定通过；core-supervisor 10 条通过。缺生产构建、打开器失败、后台启动超时日志、登记冲突后二次监听未获得新增实跑证据。

### 3. 全部状态迁移

- absent→start/serve→live，live→重复打开→同实例：被 ps 阻塞。
- stale→接管→live：陈旧判定与排他写入原语通过，真实启动链未实测。
- live→SIGSTOP→unresponsive→SIGCONT→live：注入分类通过，真实信号未实测。
- unresponsive×start/open/serve/status/stop→保留；force→停止：被 ps 阻塞。
- live×stop：无门禁→停止；有门禁→等待→超时保留或清空后停止；force→门禁存活→新驱动接收：均未取得实跑证据。
- 错误 instance→保留：真实用例阻塞；无登记 status=3、stop=0 通过。
- 无锁→持锁→释放，死持有者→接管，活持有者→超时：通过；EEXIST 不覆盖与错误 token 不删除通过。
- gate/fix_gate/merge_gate/repro_check/main_check 节点提取、普通节点与空集对照通过；内核启动/重启/离线等由 core-supervisor 10 条通过，真实驱动锁让路未实测。
- 新增 hook 回归：创建临时夹具→afterAll cleanup→临时目录删除通过；不据此声称服务子孙已被验证清空。

### 4. 输入域边界

- 端口 0、00080、65535、默认末端回退、默认单候选通过；空、负数、65536、小数、NaN、空白拒绝通过。
- 等待 0、前导零合法；负值、小数、超安全整数拒绝；空 instance、缺值、重复和未知参数拒绝通过。
- 健康身份不符、ok=false、无效回包、ps 不可读的保守分类通过。
- 日期闰日/跨年不适用：无用户日期输入。60 秒让路窗口边界仍未单独实测。

## 后续与提交

需要允许 ps 和本地监听的环境重新执行完整验收；尤其核对审计点名的 web-launcher 10、11 与 web-instance 14，以及 gate 真实进程轮询。补齐以上证据后才能标待审。未运行全量门禁或宿主快检，不启动下个会话。
