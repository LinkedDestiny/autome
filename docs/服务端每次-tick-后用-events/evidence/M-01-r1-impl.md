# M-01 · 实现 #1 · 保持开放

本轮实现事件消息类型与状态映射、单飞分页总线、连接去重与订阅、关闭处理、ws 升级适配和 main 接线；新增 8 个单元用例、5 个真实服务用例及 pidfile 包装脚本。CI web job 构建命令增加 autome-cli，ci-config 的对应断言同步修改，用例数不变。未推进 M-02–M-04，没有上一轮审计需搬入的回归检查。

## 阻塞与后续

安装 `ws@^8.22.2` 失败：registry.npmjs.org DNS 解析 ENOTFOUND；离线安装同版本以及 `@types/ws@^8.18.2` 均 ENOTCACHED。没有伪造包完整性或修改依赖版本。package.json/package-lock.json 尚未更新，新增模块暂时不能加载。下轮先完成这两个依赖的安装并提交锁文件，再运行完整验收、修复真实集成问题。不得把本轮算作端到端通过或待审。

原始输出：`.autome/output/m1-install.txt`、`m1-offline.txt`、`m1-types.txt`。未修改 .autome 配置、协议或规则，仅按任务要求在 output 保存日志。

## 验收与用例基线

各验收分别运行，没有用前一个失败阻止后一个执行。

| 命令 | 退出码 | 结果 / 原始输出 |
|---|---|---|
| npm run lint --prefix apps/web | 0 | m1-lint.txt |
| npm run typecheck --prefix apps/web | 2 | ws 模块/类型缺失及由此导致的回调隐式 any；m1-typecheck.txt |
| npm test --prefix apps/web | 1 | 18 文件中 15 通过、3 收集失败；164 用例通过；m1-web.txt |
| npm run build --prefix apps/web | 0 | m1-build.txt；此命令不验证自定义服务端 ws 模块可运行 |
| npm test --prefix apps/desktop | 0 | 104 总数、103 通过、1 基线跳过；m1-desktop.txt |
| git diff 30e5d51 --stat -- crates apps/desktop protocol plugins | 0 | m1-scope.txt 为 0 字节，无越界改动 |
| cd apps/web && npx vitest run test/eventbus.test.ts | 0 | 8/8 通过、0 跳过；m1-unit.txt |

日志路径均在 `.autome/output/`。Web 设计基线 175，本轮实际收集 164，差 **-11**：新增单元 8，原有 server/gateway-http 共 19 因导入 ws 失败未收集（175 + 8 - 19 = 164）。新增 live 的 5 个也未收集，因此与目标 188 相差 24；不是“0 跳过”即可视为全覆盖。Desktop 总数 104，差 0，保留原有 1 跳过。

## 标待审之前的自审清单（本轮未标待审）

1. **情形表逐行**：M-01 八项均由 eventbus.test.ts 实跑：初始化不推历史；2500 条三页；单飞合并补拉；失败保留游标；重复页去重；hello 和新连接过滤/断开；订阅退订；关闭和在途返回丢弃。M-01 五项真实服务检查已编写但因依赖缺失未运行，均未取证。§3 resume 情形表属于 M-02，本轮不适用。
2. **失败分支**：读盘失败用拒绝 Promise 实跑并验证中文日志和原游标重试；空页实跑。Origin 拒绝检查未运行；真实内核不可达、超时与恢复的协议行为属于 M-02，本轮不适用。依赖外部 registry 不可达实测，记录为环境阻塞。
3. **全部迁移**：未初始化→已初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（包含请求在途）均由八项单元用例实跑。内核五态仅实现映射供 hello 使用，五态广播及续传状态迁移归 M-02；真实升级及 SIGTERM 迁移未取证。
4. **输入域边界**：游标 0、Number.MAX_SAFE_INTEGER 初始化请求、空页、单事件、1000 条分页与2500条跨页、重复序号均实跑。浏览器 resume 输入校验属于 M-02，本轮不适用；真实 WebSocket 帧边界交 ws 处理，本轮尚未取证。

## 负向对照与还原

将分页循环终止条件临时替换为无条件 break，运行 `cd apps/web && npx vitest run test/eventbus.test.ts -t '2500'`，退出码 1（m1-negative-page.txt）：只广播1000条，2500条断言失败。恢复源文件后单元全量8条通过、退出码0。`main.ts` 去掉 onTick、升级守卫旁路的两项负向对照尚未执行，原因同真实服务测试缺 ws。未留下缺陷注入；git diff --check 通过。工作区此时包含本轮正常新增/修改，不能宣称为空。
