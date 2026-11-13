# 教训

Autome 没有为本任务记录指标，下面只引用证据文件、retro.md 和 git 记录里写明的事实。整轮运行：设计 1 轮（评审 #1 提出的 R1-P01～P04 由裁决 #1 全部采纳，设计定稿），实现 9 轮，审计 8 次。共 7 个里程碑，全部已完成。M-07 由实现 #2 按用户裁定从 Backlog B-03 新开。reopen 合计 1 次，是 M-07 的 `package-isolation`；M-01～M-06 都由审计一次通过。M-07 前后用了实现 #7、#8、#9 三轮：#7 实现；#8 只修宿主快检报的 lint（d5a52e9b）；审计 #7 退回后，#9 修一处平台假设（fd5c6646）。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #7（M-07，提交 6d7402ff）提交后，宿主快检报 apps/web/test/ci-config.test.ts:62 的 no-regex-spaces（本轮新写的测试代码），为此多跑了实现 #8 一轮，只把正则里的两个空格改成 ` {2}`（d5a52e9b）；这是 9 个实现轮里唯一一个不推进里程碑的修复轮
  root_cause: 实现 #7 只跑了 M-07 的验收命令（vitest ci-config.test.ts 与 sh -n），证据原话是「未运行宿主快检、全量门禁和人工验收」，没跑 `npm run lint --prefix apps/web`；这条 lint 只针对测试文件，验收命令发现不了，只有宿主快检能发现。T-53（排队任务可以调整顺序并声明依赖）的 L-01 已记过同一个缺口（当时是 clippy），本任务换成了 Web lint，又出现了一次
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-impl.md
  level: brief
  proposal: 改了代码或测试的实现轮，标「待审」前要原样跑一遍宿主快检命令（cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web），并把退出码写进本轮证据；待审的实现轮证据里没有这条记录，就算没照做
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 实现 #7 新加的 ci-config.test.ts 用例在实现轮的环境里 8/8 通过；审计 #7 在 macOS 上跑同一条验收命令，结果 7 过 1 败。失败的断言是 `path.startsWith(resolve(sandbox,'tmp') + '/')`，原因是本机 `mktemp -d` 忽略 TMPDIR。M-07 因此退回（reopen 1），多跑了实现 #9 和审计 #8。验收已经红了，审计 #7 也就没法拿它做负向对照
  root_cause: 新测试断言的不是被测脚本的契约（PREFIX 与 AUTOME_HOME 隔离、互异、结束后清理、保留退出码），而是系统工具 mktemp 在某个环境下会选哪个目录。实现轮只在自己的环境里跑过，这个环境恰好满足那个假设（实现 #9 写明「本环境 8/8 通过，与上一轮 macOS 结果不同」）。实现 #5 也记过同一类环境差异：实现轮的沙箱不让监听本地端口（EPERM），审计轮的环境可以
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-audit.md
  level: brief
  proposal: 新增测试若会经过系统工具或平台行为（mktemp 的落点、TMPDIR、能否监听端口等），断言只能针对被测代码自己写出或传入的值；实现证据里还要有一次运行结果，是在替身或第二环境下跑的、在那里该平台假设不成立（做法同实现 #9 用 PATH 注入的 mktemp 替身）
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: 审计 #4 跑整个 cli.rs 时 build_id_cli_and_kernel_agree 失败，原因是 target/debug/automed 由旧提交构建，标识跟 autome 对不上。审计轮先手工跑 cargo build -p automed 才转绿，按「环境陈旧，非产品缺陷」处理
  root_cause: 这条用例（crates/autome-cli/tests/cli.rs）用 `cli.with_file_name("automed")` 去读 target 目录里另一个包的二进制，可 `cargo test -p autome-cli` 不会重建 automed 的 bin。构建标识又会随每个提交变化，所以只要提交后没有重建 automed，单跑 autome-cli 测试就会稳定失败一次；失败信息只给出两段不同的标识，没提示「automed 产物陈旧」。只有 M-01 的验收命令前面带了 cargo build -p automed，M-04 的没带
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-04-r4-audit.md
  level: test
  proposal: 读取别的包的 target 二进制的测试，要么在测试里先构建或校验该二进制，要么在二进制标识与 HEAD 不符时用「先 cargo build -p <包>」这样的失败信息报出；里程碑验收命令若包含这类测试，命令里必须先构建对应的包
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```
