# 面板观感收尾（T-102）：教训

复盘轮 #2 覆盖写。相对复盘轮 #1：补上合并前门禁失败（1/1）与随后两次协议失败（实现 #22、#23）的教训，原 L-03 与新证据合并，其余条目按证据复核后保留。

```yaml
- id: L-01
  domain: verification
  symptom: 合并前门禁唯一一次运行失败。task-log-live.test.ts 仍按全部 <details 计数（期望 2 实得 4），因为 M-07 把时间线文字格也改成了 details.timeline-text。因此多出修复轮 #22、交接轮 #23 #24 和审计 #15，两次协议失败也都出在这几轮里。M-07 实现 #9 已在证据里点名这个文件「快检预算内未执行，由宿主合并前门禁覆盖」；审计 #8 在自己的沙箱里跑通了 playwright，却只跑了 layout.spec.ts -g 需求详情展开日志，没有跑点名的这个文件
  root_cause: 实现证据交出的「未执行、交宿主」清单没有接收方。审计只复验验收契约与自选的判别检查，不逐条接手交来的缺口；其余审计更是直接沿用实现轮的 EPERM，不自己尝试（M-01 审计把 C-2 移入 H-10，M-06 审计把 F-1 移入 H-11），任务结束时 13 条人工项仍未确认
  evidence: docs/面板观感收尾/evidence/M-07-r9-impl.md
  level: brief
  proposal: 审计证据必须逐条列出该里程碑各实现证据中标为「未执行 / 交宿主 / 待宿主复验」的测试文件或命令，并给出本轮实际执行的命令与退出码；执行失败的要附报错原文
  predicted_impact: {metric: merge_gate_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: protocol-format
  symptom: 两次协议失败都出在没有开放里程碑的合并前门禁修复轮。实现 #22 把证据写成 evidence/fix-r22-impl.md，不符合 M-xx-r<k>-impl.md；实现 #23 按简报写了 M-13-r23-impl.md，但状态块停在 implementation-round 22/66，守卫按 r22 找证据，判「没有留下证据文件」。每次都靠操作者备注（#2、#3）补救，多花一轮
  root_cause: 协议的证据命名与轮号递增都是按「推进某个里程碑」写的，里程碑全部完成之后的修复轮该用哪个 M-xx、要不要加 implementation-round，协议没写。impl-22 简报里没有给证据路径；impl-23 简报给了路径，却没说状态块要写成 23/66
  evidence: docs/面板观感收尾/evidence/M-13-r24-impl.md
  level: brief
  proposal: 每个实现轮简报（包括里程碑已全部完成的修复轮与交接轮）都写明本轮证据文件的完整路径，以及状态块 implementation-round 应写成的值
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: implementation
  symptom: 改控件或标记之后，实现沙箱跑不了的旧断言三次到宿主才变红，每次多花一轮。M-06 实现 #7 把项目下拉换成分段链接，controls.spec.ts 的 F-1 仍在等 combobox，超时 90 秒（#8 修）；M-10 实现 #17 把正常工具的安装说明改成默认收起，manual-items.spec.ts 的 W9 H-04 仍直接断言复制按钮可见（#18 修）；M-07 把时间线文字格改成 details，exclusive 组的 task-log-live.test.ts 仍数全部 <details，到合并前门禁才失败（#22 修）
  root_cause: 约束 3 的旧值检索只覆盖类名和文案，没有覆盖被删除的 ARIA 角色、新引入或改用的元素标签、改为默认隐藏的元素；命中的文件又多在 e2e 或 exclusive 组，实现沙箱因 listen/ps EPERM 跑不了，只能等宿主暴露
  evidence: docs/面板观感收尾/evidence/M-06-r8-impl.md
  level: rule
  proposal: 里程碑删除或替换某个控件角色、新引入或改用某个元素标签（如 details），或把元素改为默认隐藏或折叠时，实现证据必须列出对 apps/web/test 与 apps/web/e2e 检索该角色、标签、定位器的命令和命中数，并在同一轮改完全部命中的断言
  predicted_impact: {metric: round_gate_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: design
  symptom: M-07 用掉 6 个实现轮（#9–#14）。#10–#13 这四轮都卡在宿主 e2e「1512×944 需求详情展开日志」：第十条底边越界（942.5 / 946.5 / 943.5 / 931.5 > 920），每轮凭推测再收紧一处间距；同样有高度预算的 M-04 角色卡一轮就过
  root_cause: 设计给角色卡（B-Q1）、技能网格（C-Q1）、环境页都写了逐区块高度预算，时间线页却只写了「行高 ≤44px + 前十条可见」，没算时间线上方阶段区、统计卡、提示区和区块间距的占高。实现轮跑不了浏览器，到 #11 才发现顶部区块的占高没算进去，到 #12 才拿宿主截图列出预算表
  evidence: docs/面板观感收尾/evidence/M-07-r11-impl.md
  level: rule
  proposal: 设计中凡是「视口内完整可见 N 项 / 不被裁切」的验收，同一条款里必须附一张从视口顶到第 N 项底边的逐区块高度预算表，每行注明数值取自改前截图还是实测
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: 审计 #7 在 M-07 用 e2e 实测到折叠行高 51px（上限 44）而退回，这是本任务唯一一次 reopen。实现 #14 查明，e2e 服务的是审计本轮第三条负向对照编译出的旧构建；#14 没改 CSS，只让 e2e 启动前先构建，审计 #8 用同一条 e2e 就通过了
  root_cause: 负向对照只以 git status 干净判定「已还原」，而 apps/web/build 这类被 git 忽略的构建产物不随源码还原；当时 e2e 夹具直接服务已有产物，不先构建
  evidence: docs/面板观感收尾/evidence/M-07-r14-impl.md
  level: brief
  proposal: 负向对照还原之后，只要后面还有读取构建产物（如 apps/web/build）的检查，实现与审计证据就必须先记录一次重建命令及其退出码，再写这些检查的结论
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: tooling
  symptom: 三个实现轮的快检出现了与产品无关的失败，只能串行重跑。M-06 #7 的快检与删 clamp 的负向对照同时运行，skills-page 因此失败；M-10 #17 和 M-13 #21 的快检与验收或变异测试并发构建，add-project 初始化时找不到 .vite/manifest.json，串行重跑后不再复现
  root_cause: 快检、会构建生产包的 vitest 用例、e2e 共用同一个 apps/web 构建目录，并发运行时互相覆盖；快检读到的源码也可能是负向对照注入中的版本
  evidence: docs/面板观感收尾/evidence/M-13-r21-impl.md
  level: rule
  proposal: sh scripts/ci-quick.sh 运行期间，同一 worktree 内不得同时运行验收命令、负向对照或任何会构建 apps/web 的命令；证据里的快检结论只取串行运行的那一次
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
