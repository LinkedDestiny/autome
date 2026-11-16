# M-03 实现轮 #4

结论：待审。三类失败清单、截断原因、按作业保留原文、内核事件与修复提示词/简报交付已实现。真实 Playwright 7 条的进程残留验收被 `spawnSync ps EPERM` 拦住，按设计「验收契约」★与任务约束 5 交审计复验；没有删除断言、增加条件跳过或放宽沙箱。

## 改动与修复

- 报告器每次输出失败用例（错误和断言位置）、未执行用例（含被中断耗时）、非用例错误；附各用例、setup/teardown 与预算过半耗时。详情按 1024/2048 字节起步，超过 12 KiB 逐步缩至 300 字节，标题与位置不删，UTF-8 不切半。
- 有 `AUTOME_GATE_LOG` 时保存 `<日志>.playwright.json`，固定 `report.json` 仍供脚本读取；digest 第一行给未截原文绝对路径。后续作业不会覆盖前一作业原文。
- setup 提前失败没有 `onBegin` 时，沿用原 CLI 选择参数执行 `--list --reporter=json`（30 秒），回取全部未执行用例；读取失败/超时明确写「未能取得用例清单」。
- 实测发现 Playwright 在 deadline 到期、替换 worker 尚未启动时可生成 `skipped` 结果。此类非显式 skip 按是否收到 `onTestBegin` 归入 not_run/interrupted；显式 skip 不改。首次真实检查红为 `[failed, skipped, not_run]`，修正后真实清单验证通过、随后只在 ps 复查处受限。原始证据：`.autome/output/m03-real-check.txt`、`m03-real-recheck.txt`、`m03-real-cut.txt`。纯函数用例也覆盖该分支。
- `gate::digest` 只取末 256 KiB 内完整闭合段，按顺序合并、16 KiB 字符边界裁切并标日志位置；tail 排除清单段。`gate.result.digest` 经 `GateOutcome`、`Inject::GateFailed` 进入需求/Bug 修复轮。可选字段有 serde 缺省，旧事件反序列化通过。
- 需求修复简报含门禁类别、SHA 与完整日志位置。两条真实内核端到端测试使用本轮真实超时输出规范化的 `crates/automed/tests/fixtures/gate-digest-timeout.txt`，段后追加 80 行，验证事件、提示词及实际简报，证明不是依赖末 60 行。
- `summary.mjs` 增加 playwright 解析器，`Web e2e` 步骤与配置钉子同步。全仓字段使用方复核见 `.autome/output/m03-all-consumers.txt`：CLI/timeline 读既有字段，无新增显示与破坏性契约改动。
- README Host gate 与技术设计同步说明清单标记协议、原文留存和事件交付。
- 上轮审计为 M-02 通过，没有要求本轮修复的缺陷检查，因此「上轮缺陷原样搬入、红绿复现」不适用。

## 验收命令、数量与结果

开工基线取 HEAD `33f0d53e`。主验收基线 **83** 条，本轮 **104** 条，差 **+21**：digest +5、prompt +4、end_to_end +2、summary +2、reporter +6、reporter-run +2。设计原写 reporter 12，实际开工已有 14（M-01 补强的两条清理测试），本轮为 20，已就地修正契约数字。

| 命令 | 本轮/基线 | 结果 | 原始输出 |
|---|---:|---|---|
| `cargo test -p automed --lib gate::tests::digest` | 5/0 | 5 通过 | `.autome/output/m03-digest.txt` |
| `cargo test -p automed --test prompt_rendering` | 47/43 | 47 通过 | `.autome/output/m03-prompt-final.txt` |
| `cargo test -p automed --test end_to_end gate_digest` | 2/0 | 2 通过 | `.autome/output/m03-e2e-final.txt` |
| `node --test scripts/test/ci-summary.test.mjs` | 10/8 | 10 通过 | `.autome/output/m03-summary.txt` |
| `(cd apps/web && npx vitest run test/gate-reporter.test.ts test/gate-reporter-run.test.ts test/ci-config.test.ts)` | 40/32 | 33 通过、7 环境受阻、0 跳过 | `.autome/output/m03-web-acceptance.txt` |

上面为逐项执行的验收契约命令，主验收合计 97 通过、7 受阻。不是全量门禁通过。

