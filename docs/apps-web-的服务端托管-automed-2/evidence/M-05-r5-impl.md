# M-05 · 实现 #5

结论：网关实现完成，提请待审；不宣称 HTTP 端到端全绿。依据设计「风险与验证安排」第 2 条，19 个监听用例受沙箱 EPERM 阻塞，交审计轮在可监听环境复验。上一轮 M-04 审计通过，无退回缺陷，无需搬入审计缺陷回归检查。

## 改动与修正

- 新增 gateway：独立调用读写门禁；core.restart 本地处理；六个本地 op 未注册 501、注册后交处理器；附件编号解析为 uploads 子目录内绝对路径，拒绝目录本身、兄弟前缀、越界和相对路径；删除 task.create 的 doc_refs；暴露 core 供内部调用。
- HTTP 在安全中间件之后、Vite/RR 之前装 1mb JSON parser 和两条 POST 路由；解析失败回中文 400，网关保留 400/409/501/503/504 与内核原话/code。
- startServer 监听成功后启动 supervisor 和 3 秒 ticker，返回 gateway 与 upgrade 注册入口；close 依次停止 ticker、内核和 HTTP。home 与内核环境解析一致（包括空串），binary 使用既有四级解析。
- W1 启动 helper 使用临时 AUTOME_HOME 和显式 AUTOMED_BIN；直接 startServer 使用临时 home。源码扫描扩展到 startServer 与 npm spawn，并覆盖局部 options 写法。
- 首轮扫描窗口只看调用后方，误报已有 options 变量传参；修为调用前后局部窗口后通过。新增真实内核测试最初使用 parallel=7，被内核合法地以 409 拒绝；核对 PARALLEL_MIN/MAX 为 1–5 后改为 4（不同于默认 3）。同时核对 config.get 的 resolved.loop_defaults 响应结构。
- 无依赖变化，不修改内核、desktop、协议、其它 worktree 或远端。

## 命令与结果

验收契约各段分别运行，使环境失败之后仍能完成 build 与 desktop：

- `cargo build -p automed`：退出 0，`.autome/output/m05-cargo.txt`。
- `npm run lint --prefix apps/web`：最终退出 0，`.autome/output/m05-lint-final.txt`。
- `npm run typecheck --prefix apps/web`：最终退出 0，`.autome/output/m05-types-final.txt`。
- `npm test --prefix apps/web`：**175 例**，比上轮审计 **152 增加 23**；gateway 11（设计 9 + 不监听的真实内核生命周期 1 + home 输入边界 1）、gateway-http 10、server 新增 2。**156 通过、10 失败、9 未执行**；10 个 gateway-http 均因 `listen EPERM: operation not permitted 127.0.0.1` 失败，server 的 beforeAll 同样 EPERM 并伴随 EMFILE watcher，导致 9 个未执行。14 文件通过、2 文件环境阻塞。没有新增 skip 或删减用例。日志 `.autome/output/m05-web-final.txt`。
- `npm run build --prefix apps/web`：退出 0，`.autome/output/m05-build.txt`。
- `npm test --prefix apps/desktop`：**104 例，与基线差 0**，103 通过、0 失败、1 个既有 electron dom harness 跳过；`.autome/output/m05-desktop.txt`。
- 负向修改全部还原后 `npm test --prefix apps/web -- test/gateway.test.ts test/core-sidecar.test.ts`：29/29 通过、0 跳过；`.autome/output/m05-targeted-final.txt`。其中真实内核写读、停机、手动重启成功；这不能代替 HTTP 验收。
- 测试前后 `stat -f '%N %m' ~/.autome/state/automed.sqlite3 ~/.autome/web`：数据库 mtime 均为 **1794305868**，web 目录均不存在；before/after 两份输出 `cmp` 完全一致。日志 `.autome/output/m05-home-before.txt`、`m05-home-after.txt`。
- `git diff --check` 通过。未主动运行全仓 ci-local：宿主按配置另行执行，本轮不把它计为已通过。

## 标待审之前的自审清单

1. **情形表逐行**
   - §5 core.restart：假 core 验证 restart 被调用、call 零调用；真实 core 验证在线返回 already_running、停止后返回 restarted，均通过。
   - 六个本地 op：逐个验证未注册 501、已注册调用本地处理器、内核零调用，通过。
   - task.create：合法单个 UUID 解析后仅服务端路径进入 core；空附件可用、空 doc_refs 被删除；默认 resolver/null 和越界全部拒绝，通过。
   - 其余 op：config.set_loop 参数原样转发；非法 project.add 在门禁被拒，通过。读转发与 project.probe/session.log 任意路径拒绝通过。
   - §8 HTTP 状态：gateway 的 200/400/409/501/503/504 分支通过；JSON 400、真实 HTTP 安全与响应头的检查已落入 gateway-http，但本轮因 EPERM 未取证。
2. **失败分支**
   - 门禁拒绝、附件不存在/过期/越界、未支持本地 op、内核离线、内核业务 code、等待超时均在 gateway.test 执行通过。
   - 二进制不可达：既有 supervisor 实际失败进程用例通过；经 HTTP 的 503 中文原因用例因监听失败未取证。
   - Host/Origin/upgrade 拒绝：既有 security 14 例通过；新增 HTTP 与开发 HMR 检查受 EPERM 阻塞。坏 JSON、close 后 pid 消失的 HTTP 用例同样未取证。
   - 读盘失败不适用：网关本身不读盘，uploads 在本轮为纯 resolver；没有新外部网络服务。
3. **全部迁移**
   - 网关无持久状态机。读/写分别覆盖允许→core 和拒绝→零调用；本地未注册→注册处理器覆盖六项；core.restart 在线→already_running、stopped→starting→connected→读回已保存配置均真实执行通过。
   - supervisor 的 starting/connected/restarting/offline/stopped、ticker 忙/闲/停止各迁移由全量既有用例通过；本轮 HTTP listen→启动和 close→停止的组合用例已编写，但因 EPERM 无法实际执行。端口冲突不启动子进程由代码顺序核对，W1 端口占用测试未取证。
4. **输入域边界**
   - null、空请求、未知 method/op、路径参数拒绝；单个附件、空数组/缺省、null resolver、uploads 根目录、同前缀兄弟目录、.. 越界、相对路径均执行通过。
   - home 显式覆盖、空串、仅 HOME、两变量均缺省四项通过。既有门禁最大长度/数组数量/UUID 格式边界与 security Host/Origin 表通过。
   - HTTP 空/坏 JSON 和动态端口 0 的组合检查未取证（EPERM）。日期、闰日与跨年不适用：网关无日期输入域。

## 负向对照

临时修改后均还原：

- 将 core.restart 改为 core.call(op, params)，`npm test --prefix apps/web -- test/gateway.test.ts` 退出 1，本地处理及真实重启测试拒绝错误实现；`.autome/output/m05-negative-restart.txt`。
- 删除 W1 launch 环境的 `AUTOME_HOME: tempHome(), `，`npm test --prefix apps/web -- test/core-sidecar.test.ts` 退出 1，隔离扫描报告 server.test.ts；`.autome/output/m05-negative-home.txt`。没有启动被改坏的 server 测试，真实 home 不受影响。

## 交接

审计轮须在可监听环境运行完整验收契约，重点复验 19 个环境阻塞用例及真实 home 前后 stat；人工 H-02/H-03/H-04 仍由用户合并前确认。本轮只推进 M-05，不关闭里程碑、不启动下一会话。
