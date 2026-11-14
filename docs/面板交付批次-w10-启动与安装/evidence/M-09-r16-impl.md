# M-09 · 实现轮 16

结论：修复 W7 H-04 跨标签断言。验收浏览器初始化被 ps EPERM 阻断，未取得全绿证据，M-09 保持开放、reopen 2、domain-review；implementation-round 更新为 16/65。本轮未推进其它里程碑。

## 修复与审计检查归档

审计失败命令 `cd apps/web && npx playwright test e2e/manual-items.spec.ts` 修前原样运行，输出 `.autome/output/r16-red.txt`，实际为 spawnSync ps EPERM，未到断言，不冒称已复现预期红。审计临时检查 `.autome/output/a16-docs-tab.spec.ts` 的三条检查语句（visit 文档标签、oneScreen、文档区 data-scroll-region）逐字搬入项目现有 H-04 用例；原检查本身是审计的通过对照，不是应红用例。时间线与日志继续在默认标签检查，文档改在 `/tasks/T-1?tab=docs` 检查，同样保留一屏判据。没有改产品行为或放宽断言。

检查同类范围：manual-items.spec.ts 与 layout.spec.ts 全部同页区域断言；其它用例没有在时间线标签查找文档区，未发现同类修复点。

## 命令与结果

- 修前命令见上；修后运行契约原命令 `cd apps/web && npx vitest run test/e2e-preflight.test.ts test/core-sidecar.test.ts test/ci-config.test.ts && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts`，退出 1。输出 `.autome/output/r16-contract.txt`。
- Vitest 3 文件 33 条全部通过；Playwright setup/teardown 的进程扫描均遇到 ps EPERM，16 条未进入执行，残留检查未通过。不能记录为修后绿。
- 负向对照 2（删除看板滚动标记）和 3（日志 defaultOpen 重建）依赖浏览器执行；初始化已阻断，未注入，仍为下一轮必做项。本轮不重复无改动的对照 1、4，上一实现轮已记其证据。
- 未运行项目全量门禁与宿主快检；git diff --check 通过。

## 用例总数与基线差

契约总数 49（Vitest 33 + Playwright 16），对上一轮基线 49 差 0：修改现有 H-04 的访问步骤，无新增/删除用例。实际完成 33 条，对上一审计实际执行 49 少 16，原因全为浏览器初始化权限阻断；对上一实现轮实际完成 33 差 0。没有将未运行用例计为通过或成功跳过。

## 标待审前四项自审清单

1. **情形表逐行**：D6 总览、命令面板、铃铛、项目、全部已完成、新建任务、需求日志、Bug 详情、添加项目补全、项目设置、环境共 11 行均核对代码保留；各行动态检查均被初始化阻断。D7 日志保持、详情内部滚动、三页一屏、路径补全、剪贴板共 5 行均保留，详情文档检查已搬到正确标签；各行动态检查均未执行。D1 预检、D2 指纹、D8 CI 与守卫由 33 条 Vitest 通过。
2. **失败分支**：缺浏览器（含 shell）、缺构建、缺内核的预检通过；进程扫描权限拒绝真实触发并使验收失败。超时与进程清理缺动态证据。业务授权拒绝、业务读盘失败、外部服务不可达不适用本轮标签断言修复，未改这些路径。
3. **全部迁移**：时间线→文档标签新增检查已纳入；运行→暂停→重验保留日志、输入→补全→点选、点击复制→读取剪贴板均保留，因初始化权限全部未验证。无生产状态机改动。
4. **输入域边界**：预检空目录及缺依赖边界由 Vitest 通过；1512×944 一屏、滚动偏差 ≤2 px、~/ 补全、唯一匹配与普通浏览器无桥接仍由原用例限定，但浏览器未执行。日期、闰日、跨年、最大整数不适用本轮页面标签检查。

## 提交

尝试仅提交本轮四个文件，提交时禁用 hooks，避免 post-commit 自动启动本轮禁止的全量门禁。若沙箱拒绝提交，提交由 Autome 兜底。
