# M-02 实现 #3

结论：版本检查实现与专项验收通过；全量验收受沙箱阻塞，M-02 保留开放，不标待审。本轮只推进 M-02，未改动其它里程碑。

## 实现与审计衔接

- `server/node-version.js` 无 import，导出单一最低版本 22.22.0 与逐段整数比较函数；不可解析版本拒绝。
- `server/start.js` 唯一静态 import 为版本模块，先检查 `process.versions.node`，不满足时输出约定中文提示并退出 1；通过后才动态加载 `main.ts`。
- 两个 preload fixture 只用于测试，生产代码无测试开关；新增 12 个 vitest 用例，包括 TypeScript AST 结构检查。
- 上一轮审计 M-01 通过，无退回缺陷，故没有需要逐字迁移并先红后绿的审计回归检查。

## 命令与结果

1. `npm test --prefix apps/web > .autome/output/m02.txt 2>&1`：exit=1。共 3 文件、24 个用例；17 通过，7 跳过，server suite 的 beforeAll 失败。相比上一轮审计基线 12 个用例增加 12 个，全部来自 node-version（版本表 8、中文消息 1、拒绝进程 2、结构 1）。原有 server 7 个未执行，不计作通过。
2. `npm test --prefix apps/web -- test/node-version.test.ts`：exit=0，1 文件、12/12 通过，日志 `.autome/output/m02-version.txt`。
3. `PATH="/opt/homebrew/opt/node@22/bin:$PATH" npm test --prefix apps/web -- test/node-version.test.ts`：exit=0，Homebrew Node 22.23.1，12/12 通过，日志 `.autome/output/m02-node22.txt`。这是专项检查，未声称本轮 Node 22 的完整服务验收通过。
4. `node .autome/output/m02-mutations.mjs`：exit=0，五种错误实现分别使测试 exit=1，脚本 finally 恢复两个源文件。日志 `.autome/output/m02-mutations.txt` 与 `m02-negative-*.txt`。
5. 恢复后 `npm test --prefix apps/web -- test/node-version.test.ts test/config.test.ts`：exit=0，2 文件、17/17 通过（原有 config 5 + 本轮 12），日志 `.autome/output/m02-restored.txt`。
6. 独立执行 `node:net` 的 `createServer().listen(0, '127.0.0.1')`，error 回调记录错误并设置 exitCode=1：exit=1、EPERM，日志 `.autome/output/m02-listen.txt`。这项不加载项目代码，确认监听权限阻塞。全量同时出现 `File watcher error: EMFILE`，未通过修改产品监听或跳过断言规避。

下限靠依赖 engines 声明、22.23.1 实测与版本检查单测支撑，未在 22.22.0 上运行。22.23.1 本轮实测范围为版本专项；服务正路径历史证据见 M-01-r2-audit.md，不冒充本轮通过证据。

## 标待审之前的自审清单

1. **情形表逐行**：9.9.0、21.7.3、22.9.0、22.21.1 均拒绝；22.22.0、22.23.1、26.5.0 均接受；abc 拒绝。8 行均通过。中文提示完全匹配当前版本与下限；两种 preload 子进程都退出 1 且无启动日志；AST 检查唯一静态依赖、版本模块无 import、检查先于动态加载，以及 engines/dev/start 一致，全部通过。
2. **失败分支**：版本过低（21.7.3 与 22.21.1）各跑真实子进程，退出 1，stderr 中文、stdout 无已启动，通过；不可解析版本由单测拒绝，通过。授权、读盘失败、超时、外部服务不可达不适用：§4 未定义这些版本检查分支；超时仅是测试进程兜底，未触发。
3. **全部迁移**：入口→拒绝→退出 1 已通过两种子进程检查；入口→接受→动态加载由 AST 顺序检查与版本单测通过，实际 dev/prod 启动尝试在监听阶段 EPERM，未获得运行态证据，因此保留开放。无持久状态机或多通道。
4. **域边界**：空串、单段、缺 patch、多段、负数、预发布后缀、前置空格、尾随换行、Infinity、超出安全整数均拒绝；前导零 022.022.000 按整数接受；22.21.999 拒绝，22.22.0/22.22.1/23.0.0 接受；0.0.0 拒绝，最大安全整数组件 9007199254740991.0.0 接受。均在 8 个表用例内附加断言执行，未增加用例计数。日期边界不适用：输入为版本号。

## 负向对照与恢复

- 字符串比较：4 失败，包括 9.9.0 与 22.9.0；证明按字典序会错误放行。
- 只比较主版本：5 失败，包括 22.9.0、22.21.1 与伪造 22.21.1 进程。
- 下限降为 22.0.0：6 失败，包括结构检查。
- start.js 静态 import main.ts：结构用例失败（1 失败）。
- 删除入口检查（对应检查移至 main.ts 后入口不再负责拦截）：4 失败，包括结构用例。
- 所有注入均恢复，恢复后 17/17 通过；负向对照不计入项目用例总数。

## 下一步与阻塞

需在允许本地监听且文件监听资源可用的执行环境重跑 M-02 全量验收；取得 24/24 通过后才能标待审。未修改设计验收标准，未添加跳过条件。实现轮计数已更新为 3/20。
