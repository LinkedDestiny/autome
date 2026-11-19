# 面板服务进程把环境变量暴露在进程列表里

status: 已定位
fix-round: 1/3
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

## 复核意见

本轮结论：打回，原因是独立复验被当前沙箱阻塞，未取得泄漏场景及实例生命周期的独立通过证据；尚未发现可证实的产品实现缺陷，也不声明回归测试有缺陷。请在允许执行 `ps` 的环境对同一修复提交复验，不要为绕过沙箱删除、跳过或弱化测试。本轮未运行全量门禁，缺少全量门禁结果不是打回原因。

### 提交差异与根因

- 核对定位提交 `7ca8d3ff` 至修复提交 `89eb063dc8b9deb6e7ca8548c112e74ec8249030`：产品仅修改 `apps/web/server/launcher.ts`，增加 spawn 的 `argv0`、移除 `process.title` 赋值并添加解释注释；其余差异是任务简报与 Bug 文档。产品改动范围最小，未改变环境继承、端口选择或停止策略。
- 根因与修复方向在源码层面成立：停止就地改写标题，避免触发文档描述的原始参数区读取问题；后台启动的 `argv0` 保留 `autome-web` 与 home 标识。直接 `serve` 及 Rust CLI 入口仍通过 `bin/autome-web.js` 启动，命令行保留实例识别所需的 `autome-web`。`instance.ts` 的识别与停止逻辑未改。macOS 上的实际泄漏消除尚未由本轮运行证明。
- 定位以来 `apps/web/test/` 与 `test-groups.ts` 差异为空，回归文件的 Git blob 均为 `8fc449efbab56ebb0c8594c76bba57064387c002`。没有删除、跳过、改名、改变输入或弱化断言。
- 回归测试有判别意义：三种进程视图必须找到登记 PID 的服务，才检查哨兵与环境键值泄漏；之后检查 status、stop 与残留进程，避免以服务未启动或标识消失冒充修复。实际进程输出先在内存按 PID 过滤，泄漏断言只报告命中数量或变量名。未发现需要退回定位轮修改测试的缺陷。

### 独立执行结果

测试进程的 `TMPDIR` 指向本 worktree 的 `.autome/output/review-tmp/`，未改变测试文件或复现命令。

1. 执行头部原复现命令，退出码 1。Web 构建成功；回归测试 1 个失败，错误为 `Error: spawnSync ps EPERM`。调用链为 `test/fixtures/web-launcher.ts:16` 的 `processes()` → 第 26 行夹具预检 → `test/process-listing-env.test.ts:18`。失败发生在创建面板子进程和哨兵检查之前，不能算再次复现泄漏，也不能算修复通过。
2. 执行 `cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-launcher-cleanup.test.ts test/web-launcher-input.test.ts`，退出码 1，共 38 个用例：21 通过、17 失败，无跳过。17 项全部在同一 `ps EPERM` 预检失败；实际启动、复用、暂停、强制停止及清理行为未完成独立验证。修复说明的“四个文件共 39 个”与本轮收集数量不符；这四个文件为 38 个，加泄漏回归才是 39 个，测试文件本身没有减少。
3. `git diff --check` 通过。未执行快检或 `sh scripts/ci-local.sh`，未把宿主提供的修复门禁退出码 0 当作本轮独立通过或项目全量门禁通过。

原始输出临时写入 `.autome/output/review-repro.log` 与 `.autome/output/review-subset.log`，结论提取后按操作者要求清理，不提交临时产物。没有进行缺陷注入，没有修改产品、测试或格式化文件。

### 操作者备注逐条核对

1. 修复说明已回应禁止按名字批量结束进程。本轮未使用 `pkill`、`killall` 或按名字匹配的终止命令；真实进程测试在预检即失败，未拉起或终止面板服务，未触碰 7420 实例。
2. 修复说明已回应不得打印其它进程列表。回归源码只保留登记 PID 的进程行；本轮 `ps` 被沙箱拒绝，未获得或打印任何真实进程列表，也未运行独立的不带过滤的进程查询。
3. 修复说明已回应临时脚本位置及清理。本轮原始输出和测试临时目录均置于 `.autome/output/`，完成后删除本轮创建的文件与临时目录，不清理其它轮次产物。
4. 修复说明已回应不得写入真实环境变量值。核对定位至修复的产品与 Bug 文档差异，以及本轮新增内容，未发现真实令牌或环境变量值；只涉及变量名与代码表达式。四条备注均有处理说明，无遗漏；此前会话的实际命令执行史不在本轮可独立证明范围内。
