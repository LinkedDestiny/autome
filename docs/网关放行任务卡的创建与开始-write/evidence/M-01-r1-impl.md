# M-01 实现轮 #1（1/20）

结论：实现及非 HTTP 检查完成，验收未通过，M-01 保持「开放」。沙箱禁止监听 127.0.0.1（`listen EPERM`），不能取得设计要求的真实 HTTP 证据；不属于协议失败，不增加 reopen。本轮无上一轮审计，无需搬入审计回归检查。

## 改动与范围

- `write-gate.ts` 放行 task.start/task.reply，校验 task_id 与 info；info 沿用 8000 字符散文上限，允许空串交给内核判定。
- task.create 新增 kind/draft/brief 形状校验；brief 使用 TextEncoder 计 UTF-8 字节，上限 65536，中文超限消息与内核一致；仅 task.create 增加 524288 字符的 JSON 载荷例外。
- 新增 8 个写门禁、1 个网关转发、2 个真实 HTTP 用例。按 D-7 改写原有载荷/例外/全部 op 正路径断言，没有删除用例。
- 不改 gateway.ts、读网关、Rust、Desktop；M-02..M-04 未推进。
- HTTP 测试不对草稿调用 task.start，所有创建请求均 draft:true；临时服务先关闭，再删除夹具。不存在任务的 start 经真实内核实际返回 transition_rejected（不是 not_found），已据实校正测试期望。

## 命令与结果

所有原始输出在 `.autome/output/`，证据只记录判据。

| 命令/检查 | 结果 | 原始输出 |
|---|---|---|
| `cargo build -p automed` | 通过 | m01-build.txt |
| `npm ci --prefix apps/web` | 通过；有 fsevents 安装脚本提示 | m01-install.txt |
| 改代码前 `npm test --prefix apps/web` | 总数 B=301：268 通过、14 失败、19 跳过；缺 autome CLI 及沙箱监听限制 | m01-baseline.txt |
| `cargo build -p autome-cli` | 通过，补齐既有测试依赖；未改 Rust | m01-cli-build.txt |
| 改代码前再次 `npm test --prefix apps/web` | 仍为 301：271 通过、11 失败、19 跳过；4 个失败文件均有监听 EPERM | m01-baseline-ready.txt |
| M-01 原验收链：`cargo build -p automed && (cd apps/web && npm ci) && (cd apps/web && npx vitest run test/write-gate.test.ts test/gateway.test.ts test/gateway-task-card.test.ts) && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web` | 前两步通过；指定测试 57 例：55 通过、2 失败、0 跳过；HTTP 监听 EPERM，链在测试处停止 | m01-accept-build.txt、m01-accept-install.txt、m01-accept-targeted.txt |
| 单独 `npm run lint --prefix apps/web` | 通过，0 警告 | m01-lint.txt |
| 单独 `npm run typecheck --prefix apps/web` | 通过 | m01-typecheck.txt |
| 单独 `npm test --prefix apps/web` | 总数 312：280 通过、13 失败、19 跳过；5 个失败文件，新增 2 个失败均为 HTTP 监听 EPERM | m01-full.txt |
| 负向对照：临时将 UTF-8 计数换为 value.length，仅跑 brief 边界用例 | 预期红（exit 1），已恢复 | m01-negative-bytes.txt |
| 负向对照：临时移除 task.create 载荷例外，仅跑最大合法组合用例 | 预期红（exit 1），已恢复 | m01-negative-payload.txt |
| 恢复后 `(cd apps/web && npx vitest run test/write-gate.test.ts test/gateway.test.ts)` | 55/55 通过，write-gate 43、gateway 12 | m01-restored.txt |
| `node .autome/output/m01-core-probe.ts` | 网关→sidecar→真实内核补充检查通过，但没有 HTTP 层，不代替 A-01/A-02 | m01-core-probe.txt |
| `git diff --check` | 通过 | 无输出 |

用例数差：本轮是首轮，没有上一轮运行基线，采用改代码前实测 B=301；最终 312，差值 **+11**，来自 write-gate 35→43（+8）、gateway 11→12（+1）、gateway-task-card 0→2（+2）。基线补齐 CLI 后与最终相比，通过 +9、失败 +2、跳过不变。19 个跳过为已有 live 文件的初始化钩子被环境错误阻断；没有新增 skip 或删除测试。HTTP 未执行到业务断言，不能把计数解释为业务失败或验收通过。

补充真实内核观察：60KB 中英正文不带 request，经 createGateway 写入后返回 200、kind=bug、state=draft、标题相符，只证明正文开头到达；task.get 仍为 draft。start 不存在任务返回 409 / transition_rejected /「找不到任务 不存在的任务」；reply 草稿返回 409 / transition_rejected /「当前状态不接受该操作」。这不是成功开始/成功补充证据，成功生命周期留在 M-04。

