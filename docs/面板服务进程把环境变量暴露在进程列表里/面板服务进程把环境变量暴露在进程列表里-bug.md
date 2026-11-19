# 面板服务进程把环境变量暴露在进程列表里

status: 已定位
fix-round: 4/4
复现命令: npm run build --prefix apps/web && cd apps/web && npx vitest run test/process-listing-env.test.ts
根因: launcher.ts 的 launch() 把 process.title 设成比原始 argv 更短的 `autome-web <home> <action>`，Node 在 macOS 上就地覆写参数区，pgrep -fl 一类读原始参数区的工具会越过新标题读进紧随其后的环境变量区
复核: 通过

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

## 第 4 轮修复说明

按操作者备注二处理：本轮不改产品代码，也不改测试，只在本文档记录并重置待复核。

- 产品修复仍是提交 `89eb063d`（`apps/web/server/launcher.ts`：去掉 `process.title` 赋值，后台拉起服务时用 `argv0` 提供 `autome-web <home>` 标识）。自该提交以来 launcher 差异为空；回归测试 `apps/web/test/process-listing-env.test.ts` 未动。
- 前三轮复核都只因沙箱里 `spawnSync ps EPERM` 打回，复核自己写明未发现可证实的产品缺陷。这是环境限制，不是产品或测试问题。
- 需要 `ps` / `pgrep` 的检查以宿主修复门禁为执行证据：宿主在本机不受沙箱限制地执行了头部复现命令（含 `test/process-listing-env.test.ts`）和快检，提交 `89eb063d`、`7dda6002`、`e91ddaf0` 三次退出码均为 0，日志在 `/Users/dannie/project/autome-v2/.autome/output/gates/T-138/`。本轮新提交上内核会再跑一次修复门禁。
- 给复核轮：沙箱里因权限跑不了的检查（进程枚举、监听端口、启动浏览器）记为验证缺口，写明命令与报错，不构成打回理由；静态核对根因是否成立、差异是否超出修复范围、回归测试是否被弱化，三项成立且宿主修复门禁在受审提交上退出码为 0 即应通过。
- 本轮未运行任何进程相关命令，也未跑全量门禁。

### 操作者备注

备注一：
1. 未使用 pkill/killall 或任何按名字结束进程的方式，本轮没有启停任何进程，未触碰 7420 端口的实例。
2. 未打印任何进程列表。
3. 本轮没有产生临时脚本或输出，未使用 /tmp。
4. 本文与提交不含任何环境变量的值，只写变量名。

备注二：
1. 照做：未改产品代码与测试，只新增本节并重置待复核，产生一个新提交。
2. 照做：以宿主门禁为执行证据，见上。
3. 已写给复核轮的说明，见上。
4. 备注一的四条继续有效，见上。

## 复核意见

第 4/4 轮独立复核结论：通过。受审提交为 `be0fdbd1516dbe0c753c00b12acf93758de064c9`，产品修复为 `89eb063dc8b9deb6e7ca8548c112e74ec8249030`。根因与修复方向成立，产品差异最小，回归测试没有删除、跳过或弱化；当前受审提交的宿主修复门禁退出码为 0。按操作者备注二，本轮沙箱中的 `ps EPERM` 仅记为独立验证缺口，不再据此打回，不声明回归测试有缺陷。

### 提交差异、根因与回归约束

- 定位提交 `7ca8d3ff` 至受审提交，产品仅改 `apps/web/server/launcher.ts:66` 的后台 spawn 选项及 `:134` 附近的标题赋值和说明注释；其余差异均在本任务文档目录。`89eb063d` 至受审提交的整个 `apps/web` 差异为空；第 4 轮修复提交只改 Bug 文档。未扩展环境继承、端口选择或停止策略。
- 去掉 `process.title` 赋值，消除改写原始 argv 参数区的入口；后台 spawn 用 `argv0` 提供 `autome-web` 和 home 标识，保持原参数边界。`instance.ts:65,68` 依赖的识别字符串仍存在。Rust CLI 的 `web.rs:64` 继续传入 `bin/autome-web.js`；launchd 的 `service.ts:42` 通过 core web serve 走同一入口。静态证据支持根因与修复；macOS 参数区读取的运行证据采用宿主回归结果，本轮不声称独立验证了操作系统内部机制。
- 定位提交至受审提交的 `apps/web/test/` 与 `apps/web/test-groups.ts` 差异为空；泄漏回归文件两端 Git blob 均为 `8fc449efbab56ebb0c8594c76bba57064387c002`，exclusive 组登记仍在。断言、输入、用例名和跳过标记均未改，也不存在需要复核的测试修正。
- 回归测试先确认服务启动、取得登记 PID、三种进程视图均找到该服务，再检查环境首项随机哨兵及其它符合筛选条件的环境键值，随后验证 status、stop 和无残留。服务未启动或视图未找到进程不能冒充通过。列表先在内存按 PID 筛选，泄漏断言只报告数量或变量名。其它环境值检查限定大写变量名且值长度至少 12，不能将测试通过扩大为覆盖所有环境变量；首项随机哨兵直接覆盖本次泄漏路径。
- 已读宿主定位日志 `repro_check-7ca8d3ff3ddb.log`，原实现确实因 `pgrep -fl autome-web` 中 1 行命中哨兵而断言失败。当前同一测试在宿主通过，具备修复前失败、修复后通过的判别证据。本轮没有注入临时缺陷或重跑旧实现。

