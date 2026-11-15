# 实现 #28：Web e2e 总预算修复

本轮为宿主集成门禁修复，沿用 M-13 记证据；不推进新里程碑，里程碑表及既有审计状态保持不变。implementation-round: 28/65。

## 失败定位与修复

- 宿主提交 de270404b6：Web e2e 共 29 例，20 通过、9 未执行、2 个非用例错误。错误是 `Timed out waiting 360s for the test suite to run` 及 teardown 总超时；没有证据表明最后通过的布局用例失败，也不能把未执行的 9 例判失败。
- 配置 `globalTimeout: 360_000` 涵盖两个临时服务初始化、串行用例及清理。设计 R-1 对新增用例耗时的原估计不足，宿主实测已反证 360 s 足够这一前提。将其改为 720_000；单例 90 s、expect 10 s、workers 1、retries 0 与全部产品断言不变。
- 预算规划按 29 × 20 s + 120 s 初始化/清理余量 = 700 s，取整为 720 s。此为容量规划值，不是实测通过时长；慢用例和初始化可能继续消耗预算，需宿主确认实际余量。CI 15 分钟还包括安装构建，本轮不声称已验证 CI 全程。
- 同类检查：现有浏览器配置只有这一处总预算；后续 live 与 manual-items 共 9 例仍保留，包含生产 3000 ms tick、离线恢复及 C-2 多轮会话。未改 tick、屏障、清理或跳过策略。
- 上一审计 #19 通过，没有待迁入的缺陷判别脚本。宿主失败来自套件超时，原浏览器测试完整保留；新增项目测试体系中的预算回归，验证预算下限、低于 CI 上限、单例上限、串行及零重试。该配置检查不替代浏览器行为验收。

## 命令、结果与用例数

1. 修前单独运行 `npm run e2e --prefix apps/web -- e2e/layout.spec.ts`，退出 1；初始化 `spawnSync ps EPERM`，12 条布局用例均未执行。日志 `.autome/output/r28-e2e-before.txt`。这是环境阻断，不是重现宿主的 360 s 超时；使用宿主真实失败和当前配置作因果证据。
2. 加预算回归但未改配置：`cd apps/web && npx vitest run test/ci-config.test.ts`，13 例，12 通过、1 失败，旧 360000 小于所需 700000。日志 `.autome/output/r28-budget-red.txt`。
3. 改配置后：`cd apps/web && npx vitest run test/ci-config.test.ts test/e2e-preflight.test.ts test/test-groups.test.ts`，20 例全部通过，0 跳过。日志 `.autome/output/r28-budget-green.txt`。同范围原基线 19（ci-config 12、preflight 3、分组 4），净增 1 个预算回归；本轮为 13 + 3 + 4。上一轮的 12 例属于 fixture 与分组守卫，范围不同，不作数量等价比较。宿主浏览器基线 29，本轮仍 29，差 0。
4. 修后单跑原未执行子集：`npm run e2e --prefix apps/web -- e2e/live.spec.ts e2e/manual-items.spec.ts`，退出 1；仍被初始化 ps EPERM 阻断，9 例均未执行，不能报绿。日志 `.autome/output/r28-e2e-after.txt`。
5. `cd apps/web && npx eslint playwright.config.ts test/ci-config.test.ts --max-warnings 0` 通过；`git diff --check` 通过。未执行全量门禁和宿主快检。实际浏览器、快检及合并门禁由 Autome 复验。

## 四项自审清单

1. **情形表逐行**：串行执行、零重试、单例 90 s、总预算覆盖规划值且低于 CI 上限，由配置回归逐项通过；原 29 条行为用例没有删除或跳过。R-1 原耗时估计已就地修订。
2. **失败分支**：旧 360 s 配置红测已执行；修后预算检查绿。进程枚举拒绝在修前/修后真实触发且继续报错，没有绕过安全检查。产品授权、读盘、空输入、外部服务失败不适用，本轮未改产品路径；真实全局超时及 teardown 能否顺利完成仍待宿主。
3. **全部迁移**：不适用，本轮仅更改测试套件预算，没有业务状态机或通道变更；初始化/运行/清理逻辑保持原样，浏览器初始化阻断后清理同样报告 ps EPERM。
4. **输入域边界**：预算下限 700 s 与 CI 900 s 上限由配置回归检查，720 s 落在区间内；单例上限不变。日期、空串、业务最大输入不适用，没有新增输入域。

## 环境与交付

两次浏览器尝试在启动服务前的进程枚举被拒，没有启动面板、内核或浏览器；只使用夹具临时目录，没有接触真实账本。配置回归已取得红绿证据；720 s 的全套运行是否通过尚未取得证据。
