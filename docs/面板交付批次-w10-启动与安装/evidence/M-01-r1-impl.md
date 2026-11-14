# M-01 实现轮 #1

结论：实现与测试已落地；完整验收受当前沙箱限制，M-01 保持「开放」，不标「待审」或「已完成」。implementation-round 为 1/60。没有上一轮审计意见，无需搬入审计回归检查。

## 改动

- 新增 Node 启动入口及 instance / launcher / opener：后台启动、固定生产模式、端口回退、原子排他登记、启动锁、三态实例判定、复用、status 和带门禁等待的 stop。
- startServer 仅在 instanceFile 开启时等内核就绪后登记；healthz 带 instance/pid，新增受既有安全中间件保护的只读 api/instance；CoreSupervisor 记录近一分钟让路。关闭时仅删除自己的登记。
- stop 支持 --instance 绑定，暂停服务无 --force 时保留；强制停止前核对命令行，超时 SIGKILL 前再次核对登记。无法读取命令行时不向不确定的进程发信号。
- 新增设计中的 30 条用例，以及 7 条输入/失败分支补充用例；既有 server.test.ts 的两条 healthz 断言更新为完整三字段断言；bin 纳入 TypeScript 检查。
- 不推进 M-02 及以后；不修改 Rust 内核、发布安装、自启动、登录 PATH 或其它 worktree。

## 命令、结果与用例数

原始输出均在 `.autome/output/impl1/`，此处只记录判据和结论。

1. 设计原命令的 Vitest 部分：
   `cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-stop-gate.test.ts test/core-supervisor.test.ts`
   最终退出 1，**40 条，19 通过、21 失败、0 跳过**。core-supervisor 10/10 通过，新增 30 条中 9 条通过、21 条全部在启动前的 `ps` 检查报 `spawnSync ps EPERM`。日志：`acceptance-vitest-final.txt`。最初 gate 的 beforeAll 被拒后测试框架自动跳过四条；改成每条用例显式等待同一初始化后，最终四条均明确失败，不隐藏不可执行的验收。
2. 原命令 `sh plugins/autome/tests/card-panel.sh` 返回 0，但它只有函数定义，**实际 0 条**，不能当作通过证据。已把设计中的执行入口修正为项目既有的 `sh plugins/autome/tests/run.sh card panel`，**29/29 通过**，日志 `card-panel-real.txt`。没有改测试判据或插件源码。
3. 直接相关子集：
   `cd apps/web && npx vitest run test/web-launcher-input.test.ts test/kernel-identity.test.ts`
   **14/14 通过**（新增输入域 7 条 + 既有内核身份 7 条），日志 `related-tests.txt`。
4. 静态检查仅覆盖本轮文件：以 `.autome/output/impl1/tsconfig.json` 继承项目配置、include 本轮文件，执行 `apps/web/node_modules/.bin/tsc --noEmit --pretty false -p .autome/output/impl1/tsconfig.json`，退出 0；对本轮 14 个 JS/TS 文件执行 `npx eslint <文件清单>`，退出 0。日志 `types.txt`、`lint.txt`。`git diff --check` 通过。未运行项目级快检或全量门禁。
5. 环境实测：`node:net` 对 `127.0.0.1:0` 的 listen 报 `EPERM`，`ps -o pid= -p $$` 报 `operation not permitted`。新真实服务测试先检查 ps，失败发生在创建任何服务/内核/假 CLI 前；没有通过条件跳过或弱化断言规避。

**总数与基线差：** 本轮实际执行的不同用例总数 **83 = 40 + 29 + 14**，62 通过、21 失败、0 跳过；验收契约本身 69 条，其中 48 通过、21 失败。上一轮基线不适用（这是首个实现轮，无上一轮运行）；按本轮涉及的既有用例集合建立基线 **46 = core-supervisor 10 + kernel-identity 7 + 插件 29**，本轮 **+37**，来自设计新增 30 条与输入域补充 7 条。初次修复前重复运行不重复计数。既有 server.test.ts 的两条更新后健康接口断言因监听限制未运行，不计入这 83 条，也不声称已通过。

## 标「待审」之前的自审清单

本轮仍逐项执行；因以下实测缺口，暂不标「待审」。

### 1. 情形表逐行

