# 教训

按 Autome 记录的指标：设计 1/15 轮，实现 11/35 轮（状态块写的是 12/35，两者差一轮，本轮没有查明原因），共 7 个里程碑，全部已完成。reopen 合计 1 次，是 M-07 的 `package-isolation`（审计 #7）。实现缺陷 3，验证缺口 2，协议失败 2，关闭后被推翻 0，人工验收未确认 3 条（H-01～H-03）。共 30307838 tokens、211 turns、12.04 USD。

里程碑全部关闭后又跑了三轮实现：#10（M-02 rebase）和 #12（M-05 rebase）是用户追加的 rebase 指令；#11 修复合并前门禁报的 M-06 回归。两次协议失败都出在这几轮里，提交 f4619d40 与 656a6e88 的说明写明了是怎么回事。L-01～L-03 沿用复盘 #1 的结论，只按新指标改了措辞；L-04、L-05 是新加的。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #7（M-07，rebase 前提交 6d7402ff，现为 e354ed16）之后，宿主快检报 apps/web/test/ci-config.test.ts:62 的 no-regex-spaces（本轮新写的测试代码），为此多跑了实现 #8 一轮，只把正则里的两个空格改成 ` {2}`（rebase 前 d5a52e9b，现为 08a34481）；里程碑关闭之前，这是唯一一个不推进里程碑的实现轮
  root_cause: 实现 #7 只跑了 M-07 的验收命令（vitest ci-config.test.ts 与 sh -n），证据原话是「未运行宿主快检、全量门禁和人工验收」，没跑 `npm run lint --prefix apps/web`。这条 lint 只查测试文件，验收命令发现不了，只有宿主快检能发现。T-53 的 L-01 记过同一个缺口（当时是 clippy），本任务换成 Web lint 又出现一次。实现 #8 照做后，本地快检退出 0，跟宿主结果一致
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-impl.md
  level: brief
  proposal: 改了代码或测试的实现轮，标「待审」前要原样跑一遍宿主快检命令（cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web），并把退出码写进本轮证据；待审轮次的证据里没有这条记录，就算没照做
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 实现 #7 新加的 ci-config.test.ts 用例在实现轮环境里 8/8 通过；审计 #7 在 macOS 上跑同一条验收命令，7 过 1 败。失败的断言是 `path.startsWith(resolve(sandbox,'tmp') + '/')`，因为本机 `mktemp -d` 忽略 TMPDIR。M-07 被退回（本任务唯一一次 reopen），多跑了实现 #9 和审计 #8。验收命令本身已经红了，审计 #7 也就没法用它做负向对照
  root_cause: 新测试断言的不是被测脚本的契约（PREFIX 与 AUTOME_HOME 隔离、互不相同、结束后清理、保留退出码），而是系统工具 mktemp 在某个环境下会把目录建在哪。实现轮只在自己的环境里跑过，而这个环境恰好满足该假设（实现 #9 写明「本环境 8/8 通过，与上一轮 macOS 结果不同」）。实现 #5、#12 也记过同一类环境差异：实现轮沙箱不让监听本地端口（EPERM），宿主环境可以
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-audit.md
  level: brief
  proposal: 新增测试若会经过系统工具或平台行为（mktemp 把目录建在哪、TMPDIR、能否监听端口等），断言只能针对被测代码自己写出或传入的值；实现证据里还要有一次运行结果，是在替身或第二环境下跑的，在那里该平台假设不成立（做法同实现 #9 用 PATH 注入 mktemp 替身）
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: 审计 #4 跑整个 cli.rs 时 build_id_cli_and_kernel_agree 失败，原因是 target/debug/automed 由旧提交构建，构建标识与 autome 对不上。审计轮手工跑 cargo build -p automed 后才转绿，按「环境陈旧，非产品缺陷」处理
  root_cause: 这条用例（crates/autome-cli/tests/cli.rs）用 `cli.with_file_name("automed")` 去读 target 目录里另一个包的二进制，但 `cargo test -p autome-cli` 不会重建 automed 的 bin。构建标识每次提交都会变，所以提交后只要没重建 automed，单跑 autome-cli 测试就一定会失败一次。失败信息只给出两段不同的标识，没提示「automed 产物陈旧」。只有 M-01 的验收命令前面带了 cargo build -p automed，M-04 的没带
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-04-r4-audit.md
  level: test
  proposal: 读取别的包的 target 二进制的测试，要么在测试里先构建或校验该二进制，要么在二进制标识与 HEAD 不一致时报出「先 cargo build -p <包>」这类失败信息；里程碑验收命令若包含这类测试，命令里必须先构建对应的包
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: M-06 经实现 #6、审计 #6 一次通过，合并前门禁却失败：Web 共 722 例，721 过 1 败。失败的是项目原有用例 core-sidecar.test.ts「测试源码不按条件跳过真实内核用例」，它扫到 install-alignment.test.ts 里两处 `test.skipIf(process.platform !== 'darwin')`。为此多跑了实现 #11 一轮，把两处改成普通 test，再在夹具里显式断言 darwin
  root_cause: 设计方案 13 和 M-06 写的就是「只在 darwin 上运行」，实现 #6 照此写了 skipIf。实现 #6 和审计 #6 只跑了 M-06 的验收命令（install-alignment.test.ts 的 2 例，本机是 darwin，两例都实际执行并通过），没有跑 apps/web 的全量测试。扫描测试源码的守卫用例（2026-11-10 的 0f82391d 起就有）不在任何验收子集里，只有全量测试或门禁会跑它
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-06-r11-impl.md
  level: brief
  proposal: 新增或修改了测试文件的实现轮，标「待审」前要跑一遍该测试所在包的全量测试命令（apps/web 为 `npm run test --prefix apps/web`），在证据里写出总数、通过数和每个失败用例的名字；失败用例只有能逐条归到已知环境原因（如本地监听 EPERM）时才可以标待审
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: protocol-format
  symptom: 里程碑全部关闭后的两轮实现：实现 #10（M-02 rebase）把已完成 M-02 的「最新证据」改指到 evidence/M-02-r10-impl.md，实现 #11（M-06 门禁修复）把 M-06 的改指到 evidence/M-06-r11-impl.md。两次都被内核守卫判为协议失败，正好是本任务记录的 2 次协议失败。两次都是操作者手工把指针改回审计证据（f4619d40、656a6e88）。之后的实现 #12（M-05 rebase）没动指针，就没有出错
  root_cause: 本任务固定的 protocol/prompts/impl.md 第 17 行要求实现轮在设计文档里给「该里程碑」写一行「最新证据：<路径> · 结论 · 轮次」，没有区分里程碑是开放的还是已关闭的。而简报 impl-10/impl-11 的「本里程碑」一节写的是「还没有里程碑表，或者所有里程碑都已完成」，也没说已关闭里程碑的指针不能改。实现 #11 的证据前面写了「已审状态原样保留」，却照样改了指针，说明这是照着 prompt 写的，并非有意改状态
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-06-r11-impl.md
  level: brief
  proposal: 所有里程碑都已关闭时，实现轮简报要写明「已关闭里程碑的『最新证据』行保持指向审计证据，本轮实现证据只写 evidence/ 文件和 retro.md 一行」；照做的判据是这一轮的提交没有改动设计文档里任何「最新证据：」行
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
```
