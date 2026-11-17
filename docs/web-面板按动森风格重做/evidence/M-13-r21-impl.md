# 修复轮 · 实现 21/65

## 范围与根因

本轮只修宿主在 597d0f52672614e77ac2a983324aed9468b81a2c 上报告的门禁回归，不推进新里程碑。关联 M-03 的总览运行依赖和 M-08 的设置路由迁移（总-Q5、E-Q6/E-7、验-8/验-9）。全部里程碑保持审计关闭时的状态，已关闭里程碑的证据指针不改。

- 发布包：`stage-panel.ts` 只复制 `app/load-context.ts`，原生 Node 运行的 `server/overview-summary.ts` 却引用 `app/lib/waiting.ts`。同类检查覆盖 server 下全部跨 app 导入，另发现 `overview.ts` 依赖 `overview-model.ts`、`task-columns.ts`。暂存清单补齐这三个纯 TS 模块，类型导入不需要运行时文件。不复制整个前端源目录。
- 设置 HTTP：设计明确旧 `/projects/:id/settings` 只作 GET 302 跳转，实际表单 action 在 `/loop?project=<id>&tab=project`。原测试仍 POST 旧址，405 是该路由没有 action 的结果。迁移测试的普通与 `.data` 地址，保留 200、拒绝原文、字符上限、账本不变断言；坏 Host/Origin 同时覆盖新旧地址，移除项目用例也补查新址。旧址 GET 浏览器跳转用例继续保留。
- `settings-onboarding.test.ts` 改为经过真实 `/loop` action 分派，不再直接调用导出的内部 settingsAction；辅助函数拒绝误入角色保存结果。
- 上轮审计通过，无审计退回的临时检查可搬。宿主两条检查本来就在项目测试体系，保留其名称和安全断言。发布包既有用例新增原生 Node 导入暂存产物的断言，使缺模块可在不监听端口时复现。

## 复现、修复与验证

1. 原样运行 `npm run test --prefix apps/web -- test/install-alignment.test.ts test/w9-http.test.ts`：15 例，4 通过、11 失败、0 跳过。日志 `.autome/output/r21-red.log`。其中安装启动/清理被 `spawnSync ps EPERM` 阻断，9 条 HTTP 被 `listen EPERM 127.0.0.1` 阻断，未能在本沙箱复现宿主的启动/405 断言。
2. 修复前给既有发布包用例加原生 Node 导入检查，运行 `npm run test --prefix apps/web -- test/install-alignment.test.ts -t '发布包带运行面板'`：1 失败、5 因名称筛选跳过，明确报暂存产物缺 `app/lib/overview-model.ts`。日志 `.autome/output/r21-package-red.log`。这是同一遗漏清单根因的同类缺失文件，宿主首先报 waiting.ts。
3. 补齐暂存清单后原命令：1 通过、5 名称筛选跳过，原生 Node 成功加载 overview.ts 及其递归依赖 overview-summary.ts、waiting.ts 等。日志 `.autome/output/r21-package-green.log`。
4. 相关子集 `npm run test --prefix apps/web -- test/stage-panel.test.ts test/settings-onboarding.test.ts test/settings-config.test.ts test/loop-defaults.test.ts test/overview-summary.test.ts`：5 文件 47/47 通过，无跳过，日志 `.autome/output/r21-related.log`。
5. 修复后重跑第 1 项：仍为 15 例、4 通过、11 环境失败、0 跳过，发布包新增断言通过；安装与 HTTP 原验收没有取得绿证据。日志 `.autome/output/r21-recheck.log`。不得把环境失败写成通过。

### 用例总数与基线差

本轮显式子集共 7 个文件、62 个独立用例（15 + 47），修复后 51 通过、11 环境失败，0 跳过；发布包筛选命令是其中一例的重复执行，不另累计。与起点同七文件结构基线 62 相比差 0：未增删 test，只加强既有包导入、真实 action 分派及新旧 URL 断言。上一轮 M-13 契约是四文件 54 例，本轮 62 比其多 8，来自验证范围换为发布/设置修复，不能解释成新增 8 条测试。宿主全量基线 1352 不与本轮子集作增减比较，本轮没有执行全量。

## 四项自审

1. **情形表逐行**：发布正常暂存→原生 Node 可导入、开发依赖排除、校验和正确；缺输出/缺构建/已存在输出/源码内输出→stage-panel 三例拒绝检查通过。设置普通与 single fetch 两通道已指向新地址，旧址安全头检查保留；HTTP 实跑被监听权限阻塞。新路由真实 action 的保存、完成、跳过和失败透传由 onboarding 子集验证通过。
2. **失败分支**：拒绝授权、读盘失败、超时、内核未连接、技能拒绝在 onboarding/defaults 子集通过；超长保存由真实网关拒绝且无 core 调用通过；缺运行模块检查红→绿。Host/Origin 与实际安装启动失败分支已执行，但被环境阻断，交宿主复验；不声称成功。
3. **全部迁移**：本轮不改产品状态机，故新状态迁移不适用。已有 onboarding 第 3/4/5 步完成、失败停止、会话运行/结束等迁移测试通过；旧址 GET 跳转产品逻辑不变，POST 检查迁到实际 action，普通/.data 两通道保留。安装/重复安装/停止链路本沙箱未通过。
4. **输入域边界**：空内容、单字符、262144 字符合法、262145 字符网关拒绝，两种合法文档路径与空/父级/未知路径均由 onboarding 子集通过；默认值上下限、空串、前导零等原边界通过。暂存输出路径边界通过；实际安装 Node 版本边界用例通过。日期域未改，overview-summary 原日界与等待统计测试通过。

## 快检与交付

- 原样运行 `sh scripts/ci-quick.sh --since 597d0f52672614e77ac2a983324aed9468b81a2c`。首次 typecheck 发现测试改用真实 action 后返回类型扩大，已补角色结果排除与 OnboardingResult 类型确认；重跑同一快检，日志 `.autome/output/r21-quick-final.log`。
- 最终快检 11 步，9 成功、2 失败。Format、Clippy、Web lint、Web typecheck、Build core 和 onboarding 单元子集通过；失败仅 install-alignment（2 例 ps EPERM）与 w9-http（9 例 listen EPERM）。快检测试总数 23（8 + 6 + 9），12 通过、11 环境失败，全部是上述 62 例的子集，不能相加。
- 未执行 `sh scripts/ci-local.sh`，未启动下个会话。`git diff --check` 通过。状态轮数为 21/65，next-action 指向宿主复跑；保留现有 status 与全部里程碑状态，不新增关闭记录。
- 宿主后续必须在允许 ps/监听的环境复跑安装启动、普通及 single fetch 设置安全验证；快检与全量门禁最终结论均以宿主提交后/合并前结果为准。
