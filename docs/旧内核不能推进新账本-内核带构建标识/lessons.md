# 教训

按 Autome 记录的指标：设计 1/15 轮，实现 13/35 轮（状态块写的是 14/35，差一轮，和复盘 #2 时一样，本轮仍没查明原因），共 7 个里程碑，全部已完成。reopen 合计 1 次，是 M-07 的 `package-isolation`（审计 #7）。「reopen 按领域」一栏 7 个领域各计 1 次，正好对应里程碑表的 7 个领域列，并不是 7 次退回。实现缺陷 3，验证缺口 2，协议失败 3，关闭后被推翻 0，人工验收未确认 3 条（H-01～H-03）。共 34127398 tokens、231 turns、13.1027 USD。

里程碑在审计 #8 全部关闭，之后又跑了五轮实现，都不推进里程碑：#10、#12 处理 rebase，#11、#13、#14 修合并前门禁。三次协议失败全出在这五轮里（#10、#11、#13），都是改了已关闭里程碑的「最新证据」指针，由操作者分别在 f4619d40、656a6e88、2ec1e891 手工改回。复盘 #2 之后多了两轮实现（#13、#14），#13 让 L-05 又发生了一次，L-06 是本轮新加的。L-01～L-04 沿用前两次复盘的结论。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #7（M-07，rebase 前提交 6d7402ff，现为 e354ed16）之后，宿主快检报 apps/web/test/ci-config.test.ts:62 的 no-regex-spaces，出错的是本轮新写的测试代码。为此多跑了实现 #8 一轮，只把正则里的两个空格改成 ` {2}`（rebase 前 d5a52e9b，现为 08a34481）
  root_cause: 实现 #7 只跑了 M-07 的验收命令（vitest ci-config.test.ts 与 sh -n），证据原话是「未运行宿主快检、全量门禁和人工验收」，没有跑 `npm run lint --prefix apps/web`。lint 会查测试文件，验收命令查不到，只有宿主快检会查。T-53 的 L-01 记过同一个缺口（当时是 clippy），这次换成 Web lint 又出现一次
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-impl.md
  level: brief
  proposal: 改了代码或测试的实现轮，标「待审」前要原样跑一遍宿主快检命令（cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web），并把退出码写进本轮证据；证据里没有这条记录的待审轮次算没照做
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 实现 #7 新加的 ci-config.test.ts 用例在实现轮环境里 8/8 通过，审计 #7 在 macOS 上跑同一条验收命令却是 7 过 1 败。失败的断言是 `path.startsWith(resolve(sandbox,'tmp') + '/')`，原因是本机 `mktemp -d` 不理会 TMPDIR。M-07 因此被退回（本任务唯一一次 reopen），多跑了实现 #9 和审计 #8
  root_cause: 新测试断言的不是被测脚本的契约（PREFIX 与 AUTOME_HOME 隔离、清理、保留退出码），而是系统工具 mktemp 会把目录建在哪。实现轮只在自己的环境里跑过，这个环境恰好符合该假设
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-audit.md
  level: brief
  proposal: 新增测试如果经过系统工具或平台行为（mktemp 的落点、TMPDIR、能否监听端口等），断言只能针对被测代码自己写出或传入的值；实现证据里还要附一次在替身或第二环境下的运行结果，那个环境里该平台假设不成立（做法同实现 #9 用 PATH 注入 mktemp 替身）
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: 审计 #4 跑整个 cli.rs 时 build_id_cli_and_kernel_agree 失败，原因是 target/debug/automed 是旧提交构建的，构建标识和 autome 对不上。审计轮手工跑了 cargo build -p automed 才转绿，按「环境陈旧，非产品缺陷」处理
  root_cause: 这条用例用 `cli.with_file_name("automed")` 读 target 里另一个包的二进制，但 `cargo test -p autome-cli` 不会重建 automed。构建标识每次提交都会变，所以提交后只要没重建 automed，单跑 autome-cli 测试必定失败一次，失败信息里也没提示产物陈旧。M-01 的验收命令带了 cargo build -p automed，M-04 的没带
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-04-r4-audit.md
  level: test
  proposal: 读取别的包 target 二进制的测试，要么先构建或校验该二进制，要么在标识不一致时报出「先 cargo build -p <包>」；里程碑验收命令里有这类测试的，命令必须先构建对应的包
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: M-06 经实现 #6、审计 #6 一次通过，合并前门禁却失败了：Web 722 例 721 过 1 败。失败的是原有用例 core-sidecar.test.ts「测试源码不按条件跳过真实内核用例」，它扫到 install-alignment.test.ts 里两处 `test.skipIf(process.platform !== 'darwin')`。为此多跑了实现 #11
  root_cause: 实现 #6 和审计 #6 只跑了 M-06 验收命令里的 2 例。扫描测试源码的那条守卫用例不在任何验收子集里，只有全量测试或门禁会跑到它
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-06-r11-impl.md
  level: brief
  proposal: 新增或修改了测试文件的实现轮，标「待审」前要跑一遍该测试所在包的全量测试（apps/web 为 `npm run test --prefix apps/web`），证据里写出总数、通过数和每个失败用例的名字；只有失败用例能逐条归到已知环境原因（如本地监听 EPERM）时才可以标待审
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: protocol-format
  symptom: 里程碑全部关闭后的五轮实现里有三轮（#10 改 M-02、#11 改 M-06、#13 改 M-05）把已关闭里程碑的「最新证据」指到了本轮的实现证据，三次都被内核守卫判为协议失败，正好是本任务记录的全部 3 次。#13 的证据原话是「M-05 最新证据指针指向本文件，里程碑已审状态保持不变」。没出错的 #12 和 #14 都在证据里写明了「保持原 M-05 审计证据指针」
  root_cause: protocol/prompts/impl.md 第 17 行要求「该里程碑只留一行指针『最新证据：<路径> · 结论 · 轮次』」，没区分里程碑是开放的还是已关闭的。简报 impl-10～impl-14 除轮次号外逐字相同，「本里程碑」一节只写「所有里程碑都已完成」，第 63 行还给了指针示例。复盘 #2（849cfceb）已经把这条记成 L-05、level 写的是 brief，但 impl-13 的简报没有任何改动，同一个错在下一轮就又发生了。同一任务里，写进 lessons.md 的教训不会自动送到后面的轮次
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r13-impl.md
  level: protocol
  proposal: 所有里程碑都已关闭时，实现轮的简报或 prompt 要写明「已关闭里程碑的『最新证据』行保持指向审计证据，本轮证据只写 evidence/ 文件和 retro.md 一行」；判据是这一轮的提交没有改动设计文档里任何「最新证据：」行
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: rebase 轮实现 #12 只跑了 M-05 子集和冲突相关的 71 例，之后合并前门禁连败两次，又多跑了实现 #13、#14。第一次失败在 849cfceb，w9-http.test.ts 7 例失败：夹具重启服务时没传 binaryPath，按本任务新的查找顺序选中了已安装的旧 CLI（`autome core` 未知命令）。第二次失败在 2ec1e891，task-ops-real.test.ts 的 beforeAll 在 170 秒处超时，16 例全部跳过。这两个文件都是 rebase 从 main 带进来的（w9 夹具来自 138037a7 等，task-ops-real 来自 53e49359）
  root_cause: 本任务改了「面板找内核」这个公共入口的行为，main 新带进来的测试会经过这个入口，但 rebase 轮只验证了冲突文件和本任务原有的子集。这两处失败在本机都能看出来：#13 的证据写明，即使监听 EPERM，「启动描述仍证实选择了已安装 CLI」；#14 单跑 task-ops-real 就在 170013ms 原样超时复现。也就是说，如果 #12 跑过这些文件，两个问题能在一轮里发现，不用两次门禁往返。#13 做的同类检查（枚举全部 startServer 调用）本来就可以放在 #12 做
  evidence: docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r13-impl.md
  level: brief
  proposal: rebase 轮的证据里要列出 `git diff --name-only <旧基>..<新基>` 带进来的测试文件，逐一在本轮运行，并给每个失败用例归因（产品缺陷，或已知环境原因并附上能看到的诊断输出）；没列出这份清单的 rebase 轮算没照做
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