直接相关的补充验证：

- `cargo test -p automed --lib`：1029/1024，全部通过（含前表 5 条），`.autome/output/m03-lib.txt`。
- `cargo test -p autome-domain --lib`：487/487，全部通过，`.autome/output/m03-domain.txt`。
- `node --test scripts/test/*.test.mjs`：68/66，全部通过（含前表 10 条），`.autome/output/m03-scripts.txt`；只运行项目脚本测试体系，未启动实际项目快检或全量门禁。
- 去重后本轮上述测试共 **1673** 条，对应基线 **1652**，差仍为 **+21**；1666 通过、7 环境受阻。筛选重跑不重复计数。
- `cargo fmt --all`、`cargo clippy -p automed -p autome-domain --lib -- -D warnings`、`git diff --check` 均通过。
- 在 apps/web 下对本轮五个 TS 文件执行 `npx eslint e2e/harness/gate-digest.ts e2e/harness/gate-reporter.ts test/gate-reporter.test.ts test/gate-reporter-run.test.ts test/ci-config.test.ts` 与 `npx tsc --ignoreConfig --target ES2023 --module nodenext --types node --strict --noEmit --allowImportingTsExtensions --skipLibCheck e2e/harness/gate-digest.ts e2e/harness/gate-reporter.ts test/gate-reporter.test.ts test/gate-reporter-run.test.ts test/ci-config.test.ts`：通过，输出 `m03-eslint.txt`、`m03-types.txt`。未运行宿主快检命令或 `sh scripts/ci-local.sh` 实际门禁。

## 标待审之前的四项自审（已逐项执行）

1. **情形表逐行**：B-1 的失败/未执行/非用例错误三行由「三类清单、断言位置与各项耗时齐全」逐项断言；B-2 总超时、浏览器预算、setup 失败、其它中止四行由「四种截断成因不混淆」验证；成功运行仍有三类空清单；长清单保留第 40 条标题与位置；单条 UTF-8 限制与作业原文留存均通过。真实总超时及 setup 无 onBegin 的内容断言也已执行通过，后置 ps 复查受阻。
2. **失败分支**：断言失败、未开始、被中断、非用例错误、总超时、预算截断、setup 抛错分别有上述逻辑检查与真实夹具；`--list` 获取失败/超时用 `list timeout` 错误验证报告提示；内核缺失日志读盘失败返回无 digest；summary 缺失报告不报错；既有清理扫描失败测试仍通过并保留登记。拒绝授权/外部服务不可达不适用：本里程碑不新增授权与外部服务，实际 ps 拒绝另列为环境阻断，未伪装成功。
3. **全部迁移**：需求 Gate 失败→Implement、MergeGate 失败→Implement 两条端到端通过，且实际简报含对应门禁种类；Bug FixGate/MergeGate 失败→Fix 的既有领域测试随 487 条全过，Bug quick/full 提示词注入分别检查；无 digest 的旧事件及原 golden 不变。报告结果 passed/failed/timedout/interrupted 与完整/无结果/被中断分支均有检查；无新增调度状态或车道。
4. **输入域边界**：无段、单段、多段、未闭合、首标记掉出 256 KiB、超过 16 KiB 的中文；空详情、1024 字节恰好边界、300 字节收缩、40 条清单、无报告文件、旧事件缺字段均通过。日期/前导零/闰日不适用：此里程碑没有日期或数字字符串解析输入。

## 交审计的环境受阻项

完整复验命令：`(cd apps/web && npx vitest run test/gate-reporter-run.test.ts)`。以下 7 条本轮均报 `spawnSync ps EPERM`，不得记为通过：

- 真实运行包含 setup/teardown
- 快检过余量线改写真实退出码
- 合并前门禁过余量线退出零并保留真实样本
- 真实 poll 两秒预算一点五秒过半
- 预算截断后回收登记的服务及临时根（启动服务前 ps 检查即被拒绝）
- 真实总超时三类清单与未执行用例
- globalSetup 无 onBegin 时用 list 找回全部选择的用例

最后两条先验证清单/状态再复查残留，因此可区分“内容断言通过”和“完整清理验收未通过”。不把局部内容证据等同于上述七条通过。审计再按设计执行负向对照；本轮未声称做过 mutation 验收。
