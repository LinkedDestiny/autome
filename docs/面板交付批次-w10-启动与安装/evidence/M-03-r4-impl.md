# M-03 · 实现 #4

结论：实现已落地，M-03 保持开放（reopen 0）。沙箱禁止监听和 ps，未取得空 PREFIX 到 HTTP 200、status 同包身份及无残留的完整通过证据；不是协议失败。需宿主/审计环境复验，不能标待审。

## 改动与修复

- 新增 stage-panel.ts：复制运行文件和锁文件的生产依赖，排除 dev、devOptional（TypeScript 属于后者），保留缺失的平台可选依赖语义；缺构建、缺输出、已有输出和源码内部输出明确拒绝。规范化 macOS 路径别名后判断边界。
- package-cli.sh 发布模式构建 CLI/Web 并暂存；--binary 必须配 --web，离线模式不调用 cargo/node/npm。包内 web 与 autome 同级。
- install-cli.sh 精确定位包根 autome；Node 版本按主次号判断，缺失/旧版仅装 CLI；面板安装到 PREFIX 的 ../lib/autome/web，写新、换旧、移入，移入失败回滚旧目录。
- 内核 bundle 档使用 AUTOME_BUNDLED_CORE，args 为 core，优先于 PATH，不再认相邻 automed。README、计划 §7.3、包 README 同步；CI Web build 前移，CLI 两个 job 加 setup-node。
- 上一轮 M-02 审计通过，无退回的判别检查需要搬入。本轮自身测试先抓到 devOptional 被误带入包（红），排除后包结构用例绿；暂存边界先抓到 /var 与 /private/var 别名（红），规范化后 3 条全绿。

## 命令与结果

原始输出均在 `.autome/output/`，以下为摘要，不转存原始输出。

1. `cargo build -p automed -p autome-cli`：成功；`npm run build --prefix apps/web`：成功。仅准备本里程碑依赖，未运行全量门禁。
2. 验收：`cd apps/web && npx vitest run test/install-alignment.test.ts test/core-binary.test.ts test/ci-config.test.ts test/docs-web.test.ts && cd ../.. && sh -n scripts/install-cli.sh scripts/package-cli.sh`。
   - Vitest 最终 25 条，23 通过、2 失败、0 跳过。install-alignment 3/5；core-binary 10/10；ci-config 8/8；docs-web 2/2。
   - 两条失败：空 PREFIX 运行链路、无残留，均为 `spawnSync ps EPERM`。运行链路加了清理能力前置检查，不能检查残留时不先启动子进程；不是跳过。
   - 初次运行尚未加前置检查时已实际遇到 `listen EPERM: operation not permitted 127.0.0.1`；当时已安装并进入同包内核启动，未达到 HTTP 成功。未据此声称无残留验证通过。
   - Vitest 非零使契约中的 && 短路，因此另行执行 `sh -n scripts/install-cli.sh scripts/package-cli.sh`：通过。
   - 输出：m03-acceptance.txt（初次红）、m03-acceptance-final.txt（最终环境阻塞）。
3. 相关子集：`cd apps/web && npx vitest run test/stage-panel.test.ts test/core-sidecar.test.ts`：21/21，通过，0 跳过（新增暂存 3、现有 sidecar 18，含不允许条件跳过的守卫）。输出 m03-related-final.txt。
4. 改动文件 ESLint（stage-panel、install-alignment、core-binary、ci-config、docs-web、stage-panel.test、binary.ts）与 `cd apps/web && npx tsc --noEmit`：通过；`git diff --check`：通过。输出 m03-static.txt。

用例总数按最终非注入运行的唯一用例计：**46 条，44 通过、2 环境失败、0 跳过**。相同文件集合的修改前基线为 37（安装 2 + binary 9 + CI 8 + sidecar 18）；增加 9：安装 +3、binary +1、文档 +2、暂存边界 +3。单独里程碑验收为 19 → 25（+6）。上一轮 M-02 报 35（Rust 11、Web 24），本轮 46 比该报告多 11，但范围已换为 M-03，不能把它当全仓测试增长。

