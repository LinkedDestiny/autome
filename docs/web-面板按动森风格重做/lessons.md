# 教训 · Web 面板按动森风格重做

对照的指标：设计轮 1/15，实现轮 20/65，13 个里程碑，reopen 合计 3（全部在 M-11，领域「总览缺值显示」），实现缺陷 9，验证缺口 5，协议失败 0，人工验收未确认 18，每轮门禁 20 次失败 4。

```yaml
- id: L-01
  domain: design
  symptom: "M-11（README 五张截图）用掉实现轮 13–17 共 5 轮、审计 4 次、reopen 3 次（全任务 reopen 全在这里）才关闭，关闭时 docs/images/dashboard.png 仍是含「undefined / undefined」的旧图，只能移交 H-09"
  root_cause: "设计把「必须由需要 ps/监听端口的 npm run screenshots 生成、再提交进 docs/images 的文件」当作实现轮里程碑的验收条件；实现轮沙箱跑不了该命令，审计轮跑得了却不能写 docs/images，产物只能跨轮复制；缺值代码在实现轮 16 已修好，但图重生成不了，审计 #13 仍按旧图第二次退回，凑满 reopen 3"
  evidence: docs/web-面板按动森风格重做/evidence/M-11-r16-audit.md
  level: rule
  proposal: "凡须由带 ★（需要 ps 或监听端口）的命令生成并提交进仓库的产物，设计轮直接把生成与目视检查列进人工验收清单，里程碑验收命令只检查生成脚本与其断言，不以该产物文件为通过条件"
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: verification
  symptom: "总览卡片「设计轮 undefined / undefined」通过了 M-03 的 72 个用例与 4 项负向对照，直到 M-11 审计 #12 目视真实夹具生成的 dashboard.png 才被发现，算作实现缺陷并触发 reopen"
  root_cause: "overview-labels.ts 对轮次字段没有缺失回退；M-03 的单元夹具总带着这些字段，而截图验收（readme-images）只检查文件集合、PNG 头和 1512×944 尺寸，没有任何检查读页面上实际渲染出来的文字"
  evidence: docs/web-面板按动森风格重做/evidence/M-11-r15-audit.md
  level: test
  proposal: "截图脚本与 e2e 布局用例在每次截图或一屏判定前，断言页面可见文本不含字面量 undefined、NaN、null、[object Object]"
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: implementation
  symptom: "宿主每轮门禁 20 次中失败 4 次；实现轮 11、12、18、19 由快检/宿主门禁抓到的失败里，有四处来自更早里程碑改了契约却没同步的旧断言：board-detail-real 仍期待「门禁未跑过」（M-04 已按 C-3 去掉）、add-project-real 仍走 M-08 前的设置路径、load-context 仍期待 3 次读取（A-Q1 已加 skills.list）、flows D-2 仍期待正序（D-3 已改倒序）；实现轮 12 和 19 整轮只修这些"
  root_cause: "改契约的里程碑只更新了自己验收命令里的测试，没有在 apps/web/test 与 apps/web/e2e 全量检索旧期望；kernel 组和 e2e 用例不在该里程碑验收命令里，快检又按改动文件选测试，旧断言一直潜伏到后面的里程碑碰巧选中才变红"
  evidence: docs/web-面板按动森风格重做/evidence/M-10-r12-impl.md
  level: rule
  proposal: "实现轮删改已有的用户可见文案、回包字段、调用次数或排序契约时，证据必须列出在 apps/web/test 与 apps/web/e2e 检索旧期望的命令和命中数，并写明所有正向期待旧值的命中已在同一轮改完"
  predicted_impact: {metric: round_gate_failures, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: tooling
  symptom: "20 份实现证据里 19 份记录 spawnSync ps EPERM 或 listen EPERM，实现轮没有一次跑通浏览器用例；M-02…M-10 的浏览器验收全靠审计轮补跑，M-11 的截图与 14 条布局巡检最终变成人工项 H-09、H-10"
  root_cause: "实现轮会话的沙箱禁止进程枚举与监听 127.0.0.1，而 Playwright globalSetup 的残留进程检查和截图服务都依赖这两项；审计轮会话通常没有这个限制（审计 #11、#12 跑通了截图与布局 14 条，审计 #13、#14 的布局命令却挂起无输出），两类会话环境不一致，验证只能跨轮接力"
  evidence: docs/web-面板按动森风格重做/evidence/M-11-r14-impl.md
  level: protocol
  proposal: "实现轮会话启动时允许执行 ps 与监听 127.0.0.1，以实现证据中不再出现 spawnSync ps EPERM 或 listen EPERM 为准"
  predicted_impact: {metric: manual_items_open, direction: down, scope: task, horizon: 3}
```
