# Web typecheck 被 notifier 回归脚本阻断

status: 已定位
fix-round: 1/3
复现命令: cd apps/web && node --test test/web-typecheck-regression.mjs
根因: tsconfig 将 scripts 纳入 allowJs、checkJs、strict 检查，notifier-live-regression.mjs 的参数及 cases 集合缺少类型声明或可推断类型，产生 14 处隐式 any 诊断。
复核: 通过
repos: autome-v2

## 现象

在当前仓库 worktree（基线 `3980c4b`，包含任务卡所述 `b15e6d0`）使用 Node v26.5.0 执行 Web typecheck，退出码为 2。14 条诊断全部来自 `apps/web/scripts/notifier-live-regression.mjs`：TS7006 十处、TS7019 两处、TS7034 与 TS7005 各一处。

原有 `node --test scripts/notifier-live-regression.mjs` 退出码为 0，两条用例通过，说明运行时断言通过不能证明静态类型检查通过。

## 复现

从当前 worktree 根目录执行状态块中的命令；实际检查成员为 `apps/web`（同一 autome-v2 仓库，不是独立 Git 仓库）。前提是已安装锁文件依赖。本轮初始缺依赖，直接 typecheck 因找不到 react-router 退出 127，此结果不计复现；随后在 apps/web 执行 `npm ci --offline --no-audit --no-fund` 成功，全程无需网络。

新增 `apps/web/test/web-typecheck-regression.mjs`，使用 Node 原生测试调用真实 `npm run typecheck`，断言正常结束且退出码为 0；命令启动错误、超时或信号会单独报错。文件位于 `test/`，可被回归测试保护集识别，不依赖错误数量或具体修复手法。

实测：

- `cd apps/web && npm run typecheck`：退出 2，14 条目标诊断。
- 状态块复现命令连续执行两次：均退出 1，断言实际状态 2 不等于期望 0，均包含相同 14 条目标诊断。
- `cd apps/web && node --test scripts/notifier-live-regression.mjs`：退出 0，2/2 通过。
- 新增测试单文件 ESLint：退出 0。

完整原始输出保存在 `.autome/output/typecheck-baseline.log`、`regression-1.log`、`regression-2.log`、`notifier-baseline.log`、`regression-lint.log`，不纳入任务文档或提交。

## 根因分析

`apps/web/package.json` 的 typecheck 顺序执行 `react-router typegen && tsc --noEmit`。本轮已成功进入 tsc，排除路由类型生成失败。

`apps/web/tsconfig.json` 的 include 包含 scripts，且 allowJs、checkJs、strict 均为 true，故 `.mjs` 也接受隐式 any 检查。问题脚本的 load(execFile)、register 参数、路径拼接 rest 参数、execFile 模拟回调参数和 Promise 回调缺少类型信息；cases 空数组在不同位置无法确定元素类型。VM 中动态读取 command 也无法为后续 Promise 回调提供静态上下文类型。

诊断对照仅通过命令行执行 `tsc --noEmit --pretty false --noImplicitAny false`，未修改配置：目标脚本的 14 条诊断消失，但 bell-client/core-restart 测试产生其他类型错误，整体仍退出 2。对照结果仅支持隐式 any 检查是目标诊断来源，不代表关闭检查能修复项目。日志见 `.autome/output/implicit-any-control.log`。

任务卡关于 T-56 合并时宿主门禁关闭的历史描述未独立验证；当前根因与复现不依赖该描述。

## 修复说明

第 1/3 轮只为 `apps/web/scripts/notifier-live-regression.mjs` 补充 JSDoc：注册用例结构、execFile 模拟函数签名、注册参数、路径 rest 参数与 Promise 回调参数类型。模拟签名明确本场景传入的 timeout 为 number。运行逻辑、定时器、两条原有断言和 tsconfig 均未改变，定位轮的 `test/web-typecheck-regression.mjs` 保持原样。

验证结果：

- 修复前执行原复现命令：退出 1，仍报告定位记录中的隐式 any 错误。
- 修复后执行同一个复现命令：退出 0；其中真实 `npm run typecheck` 通过。
- `cd apps/web && node --test scripts/notifier-live-regression.mjs`：退出 0，2/2 通过，无跳过。
- 完整 `sh scripts/ci-local.sh`：退出 1，耗时 479 秒。Format、Clippy、Rust 全量测试、Desktop install、Contract、Web lint、Web typecheck、Web build 通过。Desktop 为 104 通过、1 失败，Electron DOM harness 子进程以 SIGABRT 退出，无 stderr，具体崩溃原因未确定。Web 为 38 文件通过、7 文件失败；347 用例通过、18 失败、19 跳过（测试原有结果，本轮未改跳过条件）。失败日志包含 `listen EPERM: operation not permitted 127.0.0.1`、监听器 `EMFILE` 及相关启动等待超时；当前执行环境不能完成服务监听验收。门禁未全绿，需宿主后续在支持端口监听与 Electron 的环境复跑，不能以本轮静态检查代替。

仓库 `.autome/config.toml` 中宿主 gate 配置已注释，全局配置未发现启用的 gate 段；本轮仍主动执行仓库现有完整门禁脚本。原始输出保存在 `.autome/output/fix-1-before.log`、`fix-1-reproduction.log`、`fix-1-notifier.log`、`fix-1-gate.log`，不纳入提交。

## 复核意见

通过。

- 复现命令独立复验：退出 0，真实 `npm run typecheck` 通过（修复前按记录为 14 条隐式 any 诊断）。
- 回归测试 `apps/web/test/web-typecheck-regression.mjs` 自定位提交 cf20485 起未改动（`git diff cf20485 HEAD` 仅涉及脚本与本文档），未删除、跳过或弱化；它直接调用真实 typecheck 并要求退出 0，与具体修法无关，有意义。
- 根因成立：strict+checkJs 覆盖 scripts 下的 `.mjs`，该脚本缺类型信息。修复只在 `notifier-live-regression.mjs` 添加 JSDoc 类型注解与参数类型，运行逻辑、tsconfig 均未变，范围最小，未靠放宽检查绕过。
- `node --test scripts/notifier-live-regression.mjs`：2 通过、0 失败、0 跳过；apps/web 下 ESLint 无输出（通过）。
- 完整 ci-local 在修复轮因沙箱端口监听 EPERM/Electron SIGABRT 未全绿，与本改动（仅类型注解）无关，本轮未重跑，交宿主门禁在正常环境判定。
