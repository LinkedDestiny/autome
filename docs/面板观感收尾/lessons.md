# 面板观感收尾（T-102）：教训

```yaml
- id: L-01
  domain: design
  symptom: M-07 用掉 6 个实现轮（#9–#14）。#10–#13 这四轮都卡在宿主 e2e「1512×944 需求详情展开日志」：第十条底边越界（942.5 / 946.5 / 943.5 / 931.5 > 920），每轮凭推测再收紧一处间距；同样有高度预算的 M-04 角色卡一轮就过
  root_cause: 设计给 B-Q1 角色卡、C-Q1 技能网格、环境页都写了逐区块高度预算，时间线页却只写了「行高 ≤44px + 前十条可见」，没算时间线上方阶段区、统计卡、提示区和区块间距的占高；实现轮跑不了浏览器，到 #11 才发现顶部区块的占高没算进去，到 #12 才拿宿主截图列出预算表
  evidence: docs/面板观感收尾/evidence/M-07-r11-impl.md
  level: rule
  proposal: 设计中凡是「视口内完整可见 N 项 / 不被裁切」的验收，同一条款里必须附一张从视口顶到第 N 项底边的逐区块高度预算表，每行注明数值取自改前截图还是实测
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 审计 #7 在 M-07 用 e2e 实测折叠行高 51px（上限 44）而退回，这是本任务唯一一次 reopen。实现 #14 查明，e2e 服务的是审计本轮第三条负向对照编译出的旧构建；#14 没改 CSS，只让 e2e 启动前先构建，审计 #8 同一条 e2e 就通过了
  root_cause: e2e 夹具以 production 模式直接服务 apps/web/build 里已有的产物，不先构建；负向对照只以 git status 干净判定「已还原」，而构建目录不受 git 跟踪，源码还原后产物仍是注入版
  evidence: docs/面板观感收尾/evidence/M-07-r14-impl.md
  level: brief
  proposal: 负向对照之后、任何读取构建产物（如 apps/web/build）的检查之前，实现与审计证据中必须先记录一次重建命令及其退出码
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 14 次审计里只有 M-07 的审计 #7、#8 实跑了 playwright，而且在审计沙箱里能跑通；其余审计以实现轮记录的 ps EPERM 为由不跑浏览器，M-01 审计把 C-2 滚动保持移入 H-10，M-06 审计把 F-1 实跑移入 H-11，任务结束时还剩 11 条人工验收未确认
  root_cause: 审计把实现轮的环境限制当成自己的环境事实，没有先尝试执行；设计总则写的「审计轮能跑时也跑」没有规定怎样算尝试过
  evidence: docs/面板观感收尾/evidence/M-06-r8-audit.md
  level: brief
  proposal: 审计证据中每一项写成「跑不了 / 交宿主 / 移入人工项」的浏览器检查，都必须附本轮实际执行的命令，以及它的退出码或报错原文
  predicted_impact: {metric: manual_items_open, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: implementation
  symptom: 宿主门禁两次因为既有 e2e 仍按旧控件写而失败，每次多花一个实现轮。M-06 实现 #7 把项目下拉换成分段链接，controls.spec.ts 的 F-1 仍在等 combobox，超时 90 秒（#8 修）；M-10 实现 #17 把正常工具的安装说明改成默认收起，manual-items.spec.ts 的 W9 H-04 仍直接断言复制按钮可见（#18 修）
  root_cause: 实现轮的旧标记检索只覆盖类名和文案，没有覆盖被删除的 ARIA 角色和被改成默认隐藏的元素；e2e 在实现沙箱跑不了，这类残留只能等宿主门禁暴露
  evidence: docs/面板观感收尾/evidence/M-06-r8-impl.md
  level: rule
  proposal: 里程碑删除或替换某个控件角色（如 combobox），或把元素改为默认隐藏/折叠时，实现证据必须列出对 e2e 目录检索该角色、定位器与 testid 的命令和命中数，并在同一轮改完全部命中用例
  predicted_impact: {metric: round_gate_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: 三个实现轮的快检出现了与产品无关的失败，只能串行重跑。M-06 #7 的快检与删 clamp 的负向对照同时运行，skills-page 因此失败；M-10 #17 和 M-13 #21 的快检与验收或变异测试并发构建，add-project 初始化时找不到 .vite/manifest.json
  root_cause: 快检、会构建生产包的 vitest 用例和 e2e 共用同一个 apps/web 构建目录，并发运行时互相覆盖；快检读到的源码也可能是负向对照注入中的版本
  evidence: docs/面板观感收尾/evidence/M-13-r21-impl.md
  level: rule
  proposal: sh scripts/ci-quick.sh 运行期间，同一 worktree 内不得同时运行验收命令、负向对照或任何会构建 apps/web 的命令；证据里的快检结论只取串行运行的那一次
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
