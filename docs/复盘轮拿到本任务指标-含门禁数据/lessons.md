# 教训（T-100 复盘轮拿到本任务指标 · 复盘第 1 轮）

本任务没有记录到 Autome 指标。这在预期之内：驱动本轮的是已安装的内核，本任务 A 组的修复要合并、重装之后才生效（人工验收 H-02）。
下列条目只引用证据文件里的轮次与事实，不含估算数字。

可核对的运行概况，见 `retro.md` 与设计状态块：

- 设计 1/15。评审 #1 提出 R1-P01、R1-P02 两项，裁决 1 全部采纳，当轮定稿。
- 实现 3/15。M-01…M-03 各一个实现轮、一个审计轮，审计都是一次通过，reopen 全为 0。
- 实现缺陷 0。审计记录的验证缺口为 0，但实现 #3 交给审计的「全脚本回归未取得终结结果」实际没有闭合，见 L-01。
- H-01、H-02 两条人工验收项尚未确认。

复盘轮补充的一次实测（只写进 `.autome/output/`，不入仓库）：
在本 worktree 的 HEAD `c7021629` 上，执行 `NO_COLOR=1 node --test scripts/test/*.test.mjs`（Node v26.5.0）。
11 分钟后，其它文件都已跑完，只有 `ci-reuse.test.mjs` 还没结束。它的子进程链是
`sh scripts/ci-local.sh` → `node …/inputs.mjs prepare` → `git hash-object --stdin`，最后这个 git 已卡了 9 分 54 秒。
采样调用栈停在 `cmd_hash_object → strbuf_read → xread`，也就是一直在等 stdin 结束。
原始记录：`.autome/output/retro1-scripts.txt`、`.autome/output/retro1-hang.txt`。作为对照，T-99 合并前门禁的 `Scripts` 步骤
（同一条命令）跑完 71 个用例只用了 208 秒。可见这不是「跑得慢」，而是卡死。本轮已停掉这次运行，没有改动任何被跟踪的文件。

```yaml
- id: L-01
  domain: verification
  symptom: "实现 #3 两次执行设计要求的脚本回归 `node --test scripts/test/*.test.mjs`，都因长时间没有结束而手动中止（中止时汇总的被取消文件分别是 ci-reuse.test.mjs，427 秒；ci-e2e.test.mjs，187 秒）。证据把它写为「剩余验证缺口……交独立审计复验」。审计 #3 没有执行这条命令，结论写「没有产品缺陷」「无验证缺口」并关闭了 M-03。复盘轮复跑同一命令，ci-reuse.test.mjs 卡了 11 分钟以上"
  root_cause: "实现轮点名交给审计的缺口没有接手的人：审计提示词把全量回归交给合并前门禁，于是审计按惯例跳过。可这条命令是任务 §9.5 为 scripts/ci 改动指定的「直接相关子集」，也正是合并前门禁 Scripts 步骤的原命令。缺口在两轮之间丢了，审计结论里的「无验证缺口」与实现证据互相矛盾"
  evidence: docs/复盘轮拿到本任务指标-含门禁数据/evidence/M-03-r3-audit.md
  level: brief
  proposal: "实现证据里列为「剩余验证缺口」并交审计的每条命令，审计证据必须逐条写出复跑结果（命令、退出码、用例数），或者把它原样记为验证缺口；只要实现证据里还有未闭合的缺口，审计就不得写「无验证缺口」"
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 3}

- id: L-02
  domain: tooling
  symptom: "门禁脚本测试全集在本机会挂住而不是失败：实现 #3 两次跑全集都没有自然结束。复盘轮复跑时，ci-reuse 夹具里的 inputs.mjs prepare 调起的 `git hash-object --stdin` 阻塞在读 stdin 上近 10 分钟。按验收命令单独跑 ci-reuse + ci-install（29 个）是全部通过的"
  root_cause: "inputs.mjs 第 13 行的 hash() 用不带 timeout 的 execFileSync 把内容经 stdin 喂给 git。一旦子进程的 stdin 迟迟收不到 EOF（复盘轮在 Node v26.5.0、多个测试文件并发时复现），整个调用链就一直阻塞，直到外层门禁超时。stdin 为何没有关闭，本轮没有定位；实现 #3 又因沙箱拒绝 ps 拿不到进程树，同样没能定位"
  evidence: docs/复盘轮拿到本任务指标-含门禁数据/evidence/M-03-r3-impl.md
  level: test
  proposal: "scripts/ci/ 下与 scripts/test/ 下每一处 execFileSync / spawnSync / execSync 调用都显式带 timeout，超时以非零退出、并在输出里写出被杀的命令；用一条 grep 守卫测试钉住「无 timeout 的同步子进程调用为 0 处」"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}

- id: L-03
  domain: design
  symptom: "设计初稿的 R19 写着第二次门禁「三步都是 reused」，B-Q1 的重算条件只看「本次准备成功」。评审 #1 在现有算键器的隔离副本上跑探针（review1-key-probe.mjs）推翻了这两点：安装前和两次安装之间的步骤，键会被后面的安装改掉（R1-P01）；安装失败之后，再有一个准备步骤成功，会重新认可失败安装留下的依赖状态（R1-P02）。两项都由裁决 1 采纳，并改写了 R19，新增 R23、R24、Q20 与两条负向对照"
  root_cause: "设计轮推导用例预期时，没有让现有代码（inputs.mjs 的 toolchain/calculate、lib.sh 的 execute_prep）把这些预期真跑一遍，评审第一次执行就推翻了。这与 T-99 L-04（★ 验收命令定稿前没执行过，docs/合并前门禁一次通过/lessons.md）同属一类：设计写下的预期在定稿前没有执行过"
  evidence: docs/复盘轮拿到本任务指标-含门禁数据/复盘轮拿到本任务指标-含门禁数据-review.md
  level: brief
  proposal: "设计定稿前，验收契约和新用例里凡是依赖现有代码输出的预期（例如某步骤是 reused 还是 ran、键相等还是不等），都要在现有代码上至少执行一次探针，并把探针命令和输出路径写进设计或裁决记录；没有探针记录的预期，在裁决记录里逐条标「未执行」"
  predicted_impact: {metric: design_rounds_used, direction: down, scope: project, horizon: 3}
```
