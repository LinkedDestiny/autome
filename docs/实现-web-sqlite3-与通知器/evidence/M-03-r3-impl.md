# M-03 · 实现 #3

结论：实现已交付，标待审；监听相关验收受沙箱阻断，按 M-03 契约交审计复验，不宣称全量通过。

## 实现与审计衔接

新增通知列表/已读处理函数与 attachNotifier；main 打开面板库、连接通知器，tick 在 bus.pull 完成后发起通知补拉，connected 立即补拉。关闭按 ticker、notifier、bus、core、store、HTTP 的顺序执行；启动失败释放面板库。HTTP 通知路由复用原 JSON 解析并置于 securityMiddleware 之后。

上一轮审计 M-02 通过，无实现缺陷及须逐字搬入的判别检查，因此无既有审计缺陷的红绿迁移。本轮未改已有测试；新增 API 7 项、live 3 项。live 使用临时 home、真实内核及仅退出 3 的假 CLI，通过内核包装脚本和子进程 env 注入，不修改 process.env、不调用真实 Agent。

## 命令与结果

原始输出均在 `.autome/output/`，没有复制进任务目录。命令各自执行，以下为命令自身退出码（不是 tail 退出码）。

| 命令 | 退出码 | 结果 / 原始输出 |
|---|---|---|
| `npm test --prefix apps/web -- test/notifier-api.test.ts` | 0 | 7/7，通过；m03-api.txt |
| `npm test --prefix apps/web -- test/notifier-live.test.ts` | 1 | beforeAll 的 listen EPERM，3 项未执行；m03-live.txt |
| `npm test --prefix apps/web` | 1 | 29 文件、277 项：247 通过、11 失败、19 未执行；m03-full.txt |
| `npm run lint --prefix apps/web` | 0 | 通过；m03-lint.txt |
| `npm run typecheck --prefix apps/web` | 0 | 通过；m03-types.txt |
| `npm test --prefix apps/desktop` | 0 | 104 项，103 通过、1 基线跳过；m03-desktop.txt |
| `git diff --check` | 0 | 无空白问题 |

用例总数与上一轮审计基线：Web 267 → 277（+10，新增上述 7+3 项，无删除）；desktop 104 → 104（差 0）。Web 全量 4 个失败文件为 gateway-http、eventbus-live、server、notifier-live；11 个失败用例均报 listen EPERM；另 19 项因套件初始化失败未执行，包含新 live 3 项与原监听套件 16 项。日志亦有 Vite EMFILE，不能把这些未执行项解释为产品通过。非监听的 25 个文件通过。最终 live fixture 隔离调整后另行重跑 live，仍为 EPERM。

## 负向对照

每次只注入一个变化，finally 恢复原文件，没有保留故障代码。

- 把 `void bus.pull().then(() => notifier.pull());` 改成 `void bus.pull(); void notifier.pull();`：API 验收退出 1，顺序用例失败；输出 m03-negative-order.txt。
- 把已读广播语句替换为 `void publish;`：API 验收退出 1，单条与全部已读广播断言失败；输出 m03-negative-broadcast.txt。
- 在 securityMiddleware 之前注册已读 POST：live 验收退出 1，但失败在监听 EPERM，未到达 403 断言，**此负向对照未取证**；输出 m03-negative-security.txt。审计须在可监听环境补跑。
- 还原后 API 7/7，退出 0；live 仍被 EPERM 阻断。故障注入已全部还原；本轮待提交的实现及文档改动仍在工作区，不能将其描述为 clean。

## 标待审前自审清单

1. **情形表逐行**：§5 tick 同步返回、event 在 notification 前、通知挂起不阻塞下一次 bus.pull，API 第 6 项通过；connected 补拉与其它状态仅转发，API 第 7 项通过。§6 列表的九个字段、level、未读总数、缺省 50 与 1..200，API 第 1 项通过；单条 / 全部已读落库并广播实际 id，API 第 2/3 项通过；重复已读 / 未知 id 无变化不广播，第 4 项通过；非法体拒绝且库不变，第 5 项通过。双连接广播及重启补发已有 live 三项，因 EPERM 未取证。关闭顺序按源码核对，真实服务关闭未取证。
2. **失败分支**：非法 limit、非法 ids/all 均实跑拒绝且不写库；授权拒绝的无 Origin POST 已写 live 断言但受监听阻断。读盘/迁移失败：接线直接传播 openWebStore 异常，M-01 库测试在全量中通过；超时/内核离线与补齐拒绝属 M-02 通知器，现有 notifier 测试在全量中通过。没有新的外部服务调用。空输入在 API 第 1/5 项实跑。启动 listen 失败本轮实际触发，返回 EPERM 并关闭库及中间件。
3. **全部迁移**：starting、offline、restarting、connected、stopped 均逐个传入，只有 connected 补拉；未读→已读（单条/全部）、已读→已读、未知 id→无变化均实跑。tick 的 bus 在途→完成→通知在途→后续 tick 实跑。跨通道 notification 与 notification-read 双客户端、停服→新增账本事件→重启的测试已实现，但未取证（监听限制）。
4. **输入域边界**：limit 缺省、1、01、200 通过；空串、0、负数、201、小数、指数、空白、数组、对象、超安全整数拒绝。ids 单元素、重复、200 元素、64 字通过；空数组、201 元素、65 字、非字符串、ids/all 同给、额外字段拒绝且库不变。空字符串 id 属设计允许的“不超过 64 字字符串”，作为未知 id 成功无改动。闰日/跨年不适用：接口没有日期输入。所有结果包含于 7 项 API 测试。

## 待审事项

在允许 localhost 监听的环境运行 live 与全量四件，完成安全路由前移的负向对照；当前环境不支持提权，未绕过限制。人工浏览器验收属于后续里程碑，不在本轮声称完成。
