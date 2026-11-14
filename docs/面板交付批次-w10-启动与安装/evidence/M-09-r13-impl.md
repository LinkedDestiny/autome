# M-09 · 实现轮 13

结论：实现与静态检查已推进，**保持开放，不标待审**。指定依赖下载失败，浏览器验收未执行；另有 `ps EPERM` 阻止夹具启动。没有推进 M-10，没有运行全量门禁或启动下一会话。

## 实现范围

- 增加精确版本 `@playwright/test@1.64.0`、锁文件、Playwright 单 worker / 零重试 / 170 秒总时限配置、独立预检入口。
- 真实内核夹具复用 w7-flow：私有 HOME/AUTOME_HOME、需求与 Bug、30 张草稿、400 行阻塞日志假 CLI。服务显式指定检出内核，tick 1000 ms；产品默认仍为 3000 ms。
- 一屏 11 个状态、W7/W9 五条独立浏览器用例；控制台错误作为失败；复制检查真实剪贴板，不替换浏览器 API。
- 共用进程扫描捕获父子孙、TERM 后升级 KILL 并复验；Playwright 全局清理以及 Vitest 清理复用。新增 4 条进程扫描回归。
- 本地门禁安装指纹、Web e2e 步骤、GitHub 独立 e2e job、Vitest 排除 e2e、TS 纳入 e2e、不许条件跳过守卫。
- 上轮审计为 M-08 通过，没有退回 M-09 的判别检查，因此无须搬入上轮红绿用例。

## 命令与证据

原始输出均在 `.autome/output/`，未复制到设计正文。

| 检查 | 结果 | 原始输出 |
|---|---|---|
| `npm install --prefix apps/web --save-dev --save-exact @playwright/test@1.64.0 --offline` | ENOTCACHED | r13-install.txt |
| 同版本联网安装（独立临时 npm cache，0 重试） | registry.npmjs.org ENOTFOUND | r13-install-online.txt |
| `npm install --prefix apps/web --package-lock-only --offline --ignore-scripts` | 通过。新增 registry 条目后由 npm 解析规范化；不等于依赖 tarball 已下载或校验 | r13-lock.txt |
| `npm ci --prefix apps/web --offline --ignore-scripts` | ENOTCACHED；现有 node_modules 已恢复。前一次锁文件缺嵌套 fsevents 的 EUSAGE 已修正，再次执行进入实际下载阶段 | r13-ci-attempt.txt |
| `cd apps/web && npx vitest run test/e2e-preflight.test.ts test/core-sidecar.test.ts test/ci-config.test.ts` | 2 文件通过、1 文件导入失败；30 条实际执行全过，预检文件因缺 @playwright/test 未收集到其 3 条 | r13-accept-vitest.txt |
| `cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts` | 缺 @playwright/test，配置加载失败，16 条均未执行 | r13-accept-browser.txt |
| `cd apps/web && npx vitest run test/process-scan.test.ts test/ci-config.test.ts test/core-sidecar.test.ts` | 34 条通过，0 失败，0 跳过 | r13-related.txt |
| `npm run lint --prefix apps/web` | 通过 | r13-lint-final.txt |
| `npm run typecheck --prefix apps/web` | 未通过：缺 @playwright/test 及衍生的类型推断错误 | r13-types.txt |
| 临时配置只映射已有 Playwright 声明的 `tsc --noEmit` | 通过，仅排查自身类型错误；声明来自本机 alpha 版本，不是锁定依赖验收，不替代正式 typecheck | r13-types-diagnostic.txt |
| 独立启动 harness 并 finally 清理 | `spawnSync ps EPERM`，启动预检即拒绝，尚未起服务/内核/假 CLI | r13-harness.txt |
| `git diff --check` | 通过 | 无输出 |

依赖恢复命令最初包含 `rm -rf`，被自动审批拒绝；未执行。后改为目录重命名备份、恢复，无删除原依赖。

## 用例总数与基线差

- 可比基线：core-sidecar 18 + ci-config 9 = **27**。本轮同范围变成 19 + 11 = **30（+3）**，分别增加 e2e 跳过守卫 1 条、CI job 与安装指纹 2 条。
- 加上新增进程扫描 4 条，实际执行的唯一用例总数 **34（相对可比基线 +7）**，全部通过；重复命令不重复累计。
- 新写但未运行：预检 3 + 浏览器 16 = **19**。预期总数为 **53（相对可比基线 +26）**；不能把未收集的 19 条记为通过或“0 跳过所以齐全”。
- 上轮 M-08 实跑总数为 43（desktop 22、styles 12、ci-config 9）。本轮实际 34，比该轮 **少 9**：移出不相关 desktop/styles 34，加入 core-sidecar 19、process-scan 4、ci-config 新增 2，即 43−34+19+4+2=34。差来自里程碑范围与依赖阻塞，不是删测试。

## 标待审前自审清单（未满足项保留阻塞）

1. **情形表逐行**：D6 总览、命令面板、铃铛、项目、全部已完成、新任务、需求展开日志、Bug、添加项目补全、设置、环境共 11 行，逐一映射 layout.spec；D7 日志保持、详情一屏、三页一屏、路径补全、复制共 5 行，逐一映射 manual-items.spec。源码核对完成，16 行浏览器实跑均因依赖阻塞未执行，不能判通过。D1/D2/D8 配置与指纹由 ci-config 的 11 条通过；D9 扫描四情形通过，但真实清理仍待允许 ps 的环境。
2. **失败分支**：缺 chromium、仅缺 headless shell、缺 build、缺内核已写入 3 条预检测试，因导入失败未执行；ps 拒绝在实际启动预检复现，且不能伪装无残留的单测通过；TERM 超时升级 KILL 单测通过。授权拒绝、外部业务服务不可达不适用，本里程碑不增加授权或外部业务调用。npm 下载不可达已实际记录，未条件跳过。
3. **全部迁移**：进程清理父进程已退出→仍追踪被收养子孙→TERM 清理，以及 TERM 超时→KILL→空集合均由单测通过；启动失败→全局清理路径实际遇到 ps 权限拒绝，无服务启动。浏览器日志展开→暂停滚动→CLI pause→时间线新增→保留展开/位置、补全输入→建议→点选、复制→剪贴板三组迁移已写，未实跑。无生产业务状态机变更。
4. **域边界**：固定 1512×944；私有 HOME、`~/`、无原生桥接、30 张草稿触发列滚动、400 行日志中段、偏差 ≤2 px、浏览器单独缺失、无残留集合、无关进程保护均有对应断言。进程部分通过，其余需浏览器环境执行。日期/闰日/前导零不适用：本轮未改变日期或数字解析输入域。

## 后续与提交

在可下载指定依赖、允许 ps 与本地监听的宿主执行 `npm ci`、构建前置及 M-09 原验收命令，修复首次真实浏览器执行暴露的问题后才标待审。不得以本轮声明映射检查替代正式 typecheck；不得将 M-09 标已完成。

本轮提交按用户要求在当前分支尝试，并对该次提交禁用 hooks，避免 post-commit 偷跑全量门禁；若权限拒绝则提交由 Autome 兜底。
