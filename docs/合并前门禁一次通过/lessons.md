# 教训（T-99 合并前门禁一次通过 · 复盘第 1 轮）

本任务没有记录到 Autome 指标；下列条目只引用证据文件里的轮次与事实，不含估算数字。

```yaml
- id: L-01
  domain: verification
  symptom: "三个实现轮只跑了比宿主快检窄的静态检查，宿主快检随即在静态步骤上退回，各多花一个修复实现轮——实现 #6 只跑 `cargo clippy -p automed --lib`，宿主 clippy（--all-targets）抓到测试代码 needless_borrow，修复轮 #7；实现 #8 只对三个文件跑 eslint、没跑 typecheck，宿主 typecheck 报 TS2352，修复轮 #9；实现 #10 只对两个文件跑 eslint，宿主 typecheck 报 TS7006，修复轮 #11"
  root_cause: "实现轮把「对改动文件跑 eslint / lib clippy」当作静态检查，范围比宿主快检的 fmt、clippy --all-targets、lint、typecheck 窄；三轮的证据都写了「未跑项目快检」"
  evidence: docs/合并前门禁一次通过/evidence/M-06-r11-impl.md
  level: rule
  proposal: "实现轮结束前必须原样运行一次宿主快检命令（未拿到起点时至少跑完它的静态步骤），并在证据里写出该命令和退出码；证据里写「未跑项目快检」的实现轮视为没做完自审"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: verification
  symptom: "审计 #2 看到 ci-quick 第 14 例在负载 7～10 下失败（单跑 3 次过 2 次），判为「计时脆弱」写进 Backlog 后通过；到实现 #7，同一用例在脚本全集里又失败，改成受控时钟才修好（BL-07）。审计 #6 也把 BL-01 第 4 次 `toBeEnabled` 超时判为「负载所致，未单独复现」。T-98 的 L-06 也是这个模式（审计见过的失败被放行，之后原样复现）"
  root_cause: "审计对计时类失败只给负载解释，没有给确定性复现或确定性修法；反例是审计 #7 对 B-3 第 5 次失败找出读取顺序窗口并退回，实现 #12 用确定性回归（dependency-poll.test.ts）一次修好"
  evidence: docs/合并前门禁一次通过/evidence/M-02-r3-audit.md
  level: rule
  proposal: "审计复验中任一被执行用例失败、而审计以「负载」「计时脆弱」「竞态」为由不退回时，证据里必须写出能让它必红的确定性条件（受控时钟、插入的读取间隔等）或写出它在 n 次重复里的失败次数并满足 (1-p)^n ≤ 0.05；两样都没有就按实现缺陷退回"
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 3}

- id: L-03
  domain: verification
  symptom: "M-06 reopen 2 次。BL-01 的浏览器负向对照（相关分支恒假应变红）在实现 #8、#9 都因 ps EPERM 没跑过，实现轮拿组件测试变红顶替；第一次真实执行就是审计 #6，结果恒假变异下 BL-01 仍两次通过，退回。之后实现 #10 才把它固化成 `apps/web/scripts/test-related-mutation.mjs`，审计 #7、#8 原样执行均 5/5 红"
  root_cause: "任务 A 让宿主每轮跑浏览器正向用例（实现 #9 由宿主快检抓到 BL-01 失败，机制生效），但需要浏览器的负向对照没有宿主执行路径，仍然拖到审计才第一次运行——是本任务根因 1（浏览器用例到门禁才第一次跑）在负向对照上的同构"
  evidence: docs/合并前门禁一次通过/evidence/M-06-r9-audit.md
  level: test
  proposal: "验收契约里带 ★ 的负向对照，首个实现轮就要提交成仓库内一条可直接执行的脚本，宿主在该里程碑的快检浏览器阶段运行它并把「注入后是否变红」写进快检日志；审计前快检日志里没有这条结果的里程碑不得标待审"
  predicted_impact: {metric: reopen_total, direction: down, scope: project, horizon: 3}

- id: L-04
  domain: design
  symptom: "M-06 验收契约的 ★ 命令 `playwright test e2e/controls.spec.ts -g 'B-3|BL-01' --repeat-each=5` 本身有缺陷——重复的同名用例共用夹具根，第二遍起 60 秒等不到 await_design_approval，未改动的 B-1 同样失败。实现 #8、#9 因 EPERM 没跑起来，到审计 #6 才发现，由审计把契约改成 5 次独立调用"
  root_cause: "设计轮写下的 ★ 验收命令在定稿前没有在能起浏览器的会话里执行过；裁决记录两轮都写「本轮没有跑浏览器」，实现轮又跑不起来，所以命令缺陷要到审计才暴露"
  evidence: docs/合并前门禁一次通过/evidence/M-06-r9-audit.md
  level: brief
  proposal: "裁决轮定稿前，对每条新写或改写的 ★ 验收命令在可起浏览器的会话里至少执行一次（允许用例失败），并把命令与退出码写进裁决记录；没有执行记录的 ★ 命令在裁决记录里逐条标「未执行」"
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```
