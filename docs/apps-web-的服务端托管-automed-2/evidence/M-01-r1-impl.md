# M-01 · 实现 #1

结论：待审。M-01 实现与新增用例通过；全量 Web 的 W1 监听套件受沙箱限制，按设计「风险与验证安排」第 2 条交审计复验。不得将本记录理解为全量 67 个全绿。

## 命令与结果

在当前 worktree 执行，原始输出均在 .autome/output/，没有修改 desktop、crates、依赖清单或其它里程碑代码。

| 命令 | 结果 | 原始输出 |
|---|---|---|
| cargo build -p automed | 退出 0 | final-build.txt |
| npm run lint --prefix apps/web | 退出 0，无错误或警告 | final-lint.txt |
| npm run typecheck --prefix apps/web | 退出 0 | final-typecheck.txt |
| npm test --prefix apps/web | 退出 1；发现 67 个，60 通过、7 未执行；server.test.ts 的 beforeAll 因 listen EPERM 127.0.0.1 失败，同时有 EMFILE watcher 诊断 | final-web.txt |
| npm test --prefix apps/desktop | 退出 0；104 个，103 通过、1 基线跳过、0 失败 | final-desktop.txt |
| npm test --prefix apps/web -- --exclude test/server.test.ts | 60/60 通过，仅用于区分环境阻塞，不能替代全量 | web-no-listen.txt |
| npm test --prefix apps/web -- test/core-framing.test.ts test/core-binary.test.ts test/core-sidecar.test.ts test/ci-config.test.ts | 35/35 通过，0 跳过，负向变异全部还原后的结果 | final-m01.txt |
| git diff --check | 退出 0 | 无输出 |

与上一轮设计基线的差：Web 总数 **67 − 38 = +29**，来源是 framing 4、binary 6、sidecar 18、ci-config 新增 1；没有删除 W1 测试。sidecar 是原 17 去掉已由 binary 覆盖的 #1，再增加无超时等待与禁止条件跳过两条，即 18。desktop **104 − 104 = 0**；唯一跳过仍为 electron dom harness。Web 的 7 个 skipped 是套件启动失败导致未执行，不是测试中的条件跳过；真实内核 sidecar 用例全部实际运行。

CI 配置只能本地结构取证，未声称 GitHub job 真跑过。完整 ci-local 由 Autome 宿主门禁执行，本轮未额外执行。

## 迁移与修复说明

- framing 的 4 个原用例保留 node:assert 断言，改用 vitest/ESM/TS，超长帧提示与正则改中文；在原超长帧用例补空帧、恰好上限、uint32 最大长度。
- sidecar #1 改由 binary 的 6 个用例覆盖，消除 Electron resourcesPath 与 process.env 修改。
- sidecar #2 扩到 test/*.ts 的 AutomedSidecar/CoreSupervisor 构造扫描；M-05 才扩 startServer。#3 缺 binary 直接失败，中文提示构建。所有真实内核启动显式传入测试二进制和临时 AUTOME_HOME。
- 剩余原断言保留，already pending、timed out、exited before replying 改中文正则。临时 home 改 mkdtemp，vitest afterEach 清理；外来数据库也纳入清理。进程退出断言在 stop 前注册 rejection handler，避免测试产生未处理拒绝。
- 原乱序用例仍有真实内核双请求，额外用 fake-core 强制第二条先回复，锁住关联逻辑而非依赖概率。
- 新增 requestUntilReply/isOutstanding，共用 pending 表；普通 request 超时删除 pending 的原语义不变。stop 对已经退出的进程直接释放管道，避免为已发生的 exit 再等 2 秒；其它退出/停止行为保留。
- TypeScript 显式定义命令、出站消息和回调类型；诊断 ANSI 去色保留，局部说明 no-control-regex 例外；捕获后重抛附 cause 满足既有 lint。
- CI 只改 web job，在 npm test 前 cargo build；原 ci-config 顺序断言同步更新，新增本地 Rust all-targets 在 Web test 前的顺序核对。
- 无上一轮审计，不存在需要搬入的审计回归或审计红绿证据。