## 标「待审」之前的自审清单

本轮逐项执行如下；HTTP 缺口未闭合，因此不标待审。

### 1. 情形表逐行

M-01 无独立情形表，以 D-1、D-4..D-8 的逐项落法核对；D-3 读方法表属于 M-02，不适用本轮。

| 设计情形 | 检查与结果 |
|---|---|
| D-1.1 开始/补充白名单 | 全部 op 正路径及新增必填用例通过；HTTP 未取证 |
| D-1.2 info 散文 | 空串、空白、路径样文本、8000 中文通过；8001、空字符拒绝 |
| D-1.3 brief 字节计量与空字符 | ASCII/中文/emoji 边界及空字符用例通过，内核源码常量/消息解析通过 |
| D-1.4、D-4 载荷例外 | 最大转义组合通过；524288 通过、524289 拒绝；其它 op 保持 32768；两项负向对照均红 |
| D-1.5 kind/draft/brief 类型与 task_id/info 必填 | 枚举、null、缺省、错误类型全通过；附件/doc_refs 原有守卫通过 |
| D-1.6 导出、零依赖与中文拒绝 | 常量钉值、两份门禁无 import/require、全部消息中文守卫通过 |
| D-5 info 与 feedback 同上限 | 8000 字符规则通过，不声称内核本身有限长 |
| D-6 语义交给内核 | 空/空白 brief 在门禁通过，真实内核回 409「任务卡正文不能为空」；`# \n` 回 409「任务卡正文缺少标题」 |
| D-7.1..3 旧测试等价改写 | 原载荷守卫转到 config.set_role，并加 task.create 超载荷断言；例外遍历全部 op；补充新增 op 最小参数。35 个原用例保留 |
| D-8 替身转发 | 新字段与 start/reply 原样传给 core.call；非法输入不调用 core.call，12 例通过 |
| D-8 A-01/A-02 真实 HTTP | 两例已写入项目测试，不条件跳过；监听 EPERM，未取得通过证据 |

### 2. 失败分支

- 拒绝授权：沿用既有 Host/Origin 真实 HTTP 用例，本轮全量执行但监听失败，未取证；本轮不改授权逻辑。
- 读盘失败：不适用，新增门禁无读盘行为；测试中的源码读取失败会直接使测试失败，不吞错。
- 超时、外部内核不可达：既有 gateway.test 的「本地等待超时回 504」「离线回 503」通过；HTTP 二进制缺失用例受监听限制未取证。
- 缺参、类型、枚举、字段/载荷超限、空字符：新增与原有门禁用例均通过，中文原因守卫通过。
- 空输入：task_id 空串拒绝；info 空串原样转发；brief 空串/空白在门禁放行并由真实内核以中文拒绝；无标题同样实测 409。
- 内核状态拒绝：补充检查实际跑 start 不存在任务及 reply 草稿，均 409 transition_rejected。HTTP 对应检查未取证。

### 3. 全部迁移

本轮门禁没有新增状态机。M-01 规定的建卡→draft、draft→task.get、start 不存在任务→拒绝、draft+reply→拒绝且保持 draft，均经网关和真实 sidecar 补充检查；要求的 HTTP 通道仍未取证。替身覆盖 start/reply 转发成功以及参数拒绝时不调用内核。draft→开始、await_info→补充的成功迁移明确属于 M-04，本轮不执行，避免启动真实 Agent。读通道的 project.probe 属于 M-02。

### 4. 输入域边界

- brief：0/1/65536/65537 字节；中文 21845+'a' 恰好 65536、再加 1 字节拒；中文 21846（65538 字节）拒；emoji 16384 恰好 65536、再加 'a' 拒；均通过。
- info：0、空白、8000/8001；task_id 缺失/null/空串/错误类型；kind 缺省/null/feature/bug/非法枚举；draft true/false/null/非布尔；brief 非字符串；均通过。
- 载荷：65536 控制字符 brief + 8000 控制字符 request + 512 控制字符 project_id + 256 UUID + kind/draft 通过，524288/524289 两侧通过断言；其它 op 32768/32769、附件 0/1/256/257 既有守卫通过。
- 前导零、闰日、跨年：不适用，没有数字/日期解析域。

## 后续与提交

M-01 保持开放；下一轮在允许本地监听的环境按原验收命令重跑，取得 57 例与全量 312 例通过证据后再报待审。无需改写验收或绕过 HTTP。宿主 `sh scripts/ci-local.sh` 留给 Autome 按约定执行，本轮未宣称通过。
