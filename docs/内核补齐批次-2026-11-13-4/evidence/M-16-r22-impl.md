# 实现 #22：Web test 清理钩子修复

## 范围与定位

本轮为门禁修复轮，implementation-round: 22/80；不推进新里程碑，不改已关闭里程碑及其最新证据指针。上一轮审计通过，没有需要搬迁的审计判别检查。

宿主提交 43d1b09d9e 的原始日志位于主检出 `.autome/output/gates/T-95/merge_gate-43d1b09d9e23.log`。833 个测试全部通过，但 `test/eventbus-live.test.ts` 的 afterAll 在第 57 行 `rmSync(home, { recursive: true, force: true })` 报 ENOTEMPTY，导致 110 个测试文件中 1 个失败。提取记录：本 worktree `.autome/output/r22-failures.txt`。

原清理没有重试；force 仅允许目标不存在，并不能消除 ENOTEMPTY。改用 Node 内置异步 rm，配置 maxRetries: 5、retryDelay: 100（递增退避总计至多 1500ms，另加文件操作时间）。仍先结束客户端、等待服务退出，再删除目录；最终错误继续使清理钩子失败。不修改产品代码，不更改七个真实 Web 用例的断言、等待上限或跳过条件。

同类检查范围为 eventbus 测试文件，只有该 live 文件执行临时 home 的递归删除。尚无证据证明具体是谁在删除期间写目录，不能声称已证实后台进程泄漏；本次是针对已观测错误的有限重试修复，是否解决宿主故障须宿主复跑确认。

## 命令与结果

- 修前原样单跑：`npm run test --prefix apps/web -- test/eventbus-live.test.ts`，日志 `.autome/output/r22-original.txt`。beforeAll 被 `listen EPERM: operation not permitted 127.0.0.1` 阻断，同时有文件 watcher EMFILE；1 个套件失败、7 个用例跳过。没有复现宿主 ENOTEMPTY，不能把权限失败当作该缺陷的红证据。
- 修后同命令：日志 `.autome/output/r22-after.txt`，仍受相同端口权限阻断，未取得真实套件绿证据。没有新增 skip 或改写失败为通过。
- `npm run test --prefix apps/web -- test/eventbus-protocol.test.ts test/eventbus-protocol-log.test.ts test/eventbus.test.ts`：3 个文件、33 个用例全部通过，日志 `.autome/output/r22-related.txt`。
- Node 文件 API 最小检查：创建临时嵌套目录及文件，按修改后的 rm 参数删除，stat 确认 ENOENT，再对已不存在的目标重复 rm，均通过。这是 2 个手工检查场景，不冒充项目注册用例，也不证明瞬时 ENOTEMPTY 的红绿。
- `git diff --check` 通过。未跑项目快检、全量门禁；宿主提交后快检及合并前全量结果为准。

本轮项目测试选择范围共 **40 个用例：33 通过、7 因套件初始化失败跳过**，另有 1 个套件失败；重复单跑不重复计数。相较上一实现轮 21 个为 **+19**，来自范围由 Rust 指针/合并测试切换为 Web 事件总线测试；项目注册测试增减 **0**。宿主上一轮全量 Web 基线为 833，本轮只选择其中 40（其余 793 未跑），不能把子集通过等同全量通过。

## 四项自审

1. **情形表逐行**：本轮没有新增产品情形表；原 live 套件的连接、Origin 拒绝、HMR 共存、跨进程事件、resume、内核重连、SIGTERM 七行情形均保留，实跑受权限阻断。事件协议及总线 33 条通过；清理的嵌套目录与目标已不存在两种情形手工通过。
2. **失败分支**：宿主 ENOTEMPTY 原始证据已定位，修前/修后原命令均执行但被环境阻断，未取得同故障红绿。持续清理失败仍由 await rm 拒绝并交给测试框架，无 catch 吞错。授权、外部服务不可达不适用：只修改测试清理，不新增产品分支。
3. **全部迁移**：关闭 sockets → 等待子进程退出（保留 SIGKILL 兜底）→ 等待目录删除；不新增产品状态迁移。真实关闭路径未实证通过，交宿主复验；未改变原有退出测试。
4. **输入域边界**：嵌套非空目录、已不存在目录检查通过；重试次数固定为 5，持续 ENOTEMPTY 不会无限重试。日期、数值用户输入等不适用，本轮无新增输入域。

## 交接

保留全部里程碑原状态，独立复验应重点运行 eventbus-live 并确认清理通过。若仍失败，应采集残留目录及写入者证据，而非继续增加重试或吞掉错误。未运行付费人工验收 H-01，不启动下一会话。
