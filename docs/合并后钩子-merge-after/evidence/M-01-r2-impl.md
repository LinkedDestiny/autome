# M-01 实现轮 #2：Desktop 门禁失败复现

本轮为门禁修复轮，implementation-round 为 2/20。M-01 保持待审，不推进 M-02，不增加 reopen。没有上一轮审计文件，简报确认尚无审计轮，因此没有需要逐字搬迁的审计检查。

## 复现与诊断

- Autome 报告提交 `5746d2ecb7` 的 `sh scripts/ci-local.sh` 仅 Desktop 失败；提供的末尾没有 Desktop 失败详情，不能将本轮的具体错误直接等同于该次错误。
- 本轮在 `4917ed2`（上一实现提交之后仅新增简报）启动原命令：`sh scripts/ci-local.sh > .autome/output/impl-2-ci-red.txt 2>&1`。
- 改动前单跑 `npm test --prefix apps/desktop > .autome/output/impl-2-desktop-red.txt 2>&1`：105 个用例，104 通过、1 失败、0 跳过。失败为既有 `electron dom harness: real-DOM assertions against the actual renderer`，Electron 子进程 status 为 null、stderr 为空，未产出 DOM 检查 JSON。
- 最小探针：用 Node 的 `spawnSync(require('./apps/desktop/node_modules/electron'), ['--version'], {encoding:'utf8'})`。不加载本项目 renderer 或 harness 也以 `SIGABRT` 终止，status 为 null、stdout/stderr 均空；原始结果在 `.autome/output/impl-2-electron-probe.json`。重复两次结果一致。
- 排查假设顺序：Electron 启动环境 → 沙箱/系统访问限制 → harness 崩溃。最小探针排除了「必须执行 harness 才触发」；当前 shell 的 `ps` 也被拒绝，支持环境限制的可能性，但尚不能证明原始 Autome 门禁与本轮同因。已有 macOS Electron 崩溃报告的栈指向 `_RegisterApplication` / `NSApplication`，它们不是本轮对应时间的报告，不能作为本轮根因定论。

## 改动与复验

`apps/desktop/test/dom.test.js` 保持现有真实 Electron/CSP/DOM 验证路径、60 秒超时与成功条件，仅补齐 `proc.error` 断言，并在异常退出消息中显示 signal 与 binary。原来只显示 `exited null`，无法区分启动错误、信号终止和普通非零退出。

- 改动后原有单文件检查 `node --test apps/desktop/test/dom.test.js > .autome/output/impl-2-dom-diagnostic.txt 2>&1`：1 个用例、0 通过、1 失败、0 跳过；现在明确报告 `signal: SIGABRT`。这是诊断信息改善，**不是红转绿或修复完成**。
- `node --check apps/desktop/test/dom.test.js`、`git diff --check`：退出 0。
- 没有删除、屏蔽或放宽任何测试，没有把异常退出改成 skip，没有修改产品行为或 `.autome/` 配置/规则（仅按要求写 output 日志）。

## 标待审之前的自审清单

本轮保持既有待审状态；按协议仍列明四项。

1. **情形表逐行**：本轮不改变 M-01 配置及 plan 情形表。全量门禁中的 domain/config_io 用例用于回归其工作区、无提交、Failed、Skipped、NotRun、Passed 判定，以及缺省、空白、显式配置、保存保留情形。Desktop 诊断方面，已实跑信号退出分支；真实 DOM 成功分支受 Electron 启动失败阻断，未取证。
2. **失败分支**：既有 DOM harness 异常退出已复现，新增消息包含 SIGABRT；启动错误通过新增 `assert.ifError` 明确失败，超时仍由既有 ETIMEDOUT 断言拒绝。未额外注入启动错误/超时，不将静态核对称作实跑。M-01 非法配置分支由全量现有测试回归；授权、网络、合并钩子执行超时不适用，本轮不涉及这些运行时路径。
3. **全部迁移**：不适用，本轮不改状态机或多通道行为；M-01 纯判定仍由既有测试验证。
4. **输入域边界**：已实跑 status=null、signal=SIGABRT、stderr 空串的异常边界。日期、数值范围、集合等不适用于本次诊断消息改动；M-01 的空命令、非法类型、超时上下界由原有测试回归。

## 门禁结论与交接

完整门禁的 Desktop 阶段是在诊断改动之后执行的，仍明确以 SIGABRT 失败；Rust 部分代码本轮未改动。当前不能宣称门禁通过。环境问题不设为协议失败；后续需从 Autome 宿主重跑取得完整 Desktop 错误，若为相同的 Electron 启动故障，在具备 macOS GUI 服务访问的环境验证；若为 DOM 断言失败，则依据具体断言修复。当前权限模式禁止提权，未尝试绕过沙箱。

## 用例总数与基线差

- Rust `--workspace --all-targets`：总数 1204 = 1203 通过 + 1 个既有 ignored，0 失败。设计轮总数 1196，本轮较该基线 +8，全部是实现 #1 的 D1–D4、C1–C4；本轮新增 0。与上一实现轮实际执行的受影响目标对比：domain 388、automed lib 651，共 1039，差额 0。首次实跑其余 Rust 目标不算新增测试。
- Desktop：总数 105，104 通过、1 失败、0 跳过；较本轮改动前红基线 105 差额 0。上一实现轮未跑 Desktop，Autome 提供的摘录也没有 Desktop 总数，所以不能声称已取得上一轮完整 Desktop 基线。由于 Electron 启动失败，harness 内动态 DOM 子用例没有被注册执行，105 不能当作完整 DOM 覆盖数。

- Web：总数 301 = 271 通过 + 11 失败 + 19 跳过；与用户给出的 Autome 上轮 301 总数差额 0，实际执行从 301 降到 282（-19），通过数 -30。19 个来自 eventbus-live（7）、notifier-live（3）、server（9）的套件初始化失败；11 个失败来自 gateway-http。错误包含 `listen EPERM: operation not permitted 127.0.0.1` 与 `File watcher error: EMFILE: too many open files, watch`，不是新增或删除了测试。

## 完整命令最终结果

`sh scripts/ci-local.sh > .autome/output/impl-2-ci-red.txt 2>&1`：退出 **1**，耗时 723 秒。Format、Clippy、Rust Test、Contract、Web lint、Web typecheck、Web build 均通过；Desktop、Web test 失败。依赖已经存在，本轮未触发 install；未跑可选 package。完整门禁与单跑 DOM 都未绿，本轮只改善失败诊断，未完成门禁修复；需 Autome 在提交后的宿主环境重新执行同一命令并保留 Desktop 完整失败详情。没有用模拟 DOM、取消沙箱、跳过测试或重复碰运气来制造通过结论。

协议核对：状态字段均唯一，implementation-round 为 2/20；M-01 仍待审，M-02～M-04 仍开放；retro 本轮一行、五段、不超过 200 字。`git diff --exit-code HEAD -- .autome/` 退出 0。
