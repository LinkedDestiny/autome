# 面板服务进程把环境变量暴露在进程列表里

status: 已定位
fix-round: 3/3
复现命令: npm run build --prefix apps/web && cd apps/web && npx vitest run test/process-listing-env.test.ts
根因: launcher.ts 的 launch() 把 process.title 设成比原始 argv 更短的 `autome-web <home> <action>`，Node 在 macOS 上就地覆写参数区，pgrep -fl 一类读原始参数区的工具会越过新标题读进紧随其后的环境变量区
复核: 打回

## 现象

面板由 `autome web start` 从带令牌的 shell 启动后，`pgrep -fl autome-web` 在面板服务那一行 `autome-web <AUTOME_HOME> serve` 之后跟着启动它的 shell 的环境变量（`名字=值`）；`ps -o command= -p <pid>` 与 `ps -axo pid,command` 则只显示三个词。本轮在本机实测复现（含另一个真实面板实例的行，其中出现了令牌类变量，本文不引用任何值）。

## 复现

命令（单行）：`npm run build --prefix apps/web && cd apps/web && npx vitest run test/process-listing-env.test.ts`

新用例 `apps/web/test/process-listing-env.test.ts`（已登记进 `test-groups.ts` 的 exclusive 组）：用 `harness()` 的临时 `AUTOME_HOME`，把哨兵变量 `AUTOME_LEAK_SENTINEL=<随机串>` 放在环境的第一个键（泄漏从环境区开头读起，所以它是最先暴露的值），`autome web start` 后取三种列法里属于该 pid 的行：先断言三种都找到含 `autome-web` 的那一行，再断言不含哨兵值、也不含其它环境变量的 `名字=值`（失败信息只报名字），再断言 `status` 仍认得实例、`stop` 后无残留进程。需要先 `cargo build -p autome-cli`（harness 的内核包装脚本用 `target/debug/autome`）。

修复前的失败（本机实跑，日志在 `.autome/output/red.log`）：

`AssertionError: pgrep -fl autome-web：1 行含环境变量哨兵值: expected 1 to be +0`

即 `pgrep -fl autome-web` 的输出有 1 行（面板服务自己的那一行）含哨兵；`ps -axo pid,command`、`ps -o args= -p <pid>` 两种没有命中（它们按 ps 记录的长度/标题读取）。

格式化 / lint：对新增测试文件跑 `npx eslint test/process-listing-env.test.ts --max-warnings 0` 无输出通过；整套 `npm run lint --prefix apps/web` 与 `npm run typecheck --prefix apps/web` 均通过。项目没有配置 prettier，不用它。格式化后重跑同一复现命令，仍以同一原因失败（pgrep 视图含哨兵）。

## 根因分析

- `apps/web/server/launcher.ts:134`：`launch()` 一开头 `process.title = \`autome-web ${home} ${action}\``。Node 在 macOS 上没有独立的标题缓冲，直接覆写进程启动时的 argv 内存区。
- 面板服务由 `ensure()` 以 `node <root>/bin/autome-web.js serve [--port N]` 拉起，原始参数区的长度由这条命令决定。当新标题比原参数区短时（典型：`AUTOME_HOME=~/.autome`，标题很短），参数区尾部留下以 NUL 分隔的空隙，而紧随其后就是环境变量字符串。`pgrep -f`（按 argc 数个数、读到整块区域）会把新标题之后的内容——环境变量——一并输出；`ps -o command=` 只读标题，所以看不到。
- 影响范围：只有面板服务的 node 进程（`serve`，以及同样设置了标题的 `start/status/stop` 短命进程）。核对过：`crates/autome-cli/src/web.rs` 只是 spawn node，Rust 进程不改标题；仓库里其它位置没有 `process.title` 赋值；面板带起的内核 `autome core` 是 Rust 二进制，参数区未被改写，不受影响。
- 识别依赖：`apps/web/server/instance.ts:65,68` 用 `ps -o command=` 的输出包含 `autome-web` 判断，修复若改动标题（例如把标题补齐/不设置），需保证这两处与 `status/stop/--force` 行为不变。
- 修复方向（供修复轮参考，不在本轮做）：不改传给内核与会话的环境；要么不再把标题缩短到短于原参数区（把标题补空格到原参数区长度），要么不用 `process.title` 而在启动命令行里直接带 `autome-web` 标识。

## 修复说明