| 设计用例 | 检查与结果 |
|---|---|
| instance 1 原子单行字段 | 通过；包含 0600、EEXIST 不覆盖负向对照 |
| instance 2 只删自己 | 通过；错误 instance 保留，正确 instance 删除 |
| instance 3 pid 已死 | 通过；不发健康请求即 stale |
| instance 4 端口无监听 | 通过；注入 ECONNREFUSED 判 stale |
| instance 5 端口换主人 | 通过；分别核对 instance 与 pid 不符 |
| instance 6 缺省换端口 | ps EPERM，未进入真实服务检查 |
| instance 7 显式占用拒绝 | ps EPERM，未进入真实服务检查 |
| instance 8 healthz 兼容 | ps EPERM；插件独立 29 条通过，不替代此 HTTP 实测 |
| instance 9 Host 安全 | ps EPERM；测试包含错误 Host 的 GET 与错误 Origin 的 POST，遵循既有安全中间件语义 |
| instance 10 已有活 serve | ps EPERM，未实测 |
| instance 11 活锁不按年龄接管 | 通过；mtime 设为 epoch，超时仍保留 token |
| instance 12 死锁接管 | 通过；死 pid 接管、新 token 释放 |
| instance 13 三态表 | 通过；死 pid、拒绝连接、身份不符、不响应且命令匹配/不匹配/不可读、健康及 ok=false 全部逐行断言 |
| instance 14 暂停后 serve | ps EPERM，未发 SIGSTOP/SIGCONT |
| launcher 1 无 NODE_ENV 生产启动 | ps EPERM，未实测 |
| launcher 2 重复打开 | ps EPERM，未实测 |
| launcher 3 并发 start | ps EPERM，未实测 |
| launcher 4 status 内核身份 | ps EPERM，未实测；内核身份模块既有 7 条通过 |
| launcher 5 驱动锁让路 | ps EPERM，未运行 lockf |
| launcher 6 无门禁 stop | ps EPERM，未实测 |
| launcher 7 非 macOS 打开器 | 通过；注入 linux，不启动系统打开器 |
| launcher 8 serve/start 交错 | ps EPERM；三轮并发用例已写入，未实测 |
| launcher 9 旧 instance stop | ps EPERM，未实测 |
| launcher 10 暂停三读者与恢复 | ps EPERM，未实测 |
| launcher 11 暂停 force | ps EPERM，未实测 SIGKILL 兜底 |
| launcher 12 子孙无残留 | ps EPERM，无法取得进程表证据；预检禁止创建子进程 |
| stop-gate 1 等待超时 | ps EPERM；未运行 w7-flow 或门禁 |
| stop-gate 2 强停保留门禁 | ps EPERM，未实测 |
| stop-gate 3 驱动接收结果 | ps EPERM，未实测 |
| stop-gate 4 无残留 | ps EPERM，未取得进程表证据 |

A5 无响应读者表逐行：start/无参数/open、serve、status、stop 的真实检查对应上表；link 冲突后的活/无响应/陈旧分支已实现，但本轮不能取得端口竞争实测证据，保留为宿主复验项。单纯排他 link 不覆盖已由 instance 1 通过。

### 2. 失败分支

- 授权拒绝：pid 探测 EPERM 注入检查通过，保持拒绝、不能当陈旧接管；HTTP Host/Origin 拒绝已写真实用例，但沙箱阻塞。此里程碑无交互式授权流程，交互式授权项不适用。
- 读盘失败：把登记路径变成目录，readInstance 明确抛错；空串、非法 JSON、无效 pid/port/instance 均拒绝，通过。
- 写盘失败：父路径是普通文件时写入失败，通过，不伪造成功登记。
- 超时：活锁超时、健康超时三态检查通过；启动 20 秒、锁等待 30 秒、门禁等待、停服务 15 秒兜底的真实进程行为未取得通过证据。
- 空输入：空端口、空参数值拒绝；无参数启动的服务用例被 ps 阻塞。
- 外部服务不可达：健康 ECONNREFUSED 判陈旧通过；错误健康回包及 EPERM 不盲目接管通过。真实内核快速失败、缺失二进制、延迟响应等由 core-supervisor 原有 10 条通过。
- 缺生产构建、打开器执行失败、登记冲突后二次监听、后台启动超时日志等已保留明确错误传播；本轮无对应真实进程通过证据，不把静态检查算成实测。

### 3. 全部迁移

- absent → start/serve → live；live → 重复启动 → 同实例：用例已写，环境阻塞。
- stale → 排他接管 → live：失效判定/登记原语通过；端到端启动未实测。
- live → 暂停 → unresponsive → 恢复 → live：三态替身通过；真实 SIGSTOP/SIGCONT 被阻塞。
- unresponsive × start/open/serve/status/stop：保留登记；force → 停止：真实用例被阻塞，不能声称已完成。
- live × stop：门禁为空→停止；门禁非空→等待→超时保留，或 force→停止但门禁留存→新驱动收取：真实用例被阻塞。门禁自然清空后再停止的轮询分支尚无独立实测证据。
- stop 绑定不同 instance → 不发信号/不删除：真实用例被阻塞。缺登记 status=3、stop=0 已通过。
- 锁 absent→持有→释放、死持有者→接管、活持有者→等待超时：通过。登记 EEXIST 不覆盖、删除错误 token 不生效：通过。
- 门禁节点 gate/fix_gate/merge_gate/repro_check/main_check 全部从 dashboard.running[].task 提取，普通节点与空集负向对照通过。
- 内核启动/连接/重启/离线/停止等既有迁移由 core-supervisor 10 条通过；让路统计真实锁检查被阻塞。

### 4. 输入域边界

- 端口：0、00080、65535、默认范围到 65535 后退到 0、默认 0 单候选通过；空串、负值、65536、小数、NaN、空白拒绝通过。
- --wait：0、前导零合法；负值、小数、超出安全整数拒绝通过。--instance 空串、缺参数值、重复/未知参数拒绝通过。
- 健康身份：instance 不符、pid 不符、ok=false、无效 JSON 响应、不可读 ps 的保守分支通过。
- 日期输入、闰日、跨年不适用：本里程碑没有用户日期输入，只记录系统时间。60 秒让路窗口的时间边界未单独实跑。

## 尚需完成

在允许本地监听及 ps 的宿主/下一轮运行完整 M-01 验收，补齐上述失败分支与迁移实测；同时运行更新后的 server.test.ts 健康接口用例。保留强进程清理断言，不以「静态全绿」代替服务验收。确认全过后才能标「待审」。未运行 `sh scripts/ci-local.sh` 或宿主快检，也未启动下一会话。
