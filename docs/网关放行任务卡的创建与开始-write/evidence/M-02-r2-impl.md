# M-02 实现 #2

结论：实现已落地；真实 HTTP 验收受沙箱监听权限阻塞，M-02 保持开放，不标待审。implementation-round 为 2/20。上一轮 M-01 审计通过，无被退回缺陷，故没有需逐字迁入及红绿复现的审计检查。

## 实现与范围

- read-gate 放行 project.probe、config.validate；冻结有意不放行清单 events.since、env.detect，并注释理由。
- 只有 project.probe.path 允许 `~` 或 `~/` 家目录拼写，拒绝 `..` 路径段、反斜杠和空字符；其它方法和键维持既有路径拒绝。载荷仍限 4096 字符，零依赖。
- 解析真实 dispatch.rs 的 READ_METHODS，双向检查及例外归属、不相交检查；真实源码声明数量独立验证。注入 task.fake_read 后差集精确为该方法，断言空差集会抛错；声明数量不符也会抛错。
- 新增真实内核 HTTP probe 用例，在启动前隔离 HOME，清理时先关闭服务再恢复 HOME、删除临时目录。未开始任何真实 Agent 任务。
- 按 D-7 改写最小参数、内部方法清单、中文错误守卫，未删除任何测试。不改 crates、desktop、gateway 产品实现或后续里程碑。

## 命令与结果

- M-02 原验收命令：`cargo build -p automed && (cd apps/web && npm ci) && (cd apps/web && npx vitest run test/read-gate.test.ts test/write-gate.test.ts test/gateway.test.ts test/gateway-task-card.test.ts) && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web`。
- 原始输出：`.autome/output/m02-acceptance.txt`。cargo build、npm ci 成功；定向共 74，71 通过、3 失败。3 个失败均在 gateway-task-card.test.ts 的服务监听阶段抛 `listen EPERM: operation not permitted 127.0.0.1`，尚未执行 HTTP 断言。其余 read-gate 15、write-gate 43、gateway 13 全通过。
- 因串联命令停止，分别继续执行 `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`，通过，无 lint 警告。原始输出分别为 `.autome/output/m02-lint.txt`、`.autome/output/m02-typecheck.txt`。
- `git diff --check` 通过。

## 标待审之前的自审清单

1. **情形表逐行**：D-3 的 project.probe 合法家目录路径门禁通过，替身确认原样转发；真实 HTTP 展开检查已执行但被 EPERM 阻塞，未取证。config.validate 最小参数通过，路径形状 project_id 被拒。events.since、env.detect 逐项拒绝，冻结清单及与内核的归属关系通过。D-2 仅方法加键例外、零依赖、载荷上限均受现有或新增检查覆盖。
2. **失败分支**：缺 path、空串、数字/null/布尔/数组/对象、绝对路径、相对路径、~user、穿越、反斜杠、空字符均拒绝并有中文原因；额外路径键和其它方法使用家目录值均拒绝；超过载荷限制拒绝。gateway 现有离线 503、超时 504、内核错误 409 检查通过。拒绝授权不适用（本里程碑不改变授权）。读盘失败不适用（门禁不读盘，内核 probe 既有实现只读）；真实内核路径存在结果尚因监听权限未取证。
3. **全部迁移**：不适用状态机（probe/config.validate 无状态迁移）。通道检查：读通道新增两方法成功过门禁，写通道仍不可达（write-gate 全量 op 及内核读方法交叉检查通过）；project.add 两通道拒绝；被保留的内部读方法仍不对浏览器开放，env.detect 仅保留既有写入口。未推进 M-04 生命周期。
4. **域边界**：`~`、`~/`、中文空格目录、`a..b` 均通过；空串及所有非法形状拒绝；JSON 长度 4096 通过、4097 拒绝；新增内核方法差集能被检测，真实声明 N 与解析项数一致、篡改 N 时失败。日期、闰日、跨年不适用（无日期输入域）。

完整验收尚未通过，以上自审不作为标待审的依据；下轮应在允许本地监听的环境复验，无需为此改动产品行为或弱化测试。

## 全量结果与基线差

`npm test --prefix apps/web` 已完整执行，退出 1；34 个文件中 29 通过、5 失败；318 个用例中 285 通过、14 失败、19 跳过。原始输出 `.autome/output/m02-full.txt`。环境错误包括 HTTP 监听 EPERM 与 File watcher EMFILE（导致提前退出或等待超时）；19 个跳过照实保留，未新增条件跳过。不能以单元通过替代全量通过或真实内核 HTTP 验收。

上一轮审计实测全量 312 = 初始 B(301)+11；本轮 318 = B+17，比上一轮增加 6：read-gate 11→15（+4），gateway 12→13（+1），gateway-task-card 2→3（+1），write-gate 保持 43。指定四文件从 68 增至 74，差同为 6，未删减用例。