`apps/web/server/launcher.ts`：去掉 `launch()` 里的 `process.title = ...`；`ensure()` 拉起服务时给 `spawn` 加 `argv0: \`autome-web ${home}\``，让进程标识（`autome-web` 与 AUTOME_HOME）由真实 argv[0] 提供，`instance.ts` 的 `ps -o command=` 识别与测试夹具按 home 找进程都不变。

根因补充：`process.title` 只会写成 argv 区里的一个字符串，而 argc 为 3，`pgrep -f` 读完标题后还要再读 argc-1 个字符串，于是读进紧随其后的环境变量区；把标题补空格到原长度（试过）不能解决，因为 Node 会把标题截到原参数区长度且仍只占一个字符串。不设标题、改用 argv0 后，argv 区未被改写。

验证：复现命令通过（1 passed）；`web-instance`、`web-launcher`、`web-launcher-cleanup`、`web-launcher-input` 共 39 个用例全过（第一版「只删标题」会让夹具按 home 找不到进程，5 个用例失败，已因此改用 argv0）；`sh scripts/ci-quick.sh --since <起点>` 全部通过。未改回归测试。

操作者备注：①没有使用 pkill/killall，也没有结束任何进程，只在测试夹具里起停自己的实例；②没有打印不带过滤的进程列表，调试时只打印命中与否和行数；③临时脚本放 `.autome/output/` 并已删除；④本文与提交不含任何环境变量的值。

## 第 2 轮修复说明

上一轮复核被沙箱的 `ps EPERM` 阻塞，未发现产品缺陷；产品代码（`launcher.ts` 的 `argv0`）无需再改。本轮在可执行 `ps` 的会话里独立复验同一修复：

- 复现命令通过（1 passed，退出码 0）。
- `web-instance`、`web-launcher`、`web-launcher-cleanup`、`web-launcher-input` 加泄漏回归共 5 个文件 39 个用例全过（复核所指的 38 个 + 泄漏回归 1 个）。
- `sh scripts/ci-quick.sh --since 2be37b74…` 全部通过。
- 未改回归测试；未跑全量门禁。

### 操作者备注

1. 没有使用 pkill/killall 或任何按名字结束进程的方式；只运行测试夹具，由夹具按自己拉起的 pid 起停，未触碰 7420 端口的实例。
2. 没有打印任何进程列表；终端只看了测试结果与快检汇总。
3. 临时输出放在本 worktree 的 `.autome/output/`（repro3/subset3/quick3 日志），不在 `/tmp`。
4. 本文与提交不含任何环境变量的值，只写变量名。

## 第 3 轮修复说明

上两轮复核都卡在沙箱的 `ps EPERM`，没有指出可证实的产品缺陷；本轮会话里 `ps` 可用，产品代码（`launcher.ts` 的 `argv0`）无需再改，直接在本会话复验：

- 复现命令通过（构建成功，1 passed，退出码 0）。
- `web-instance`、`web-launcher`、`web-launcher-cleanup`、`web-launcher-input` 加泄漏回归，5 个文件 39 个用例全过。
- `sh scripts/ci-quick.sh --since 3b0ce366…` 全部通过（5 步，失败 0）。
- 未改回归测试，未跑全量门禁。

### 操作者备注

1. 没有使用 pkill/killall 或任何按名字结束进程的方式；只运行测试夹具，由夹具按自己拉起的 pid 起停，未触碰 7420 端口的实例。
2. 没有打印任何进程列表；终端只看测试与快检的汇总。
3. 临时输出放在本 worktree 的 `.autome/output/`（r3-*.log），不在 `/tmp`；它们在已忽略的目录里，不入提交。
4. 本文与提交不含任何环境变量的值，只写变量名。

## 复核意见

第 3/3 轮独立复核结论：打回。复核对象为 HEAD `e91ddaf05316d2fad0ed31ad1133ed181a57cae4`，产品修复仍为 `89eb063dc8b9deb6e7ca8548c112e74ec8249030`。本轮重新执行原复现命令及直接相关测试子集，均在真实进程测试的夹具预检处遭遇 `spawnSync ps EPERM`，未取得泄漏消除和实例生命周期的独立通过证据。未发现可证实的产品实现缺陷，不声明回归测试有缺陷。需要宿主提供允许执行 `ps` 的独立复核环境，重验同一提交；不要通过改动或跳过测试绕过环境限制。本轮未修改 fix-round，不自行启动后续会话。

### 提交差异与根因

