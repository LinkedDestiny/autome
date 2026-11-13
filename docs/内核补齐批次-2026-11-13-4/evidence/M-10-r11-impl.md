# M-10 · 实现 #11

范围：J-E1–J-E3、J-T1。上一轮审计 M-09 通过，无退回检查需要搬入。本轮只推进 M-10。

## 标待审之前的自审清单

1. **情形表逐行**：旧模板 + 有范围 → legacy 集成审计；旧模板 + 无范围 → 不启用；新模板 → 既有强制审计；已完成指针 → 还原；开放里程碑、围栏及其它正文 → 不动。用 `closed_pointer_`、`legacy_pointer_` 与 `integration_` 检查，12 个主用例与相关 45 个集成用例通过。
2. **失败分支**：无法识别文档、读 Git 失败、工作区不干净、索引锁导致提交失败均应保留错误；旧协议集成审计缺证据在表不动/退回两种分支均为提醒。拒绝授权、外部服务不可达、超时不适用：本次修复无授权或外部服务调用。全部通过；追加只读文件的写盘失败检查，也保留原文。
3. **全部迁移**：实现 → 自动还原 → 集成审计 → 复盘/合并卡点；还原失败 → 协议失败；集成审计通过/退回；实现擅自关闭里程碑仍拒绝。新旧模板分别核查，全部通过；对应 `guards::tests` 37 个、集成 e2e 5 个及本项 e2e 3 个。
4. **输入域边界**：空文档、无匹配段、无原指针时新增一行/多行、列表前缀、反引号/波浪围栏、LF/CRLF、无末尾换行；版本 v0/v7/v8/v9、前导零、u32 最大值/溢出、空串/非法格式。日历日期不适用：无日期输入。通过，边界断言在 2 个纯函数用例与 1 个版本解析用例中逐项执行。

## 修复与回归

- 缺陷回归测试独立提交 `1923e77d`，未含产品修复；提交前 `cargo fmt --all -- --check` 退出 0（`.autome/output/r11-fmt-check.txt`）。
- 同一复现命令：`cargo test -p automed --test end_to_end legacy_pointer_e2e_v7_task_is_restored_and_audited`。
- 红：1 个用例，0 通过、1 失败；`impl_closed_milestone` 指针错误导致未抵达 `await_merge`。完整输出 `.autome/output/r11-red.txt`。
- 真实 v7 模板取自 `47fa384a^:protocol/prompts/audit.md`，该提交 CHANGELOG 最新为 protocol/v7；冻结夹具 `crates/automed/tests/fixtures/protocol-v7-audit.md`。
- 自动还原只重写受影响的指针行，保存内核提交与 `guard.auto_fixed` 事件；所有版本均自动修正，文案区分版本。重新跑守卫，不能吞掉同码的「实现轮关闭里程碑」错误。
- 旧模板已有集成范围时保留范围/触发标签/首次基线并加 `legacy: true`，简报由内核补验收说明，缺证据的两个分支只提醒；新模板载荷不增加 legacy 键。
- 技术设计同步兼容原则与例外；CLI 渲染自动还原提醒。
- 初次故障夹具留下未跟踪文件，被生产兜底提交正常收走，因此未触发失败；改为会话提交后持有 Git 索引锁。脏工作区分支另用直接调用测试验证，不放宽实现。

## 验收命令与总数

| 命令 | 最终用例数与结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed -- closed_pointer_ legacy_pointer_` | 12，通过（库 9 + e2e 3） | `.autome/output/r11-green-final.txt` |
| `cargo test -p autome-domain protocol_ref_version` | 1，通过 | `.autome/output/r11-domain.txt` |
| `cargo test -p autome-cli legacy_pointer_` | 1，通过 | `.autome/output/r11-cli.txt` |
| `cargo test -p automed -- integration_` | 45，通过（库 36 + e2e 5 + 金样 4） | `.autome/output/r11-integration.txt` |
| `cargo test -p automed --lib guards::tests` | 37，通过 | `.autome/output/r11-guards.txt` |
| `cargo test -p automed --test end_to_end legacy_pointer_e2e_v7_task_is_restored_and_audited` | 绿：与红相同命令，1 通过 | `.autome/output/r11-repro-green.txt` |
| `cargo test -p automed --lib legacy_pointer_restore_io_and_commit_failures_preserve_the_document` | 补写盘失败断言后 1 通过 | `.autome/output/r11-io-final.txt` |
| `cargo test -p automed --test end_to_end legacy_pointer_e2e_restore_failure_keeps_todays_failure` | 补无自动修复事件、未启动审计断言后 1 通过 | `.autome/output/r11-failure-final.txt` |
| `cargo test -p automed --lib -- closed_pointer_ legacy_pointer_` | 最后补旧模板无范围/新模板无 legacy 键断言，库 9 个通过（不新增用例） | `.autome/output/r11-library-final.txt` |
| `git diff --check` | 退出 0 | 无输出 |

主验收 **14 个**，同过滤条件的轮前基线为 **4 个**，差 **+10**：automed 新增 7 个、旧模板已有用例更名后纳入本过滤 1 个、域新增 1 个、CLI 新增 1 个。设计原文列举的用例合计 10 而写 11，已按实测修正并纳入 2 个补强库用例及 CLI 检查。

最终验证集合 **81 个不同用例**，主验收与相关子集共执行 96 次（重复 15 次），全部通过，0 失败、0 跳过；单独复现与补强复跑不再增加总数。上一实现轮最终集合 **83 个**，差 **-2**：共同 5 个（handoff 集成触发标签 1、集成金样 4）；本轮不跑上一轮其余备注/域/交接/金样检查 78 个，新纳入指针/集成/守卫/版本/CLI 检查 76 个。因此是任务范围变化，不是删除测试或全库覆盖下降声明。用例名集合保存在 `.autome/output/r11-test-names.txt`。

自审发现的现有测试契约变更仅两处：旧模板缺标记从 None 改为 legacy payload；scheduler 的指针基线 wiring 从错误改为自动还原事件与原指针内容。既有 4 个 `closed_pointer_` 检测用例保留原断言，v8 集成金样不修改。

未跑 `sh scripts/ci-local.sh`、宿主 clippy/lint/typecheck 快检或全库测试，按本轮边界留给宿主。无真实账本、真实模型调用、跨仓库改动；本轮测试使用临时目录和隔离 Store，假 CLI 已由 settle 收尾。

