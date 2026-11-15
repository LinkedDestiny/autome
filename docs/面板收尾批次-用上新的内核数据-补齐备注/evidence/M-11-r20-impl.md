# M-11 · 实现轮 20/65

结论：实现已落地；验收受沙箱 `spawnSync ps EPERM` 阻塞，保持「开放」，未冒充取得通过证据。未触及真实账本，真实进程用例在 preflight 即失败、没有启动子进程。上一轮审计 M-10 已通过，无退回缺陷需逐字搬入。

## 改动

- 新 `server/core/binary-watch.ts`：每 3000 ms stat，连续两次 ENOENT/ENOTDIR 后触发一次；存在即清零，stop 清理计时器。其它读盘错误记日志并清零，不将无法判断当成文件消失。
- `startServer` 注册完成后启动监视（避免等待内核启动期间触发未就绪的退出回调），close 首先停止监视；默认回调只记日志。
- `runFromEnv` 接入明确中文退出日志与既有 shutdown，完成清理后 exitCode=0；清理失败仍为 1，重复信号不重复清理。
- 新 unit 5 例、kernel 1 例并登记 test-groups。真实用例复制当前检出的 automed，临时 HOME/AUTOME_HOME，通过同一 runFromEnv 入口启动；断言副本删除后 15 秒退出、实例删除、预先追踪的内核与面板均无残留，finally 仍清理。

## 命令与结果

- 契约原命令：`cargo build -p automed -p autome-cli && npm run build --prefix apps/web && (cd apps/web && npx vitest run test/core-binary-watch.test.ts test/web-binary-gone-real.test.ts test/test-groups.test.ts)`。
  Rust 与 Web 构建通过；测试总数 10，9 通过、1 因 ps EPERM 失败、0 跳过。日志 `.autome/output/m11-build-core.txt`、`m11-build-web.txt`、`m11-tests-final.txt`。首次同命令同结果见 `m11-tests.txt`。
- 直接相关子集：`(cd apps/web && npx vitest run test/core-binary-watch.test.ts test/test-groups.test.ts test/core-restart.test.ts)`：15/15 通过，见 `.autome/output/m11-unit-final.txt`。
- 定向 eslint：`(cd apps/web && npx eslint server/core/binary-watch.ts server/main.ts test/core-binary-watch.test.ts test/web-binary-gone-real.test.ts test-groups.ts)`：通过，见 `.autome/output/m11-eslint.txt`。曾发现 prefer-const，已以显式 undefined 初始化修正生命周期变量。
- 负向对照：仅将 `missing < 2` 临时改为 `missing < 1`，原 unit 文件 5/5 失败，含「一次缺失不触发」；还原后 5/5 通过（包含在 15 例子集中）。日志 `.autome/output/m11-negative-unit.txt`。变异已还原。
- 「不接退出回调」「不走 shutdown」两个真实进程负向对照未执行：原正向用例已被 ps 权限拒绝，不能在此环境形成有效判别。宿主须补正向与两个对照。
- 不运行全量门禁或宿主快检；只执行契约与直接相关子集。

## 用例总数与基线

本里程碑原设计 9 例，现 10 例（+1：读盘权限错误不能误判连续缺失）。同选择范围在实现前只有 test-groups 4 例，新增 6 例；core-restart 既有 6 例不变。因此本轮唯一用例总数 16：15 通过、1 环境失败，无跳过。上轮 M-10 审计基线为 vitest 18 + Playwright 1 = 19，本轮 16，差 -3 来自里程碑选择范围不同，并非删测；没有用不同范围的总数声称全项目覆盖变化。

## 四项自审清单

1. **情形表逐行**：G.2 一次缺失不触发（unit 通过）、连续两次只触发一次（通过）、恢复清零（通过）、stop 后停止（通过）；默认回调只记录与 runFromEnv 退出接线已检查源码。真实删除副本后的退出/实例清理/无残留三项已写进 kernel 用例，但 ps EPERM 阻塞，未取得运行证据。
2. **失败分支**：二进制缺失由 unit 模拟通过；读取失败 EACCES 注入通过并确认计数清零；15 秒超时由真实用例有界轮询表达，环境阻塞；授权、空输入、外部服务不可达不适用：本模块仅检查启动时确定的本地二进制路径，无这些接口。
3. **全部迁移**：0→1、1→触发、1→0（恢复）、错误→0、触发后不再检查、stop 后不再检查均由 unit 实跑；生产运行→shutdown→停止内核/删实例/退出由真实用例覆盖但未实跑。启动失败关闭时监视尚未建立，用可选 stop 处理，代码检查无提前启动的定时器。
4. **域边界**：5999 ms 不触发、6000 ms 触发；缺失 1 次/2 次；恢复后重新累计；重复 stop，均实跑通过。日期、前导零、最大字符串、集合单元素不适用：没有此类用户输入域。

## 后续

宿主允许 ps 与端口监听的环境运行原契约与真实负向对照；取得通过证据后再标待审。本轮保持 implementation-round: 20/65，不增加 reopen、不标协议失败。