- 比较定位提交 `7ca8d3ff` 至本轮 HEAD：产品仅修改 `apps/web/server/launcher.ts:66` 的后台 spawn 参数，以及 `:134` 附近的标题赋值和解释注释；其余差异为任务简报与 Bug 文档。产品修复提交至当前 HEAD 的 launcher 差异为空，第 3 轮未新增产品改动。环境继承、端口选择和停止策略未改，修复范围最小。
- 源码证据支持修复方向：移除 `process.title` 赋值，停止改写原始参数区；后台服务用 `argv0` 保留 `autome-web` 与 home 标识。`instance.ts:65,68` 的识别逻辑未改。直接 serve、Rust CLI 和 launchd 的启动链仍经过 `bin/autome-web.js`，保留服务识别字符串。macOS 下 argc/参数区的具体读取机制沿用定位证据，本轮没有重新实测；源码判断不能代替泄漏复验通过。
- 定位至 HEAD 的 `apps/web/test/` 与 `apps/web/test-groups.ts` 差异为空；泄漏回归测试两端 Git blob 均为 `8fc449efbab56ebb0c8594c76bba57064387c002`。没有删除、跳过、改名、改变输入或弱化断言，无回归测试修正需要核对。
- 回归测试有意义：先要求三种视图找到登记 PID 和服务标识，再检查哨兵与继承环境键值，最后验证 status、stop 和无残留，无法以服务未启动冒充修复成功。进程视图在内存按 PID 过滤，泄漏断言报告命中数量或变量名。其它环境变量检查有大写变量名和长度至少 12 的筛选，不能将通过扩大解释为覆盖所有环境变量；位于环境首项的随机哨兵仍直接覆盖本次缺陷。没有发现产品修好后由测试自身错误卡住的证据。本轮未注入临时缺陷，未重跑修复前负向对照。

### 独立执行结果

测试 TMPDIR 指向本 worktree 的 `.autome/output/review3-tmp/`，未改头部复现命令或测试文件；所需 `target/debug/autome` 可执行文件存在。

1. 执行头部原复现命令，退出码 1。Web 构建成功，泄漏回归 1 个失败。错误为 `Error: spawnSync ps EPERM`，调用链为 `test/fixtures/web-launcher.ts:16` 的进程查询 → 第 26 行 harness 预检 → `test/process-listing-env.test.ts:18`。失败早于服务创建和哨兵检查，既不算再次复现泄漏，也不算修复通过。
2. 执行 `cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-launcher-cleanup.test.ts test/web-launcher-input.test.ts`，退出码 1：4 个文件中 2 通过、2 失败；38 个用例中 21 通过、17 失败，无跳过。17 项失败均为相同的 `ps EPERM` 预检错误。加泄漏回归共 39 个，与第 3 轮修复说明一致；真实启动、复用、暂停、停止场景未完成独立验证。
3. `git diff --check` 通过。未执行快检或全量门禁 `sh scripts/ci-local.sh`。用户提供的修复门禁退出码 0 只作为已有证据，不等于本轮独立复验或项目全量门禁通过。未跑全量门禁不是打回原因。

本轮原始输出保存在 `.autome/output/review3-*.log`，提取证据后删除本轮日志与临时目录，不提交临时产物。最终仅修改本 Bug 文档的复核字段和复核意见，产品、测试及其它被跟踪文件未改。

### 操作者备注逐条核对

已核对本轮送达备注、`brief/impl-4.md` 的「操作者备注」及「第 3 轮修复说明」中的逐条回应：

1. 修复说明明确回应只按自建实例 PID 起停、未触碰 7420 实例。本轮未使用 `pkill`、`killall` 或按名字匹配的终止命令；真实进程测试均在创建服务前被预检拒绝，未拉起或终止面板服务，未接触 7420 实例。
2. 修复说明明确回应未打印进程列表。本轮没有独立执行未过滤的进程查询；测试内真实 `ps` 被拒绝，没有取得或打印本机进程列表。终端仅提取测试汇总和错误类型，未输出真实环境值。
3. 修复说明明确回应临时输出放在本 worktree 的 `.autome/output/`，但只说明忽略、不入提交，没有提供用完删除的证明。本轮日志、读取快照和测试临时目录均放在该目录，未使用 /tmp，完成后删除自己创建的产物。历史产物清理情况未独立证明，不擅自清理；此项不能把「不入提交」当成「已删除」。
4. 修复说明明确回应文档与提交不含真实环境变量值。本轮产品差异只涉及变量名与代码表达式，复核内容没有真实环境变量值或令牌。四条备注都有核对记录；此前修复会话实际命令史不在本轮可独立证明范围内。
