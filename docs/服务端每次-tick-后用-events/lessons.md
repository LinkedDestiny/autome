# 教训 · 服务端每次-tick-后用-events（W3 事件总线）

本次运行：设计 1 轮；实现 25 轮（状态块 `implementation-round: 25/30`，Autome 指标记为 20/20，见 L-07）；4 个里程碑全部一次审计通过，reopen 0，实现缺陷 0，人工验收 H-01..H-03 未确认。M-01 一个里程碑用掉实现 #1–#22，M-02..M-04 各一轮。M-01 的 22 轮里，实现 #2–#18 卡在宿主门禁缺 `ws` 包（L-01、L-02），实现 #19–#22 卡在沙箱监听 EPERM 和两处只有监听才跑得到的测试错误（L-03、L-04、L-06）。

L-04、L-05、L-06、L-07 与前几个任务的条目同义，proposal 逐字沿用，便于累计次数。L-05 的后半句（「在同一轮改写契约」）与 `apps-web-的服务端托管-automed-2` L-01（实现轮不得改验收契约）冲突；本次实际做法是审计 #1 更正契约文字。另：Autome 指标「验证缺口 0」，但审计 #1（`evidence/M-01-r22-audit.md`）与审计 #3（`evidence/M-03-r24-audit.md`）都在证据里写明发现了验证缺口并当场补强。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #2–#18 共 17 轮都是门禁修复轮，每轮都以同一签名结束：宿主门禁 Web typecheck 报 TS2307 找不到 ws，Web test 3 个文件导入失败（188 个用例只收集到 164 个）。从实现 #2 起 package.json 与锁文件已声明 ws 8.22.2、@types/ws 8.18.2，但 apps/web/node_modules 里一直没有这两个包；实现轮沙箱里在线安装报 ENOTFOUND registry.npmjs.org，离线安装报 ENOTCACHED。实现 #19 发现依赖已经装好（原因未核实），门禁这才越过这一步'
  root_cause: 'scripts/ci-local.sh 只在 apps/web/node_modules 目录不存在时才执行 npm ci（第 108 行；desktop 段第 93 行同理）。任务新增依赖后，宿主门禁沿用 worktree 里已有的 node_modules，永远不会去装新依赖；实现轮沙箱访问不了 registry，也装不上。两边都不会装，于是每轮门禁失败签名都一样'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-01-r9-impl.md'
  level: test
  proposal: 'scripts/ci-local.sh 的 Web install 条件改为「node_modules 不存在，或 apps/web/package-lock.json 比 apps/web/node_modules/.package-lock.json 新」，并由 apps/web/test/ci-config.test.ts 的一条用例断言这个条件存在'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: process
  symptom: '实现 #2–#18 的证据逐轮写「环境阻塞，不是协议失败」；实现 #9 写「下一轮需先恢复锁定依赖的安装条件」，实现 #10 写「环境未改变，未重复同一门禁」。但 17 轮里设计文档 `## 争议项` 始终为空，没有一轮写出需要人执行的那条命令。实现预算在 M-01 上用到 20/20（实现 #20），之后又扩到 30'
  root_cause: '门禁失败后，Autome 每次重跑都得到同一签名，于是再派一轮修复；修复轮把阻塞写进证据就结束，而证据不是用户会去看的地方。这与「修复已停止或已暂停的任务恢复时从任务整理重新开始」L-01（Electron 下载被沙箱 DNS 挡住，三轮不改代码）是同一形态，但那条提案只覆盖「失败在任务不许改的目录之外」；本次缺的包在允许改动的 apps/web 里，那条规则不会触发'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-01-r10-impl.md'
  level: protocol
  proposal: '门禁修复轮没有改动任何代码就结束、且失败签名与上一轮证据记录的相同时，必须在设计文档 `## 争议项` 新增一条，写明失败签名和需要人在宿主上执行的具体命令及其工作目录'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: '宿主门禁报 eventbus-live 第 78 行的 R-02 跨进程 cancel 用例等满 3500 ms 超时。实现 #22 查明：测试谓词按 `payload.state === "cancelled"` 判断，而真实 events.since 返回的是 `{"state":{"state":"cancelled"},"trigger":"cancel"}`，谓词永远不匹配。测试照抄的是设计背景节「实测」段和 §6.2 写的扁平载荷 `{state:"cancelled", trigger:"cancel"}`'
  root_cause: '设计轮确实跑过真实命令行（输出在 .autome/output/w3-cli-exp*.txt，不入库），但写进设计时把载荷改写成了简写，丢掉了 TaskStateRepr 的嵌套标签。原始输出不在仓库里，后面各轮只能相信这段转述。实现 #22 只用不需要监听端口的真实 CLI 加 `autome rpc events.since` 就复现了问题，这个实验在实现 #1 就能做'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-01-r22-impl.md'
  level: rule
  proposal: '设计文档引用实测得到的数据结构（RPC 返回值、事件 payload）时，必须逐字粘贴原始 JSON 输出片段，不得改写成简写或伪 JSON'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: tooling
  symptom: '实现 #19–#25 的 Web 全量都被 `listen EPERM 127.0.0.1`（伴随 watcher EMFILE）阻断：M-01 时 11 个失败、14 个跳过；M-02 起有 27 个监听用例未执行（gateway-http 11、server 9、eventbus-live 7）。审计 #1–#4 在另一环境跑同样的命令全部通过，0 跳过。实现 #20、#22 修的两处测试错误（/ws 注册后旧 404 用例的前提失效、cancel 载荷层级不对）都只有监听才跑得到，实现轮写完一次都没跑过，都是宿主门禁发现的'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮和宿主门禁允许。这是继 W1（在-apps-web-建-react L-01）、W2（apps-web-的服务端托管-automed-2 L-02）之后第三个出现同一问题的任务。任务文件的「前车之鉴」只能让实现轮把阻断写清楚，不能让它跑到这些断言'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-02-r23-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: verification
  symptom: 'M-01 验收契约写：/ws 改用 server.on("upgrade") 自行处理时，eventbus-live ② 与 security.test 会失败。审计 #1 实测注入后这两条都没失败（19 个全过），因为 security.test 用的是 createHttpServer 而不是 startServer，守卫监听器仍会先拒绝外站。审计当场在 gateway-http 补了 listenerCount 断言'
  root_cause: '契约里「变异 → 哪条测试变红」是设计轮推理出来的。实现 #1 只跑了分页 break 这一条变异，去掉 onTick 和升级旁路两条写明「尚未执行」；之后 21 个实现轮的证据里都没有这两条的退出码，偏差直到审计才暴露'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-01-r22-audit.md'
  level: rule
  proposal: '实现轮对验收契约中的每条负向对照，必须对契约点名的那条测试实跑该变异并在证据里写出它的退出码；点名的测试没有变红时，在同一轮改写契约里的这条表述'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-06
  domain: process
  symptom: '实现 #20 证据写「用户粘贴的宿主日志只展示第二个失败位置，未展示第一个失败的名称和断言」，只修掉了一处（gateway-http 404 的前提），另一处（cancel 载荷）到实现 #22 才定位。实现 #19、#20、#22 各自先在本地原样跑一遍 ci-local（每次 149–164 秒）找失败，可本地被 EPERM 抢先阻断，到不了宿主失败的那条断言'
  root_cause: '宿主门禁的失败只通过用户在 prompt 里粘贴的片段传给修复轮，简报 brief/impl-19.md 到 impl-22.md 里没有失败用例名和断言原文；实现轮沙箱又跑不到监听用例，本地重跑替代不了宿主日志'
  evidence: 'docs/服务端每次-tick-后用-events/evidence/M-01-r20-impl.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-07
  domain: tooling
  symptom: '本次复盘收到的指标是「实现轮 20/20」，而设计文档状态块终值是 `implementation-round: 25/30`，retro.md 记有实现 #1–#25，简报 brief/impl-21.md 到 impl-25.md 都存在。前两个任务（在-apps-web-建-react L-05、修复已停止或已暂停的任务恢复时从任务整理重新开始 L-04）也记过轮次指标与状态块不符'
  root_cause: '指标里的 20/20 正好等于扩容前的预算上限（实现 #20 证据写 20/20，实现 #21 起为 /30），说明 Autome 的轮次指标没有取任务分支上状态块的最终分子；具体取自哪个快照未核实'
  evidence: 'docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md'
  level: test
  proposal: 'Autome 增加一条测试：任务结束后，指标中的 design_rounds_used 与 impl_rounds_used 分别等于任务分支上设计文档状态块 `design-round`、`implementation-round` 的分子'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