## 标待审之前的自审清单（本轮未标待审）

1. **情形表逐行**
   - A2 暂存文件/生产依赖：包结构、正确 sha256、无 vite/vitest/typescript，通过；缺构建提示通过。
   - A2 发布/预构建两模式：预构建且 PATH 无 cargo/node/npm 的打包通过；发布路径的 cargo 与 Web build 分别成功，未跑联网 npm ci 的完整 release 打包。
   - A2 安装布局与替换：首次运行已完成空 PREFIX 安装与重复替换、旧标记移除，随后监听失败；最终整条用例因 ps 前置拒绝，仍待宿主完整复验。
   - A3 覆盖变量、bundle、PATH、本地、cargo、旧 automed 排除：10 条通过。
   - A4 Node 缺失、21.7.3、22.21.9、22.22.0、23.0.0：安装分支均通过；旧 Node 在 PATH 中用现有 fake-node-21.7.3.mjs。拒绝时检查安装提示及 CLI 诊断。
   - A5 空 home 到打开器/HTTP/status/stop/无残留：已写入测试，但环境阻塞，未通过。
   - A8 README/计划：2 条通过；CI 顺序和 setup-node：8 条通过。
2. **失败分支**
   - 缺 build、缺输出、已有输出、源码内部路径、缺 --web、空 --web、Node 缺失/旧版：均实际检查并通过。
   - 监听失败：初次运行得到 EPERM；进程枚举失败：最终运行得到 ps EPERM；保留失败，不条件跳过。
   - 授权拒绝/外部服务不可达：不适用，本里程碑的本地 --from 验收无授权或外部服务调用；下载脚本原有路径未改。读盘失败通过缺 build 检查；启动超时由既有启动器处理，本轮运行链路阻塞，未另造超时。
3. **全部迁移**
   - 未安装 → CLI-only（缺 Node/旧版）通过；未安装 → CLI+panel（22.22/23）通过。
   - 已安装 → 替换：初次运行完成替换与旧文件清除；已安装 → 服务启动 → 打开 → status → 停止 → 无残留：完整断言已写，但因环境受阻未全部执行，需复验。
   - bundle 解析优先级无持久状态，10 条涵盖各分支；无其他新状态机。
4. **输入域边界**
   - Node 下界 22.21.9/22.22.0、跨主版本 23.0.0、缺失/21.7.3；空参数；含空格的 PREFIX/暂存路径；源码根及其子目录；既有输出：通过。
   - 日期、闰年、最大数值、单元素业务集合：不适用，未新增此类输入域。

## 调用点检查与负向对照

- 枚举全部 `startServer(` 与 `new CoreSupervisor(`，清单 m03-callers.txt。真实内核测试都显式传 binaryPath；core-supervisor/ticker 通过 options 传入；older-kernel-core 是模块替身；生产 main 在唯一解析入口选内核再显式传入 supervisor，无遗漏需改。
- 临时去掉 bundle 变量：core-binary 两条失败，退出 1；还原后 10 条通过。
- 临时强制 node_ok=1：旧 Node 安装用例失败，退出 1；还原后该用例通过。负向用 `-t` 定位，Vitest 把未选的 4 条显示为 skipped，不计正式验收总数。
- 临时还原 CI 为 test 先于 build：CI 顺序用例失败，退出 1；还原后 8 条通过。
- devOptional 漏过滤的初次真实失败已证明包结构检查能抓到开发依赖；--binary 无构建工具检查通过。生产模式/HTTP 链路的负向注入未跑，避免将环境失败当有效负向证据。

## 交付边界

未跑 `sh scripts/ci-local.sh` 或宿主快检组合；未触碰真实账本、launchd 或其他 worktree；未推进 M-04。状态块写 4/65，当前里程碑仍为 M-03。