## 标待审之前的自审清单

1. **情形表逐行**
   - §2 第 1 行 env：非空覆盖不查盘，通过；空串按未设通过。
   - §2 第 2 行 bundle：存在时优先 local/cargo，通过；锁定发布包 web 同级路径和 source。
   - §2 第 3 行 local-bin：bundle 不存在时选择 home/.local/bin/automed，通过。
   - §2 第 4 行 cargo：即使不存在仍返回，debug/release 两种 profile 通过。
   - §6 framing 与 sidecar 的迁移行及 CI 行逐条落实如上；ipc/write/server 的迁移行属于后续里程碑，本轮不适用。

2. **失败分支**
   - 缺二进制：spawn error 被收敛，一次 onExit，failureReason 中文；未启动 child 不发信号，通过。
   - 数据库读取/打开失败：真实 sqlite3 外来 schema，内核拒绝且原始中文原因被保留，ANSI/日志字段被剥离，通过。
   - 超时：普通 request 在 20ms 拒绝；无超时请求经调用方 20ms 本地等待结束后仍在途、进程仍在，迟到 Reply 才清理，通过。
   - 重复 request_id：第二条拒绝；进程退出：等待请求拒绝；向已退出进程写入：同步错误且没有异步 EPIPE 崩溃，通过。
   - 拒绝授权、外部网络服务不可达：不适用，M-01 是 stdio 子进程，不含 HTTP 鉴权和外部网络连接。
   - 空输入：空 AUTOMED_BIN 和空帧通过；空帧仅测试 framing 编解码，不声称空 JSON 是合法命令。

3. **全部迁移**
   - 帧：不足头部 → 继续等待；完整头部但 payload 不足 → 等待；完整 → 发出；粘包 → 顺序发出；尾部半帧 → 下一块补齐；超长头 → 拒绝，均通过。
   - 请求：发送 → pending；Reply → resolve/删除；重复 ID → 拒绝且原 pending 保留；普通超时 → reject/删除；退出或 stop → reject/清理；无超时等待经过本地 timeout → 保留；迟到 Reply → resolve/清理；无超时等待退出 → reject/清理，均通过。
   - 通道：stdin 命令、stdout reply/request_id 关联与 event 顺序、stderr 诊断分别验证；主动 stop 不报告崩溃，缺 binary 错误只报告一次，通过。
   - 五态 supervisor 与周期 tick 属于 M-03，本轮不适用。

4. **域边界**
   - 帧长度 0、4 字节上限本身、超过上限、0xffffffff 头部、3 字节未满头、单帧、粘连帧、分片 payload 均通过。
   - 查找空覆盖变量、所有候选都不存在、所有候选都存在、bundle/local 的交界、release profile 通过。
   - request_id 相同拒绝、不同且逆序回复正确关联，通过。
   - 日期、闰日、跨年、前导零：不适用，M-01 无日期/数字文本输入域。

## 负向对照

每项独立修改、运行、还原；所有退出码为 1，最后 M-01 35 个用例恢复全绿。原始日志 mutation-*.txt。

- BE 改 LE：framing 分片失败；另跑真实 sidecar correlated by 用例，因收不到合法 Reply 失败（mutation-endian-sidecar.txt）。
- 禁用 local-bin 优先分支（等价于 cargo 抢先兜底）：binary 第 3 条失败。
- pending 按到达顺序取而非 request_id：强制乱序用例失败。
- 删 web job 构建步骤：ci-config 顺序用例失败。
- 把构建移到 npm test 后：同一顺序用例失败。
- 给真实往返用例加 test.skipIf(process.env.CI)：源码守卫失败。

## 交接

审计应独立复验 M-01，并在允许回环监听的环境跑完整 67 个 Web 用例。没有产品缺陷证据；本轮唯一未取证项为既有 W1 监听测试。M-02 保持开放，不在本轮推进。
