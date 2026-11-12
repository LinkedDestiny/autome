# M-01 实现轮 #2：门禁复现与环境阻塞

## 范围与结论

本轮按修复轮指令处理 `f00e2d9cae` 的宿主门禁失败，不推进 M-02。M-01 保持「待审」，implementation-round 为 `2/25`。没有上一轮审计文件，也没有需要逐字迁入测试体系的审计检查。

未取得 `sh scripts/ci-local.sh` 全绿证据，不能称宿主门禁已修复。当前会话的 Electron 启动中止与回环端口监听拒绝，使本地无法完成对应验收；不得通过跳过真实测试、放宽断言或增加超时伪造通过。宿主必须在本轮提交上重跑门禁，并保留 Desktop/Web 完整失败详情。人工 H-01–H-03 未执行。

## 复现、判别与修改

- 原样启动 `sh scripts/ci-local.sh > .autome/output/r2-ci-before.txt 2>&1`；Rust 检查与测试通过，Desktop 失败。命令启动时尚未修改代码；运行期间只补充了 Desktop 失败诊断，没有修改生产逻辑或通过判据。本日志不是固定提交的发布证明。
- `npm test --prefix apps/desktop`：总计 105 个用例，104 通过、1 失败、0 跳过。real-DOM harness 返回 `status=null`、stderr 为空，原消息无法区分信号中止与断言错误。日志：`.autome/output/r2-desktop-before.txt`。
- 最小 Electron 探针：通过 `spawnSync(require('electron'), ['--version'])`，尚未加载 harness 即得到 `status=null, signal=SIGABRT`；直接启动 harness 同样 SIGABRT。没有据此推断费用实现错误，也没有借旧崩溃报告认定本次崩溃的精确系统根因。
- 唯一代码变更：`apps/desktop/test/dom.test.js` 原退出码断言的错误消息增加 `proc.signal` 与 `proc.error.message`，不改退出码期望、超时、跳过条件和任何 DOM 断言。理由同步写入设计 §5。
- 修改后原样跑 `node --test apps/desktop/test/dom.test.js`：1 个用例仍失败，明确显示 `signal: SIGABRT, spawn error: none`。日志：`.autome/output/r2-dom-diagnostic.txt`。这是诊断改善，不是红转绿；没有宣称 Electron 已修复。
- `npm test --prefix apps/web -- test/security.test.ts`：14/14 通过，包括宿主尾部指向的「开发服务只保留一个 upgrade 守卫且关闭 Vite CORS」。日志：`.autome/output/r2-security-before.txt`。单跑通过不能证明宿主全量的两项失败已解决。
- `npm test --prefix apps/web`：出现文件监听 `EMFILE`，notifier-live 3、eventbus-live 7、server 9 个用例因初始化失败未执行；随后未正常结束，主动中断（退出 130）。没有完整的 301 用例结果。日志：`.autome/output/r2-web-before.txt`。
- 单变量对照 `ulimit -n 4096; npm test --prefix apps/web -- test/notifier-live.test.ts`：仍出现 EMFILE，并明确报 `listen EPERM: operation not permitted 127.0.0.1`，3 个用例均因 suite 初始化失败跳过。日志：`.autome/output/r2-notifier-limit.txt`。提高上限没有修复问题，故没有将 ulimit 改进门禁脚本。
- 不导入项目的 `node:net.createServer().listen(0, '127.0.0.1')` 同样退出 1、报 EPERM；这是会话环境限制的直接对照。日志：`.autome/output/r2-listen-probe.txt`。此处 EPERM 并非已证实与宿主两项 Web 失败同根因。

## 验收与用例总数差

- `cargo test -p autome-domain --lib cost_summary_`：4 个通过，日志 `.autome/output/r2-cost-summary.txt`。门禁内 `cost_rollup_task_get_sums_two_sessions_and_counts_missing_cost` 通过；M-01 共 5 个验收用例，与上一轮 5 个相比差 **0**，无新增、删除或弱化。
- 门禁内 Rust 各套件为 25、14、388、1（忽略）、648、0、104、16、7 个，总计 **1203** 个：1202 通过、1 个既有忽略。上一轮证据只记录 domain 全量 388 与 M-01 验收 5，没有 workspace 总量；因此不编造 workspace 的跨轮差值。domain 仍为 388，差 **0**；本轮没有改 Rust 测试。
- Desktop 本轮复现基线 **105**，本轮不增删测试（差 **0**）；DOM 子用例因 Electron 启动失败未展开，不能把这 105 个称为完整 DOM 覆盖。宿主提供的尾部没有 Desktop 总数，无法计算宿主总数差。诊断修改后的单文件检查总数 **1**，与同文件原基线相同。
- Web 宿主基线 **301**（299 通过、2 失败）。本轮完整运行未完成，无法给出完整实际总数或可信的全量差；不得把单文件 14 个通过替代 301 个。测试定义未变；security 子集 14 个相对同子集差 **0**，notifier 子集 3 个相对同子集差 **0**，但后者全部未执行。

## 标待审之前的自审清单

本轮保持已有待审状态，不重新标记新里程碑；仍执行四项自审：

1. **情形表逐行**：已结束 Some(1.25)/Some(2.5) 求和 3.75、None 计零且缺失数 +1、Running 不计入且不生成角色键，均由本轮 4 个聚合单测通过覆盖。真实费用 e2e 在全量门禁通过。Desktop 的正常退出判据仍为 0；信号退出仍失败并报告信号，已实跑 SIGABRT 分支。
2. **失败分支**：空集合、缺失费用通过；M-01 无新增授权、读盘、外部服务或超时分支，不适用。门禁中的 Electron 启动中止、Web 监听 EPERM 已实际触发且仍失败；不吞错、不跳过。spawn 错误字段仅用于诊断，本轮没有添加新的错误恢复行为。
3. **全部迁移**：聚合单测覆盖 Running 到 Exited(0)、Exited(1)、Killed、Vanished 四种结束状态，以及 6 个 Role、3 个 BugStep、intake、onboarding 全部键。本轮实际运行通过。诊断修改没有状态机，不适用额外状态迁移检查。
4. **输入域边界**：空集合、单个 None、单个 0、小数 0.001、大额 1,000,000 已在聚合单测执行通过；金额不取整。日期、前导零与字符串解析不适用：纯聚合未增加这类输入。没有靠新增测试数量掩盖原覆盖缺失。

## 最终门禁状态

原始门禁在 Web test 初始化失败后未正常结束，已主动中断（退出 130），没有完整门禁通过结果。此前 Format、Clippy、Rust 全量、Contract、Web lint、Web typecheck 均已通过；Desktop 失败，Web test 未完成。Web build 另行补跑通过（退出 0），结果见 `.autome/output/r2-web-build.txt`。`node --check apps/desktop/test/dom.test.js` 与 `git diff --check` 通过。

下一节点需要宿主运行权限：能启动 Electron 的 macOS 图形会话、能监听回环端口的 Node 环境；以 Autome 在本轮提交上运行的门禁结果为准。没有将状态设为「协议失败」，没有关闭 M-01，也没有推进后续里程碑。

协议格式检查通过：状态字段齐全、M-01 待审及其余 4 项开放保持不变、证据指针唯一、retro 单行五段且少于 200 字。