### 宿主证据与独立执行结果

宿主证据目录：`/Users/dannie/project/autome-v2/.autome/output/gates/T-138/`。

1. 已核实受审提交对应的 `fix_gate-be0fdbd1516d.exit`：`exit_code=0`，结束时间 `2026-11-19T14:12:55Z`；对应 `.kind` 记录的是本轮提示词中的复现命令与快检组合，种类为 quick。对应 `.log` 显示 Web 构建和泄漏回归实际执行，1 文件、1 用例通过。`.report` 与日志显示 Format、Clippy、Web lint、Web typecheck 沿用缓存结果，快检为执行 0 步、沿用 4 步、失败 0 步；不将它们写成当前提交重新执行。这是当前提交的宿主门禁证据，不以旧提交的通过记录替代。
2. 本轮独立执行头部原命令 `npm run build --prefix apps/web && cd apps/web && npx vitest run test/process-listing-env.test.ts`，退出码 1。Web 构建成功，泄漏回归 1 个失败，错误为 `Error: spawnSync ps EPERM`，位置是 `test/fixtures/web-launcher.ts:16` → `:26` 的 harness 预检 → `test/process-listing-env.test.ts:18`。失败早于服务创建和哨兵检查，属于沙箱权限缺口，既不算泄漏再次复现，也不记为本轮独立通过。
3. 独立执行 `cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-launcher-cleanup.test.ts test/web-launcher-input.test.ts`，退出码 1：4 文件中 2 通过、2 失败；38 用例中 21 通过、17 失败，无跳过。17 项失败均为同一 `spawnSync ps EPERM` 预检错误。加泄漏回归共 39 用例，与此前范围一致；真实进程生命周期尚未在本沙箱独立跑通。按备注二采用宿主进程检查证据，不因该环境限制要求改代码或测试。
4. 测试所需 `target/debug/autome` 可执行文件存在。测试的 `TMPDIR` 指向本 worktree 的 `.autome/output/review4-tmp/`；原始输出保存在 `.autome/output/review4-repro.log`、`review4-subset.log`。未修改测试或头部复现命令。`git diff --check` 通过。本轮未重跑快检，未运行全量门禁 `sh scripts/ci-local.sh`；修复门禁通过不等于项目全量门禁通过，全量由宿主合并前统一运行。

### 操作者备注逐条核对

已核对 `brief/impl-5.md` 中送达的两份备注，以及「第 4 轮修复说明」的「操作者备注」小节；八项均有回应，并核对当前修复提交差异。历史会话的全部操作过程不在本轮可独立证明范围内，不据此要求补充操作者已明确免除的证明。

备注一：

1. 照做。修复说明明确本轮没有启停进程，实际提交只有文档。本复核未使用 pkill、killall 或按名字匹配的终止命令；真实进程测试在创建服务前被预检拒绝，没有拉起或终止面板服务，未接触 7420 实例。
2. 照做。修复说明明确未打印进程列表；本复核未输出本机进程列表，只提取测试汇总和错误类型。测试内真实 ps 调用被拒绝，没有取得列表；没有打印环境值或令牌。
3. 照做。修复说明记录本轮没有临时产物。本复核临时输出和测试目录均在本 worktree 的 `.autome/output/`，不在 /tmp，也不入提交。按备注二第 4 条，不再要求历史产物删除证明，不清理其它会话产物。
4. 照做。修复提交和本轮复核文档均不含真实环境变量值；涉及环境时只记录变量名或代码表达式。

备注二：

1. 照做。已核实第 4 轮修复仅在 Bug 文档新增说明并重置待复核；产品仍是 `89eb063d`，测试未改。本复核也只修改 Bug 文档的复核字段和复核意见，保持 status、fix-round、复现命令、根因及 repos 状态不变。
2. 照做。需要 ps / pgrep 的运行证据采用当前受审提交 `be0fdbd1` 的宿主日志与退出码；前三轮结果只作背景，不代替本次证据。
3. 照做。本轮重新尝试原复现命令与相关子集，将 ps 权限失败明确记为验证缺口。静态核对三项成立且当前宿主修复门禁成功，结论为通过。
4. 照做。备注一四条继续执行；临时产物留在允许的忽略目录且不提交，无需另补删除证明。未运行全量门禁，未合并、推送、切换分支或启动后续会话。

本轮提交范围仅为本 Bug 文档；后续由 Autome 调度。
