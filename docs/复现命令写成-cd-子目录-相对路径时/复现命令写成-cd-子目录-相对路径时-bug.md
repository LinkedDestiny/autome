# cd 子目录后的复现脚本漏入回归保护集

status: 已定位
fix-round: 2/3
复现命令: cd crates/automed && cargo test --offline --test end_to_end bug_cd_repro_ -- --nocapture
根因: regression_protection 仅在命令原文匹配仓库相对路径，未解析 cd 改变的目录；定位阶段无条件保存空保护快照，直到修复阶段才检查为空。
复核: 通过

## 现象

在定位基线 `06bc187cb1d75fed6ef6570ccdb93190fe57b729` 上，新脚本 `apps/web/scripts/x-regression.sh` 未带测试命名标记。使用 `cd apps/web && sh scripts/x-regression.sh` 或 `( cd apps/web && sh scripts/x-regression.sh )` 后，实际复现命令正常失败，定位却写入 `files: []`。夹具的修复轮仅修改产品标记，随后任务失败于 `Fix`，错误为「回归测试保护集为空：定位轮必须新增或修改测试文件」。

把同一脚本的调用改为 `sh apps/web/scripts/x-regression.sh`，则保护类型为 `file`，产品修复后可正常走到 `await_merge`。因此不是脚本不可执行、命令未找到或复现失败造成的错误。

## 复现

从当前仓库根目录执行状态块中的命令。此项目为单 Git 仓库中的 Cargo workspace，`crates/automed` 是 Cargo 包目录，不是 Autome 多仓库项目成员，因此无需虚构 `repos` 名称。

新增回归位于 `crates/automed/tests/end_to_end.rs`，统一过滤前缀为 `bug_cd_repro_`：

| 用例后缀 | 期望 | 当前结果 |
| --- | --- | --- |
| `root_relative_control` | 完整路径保护为 `file`，产品修复后到 `await_merge`，脚本未改 | 通过 |
| `relative_script_is_protected` | `cd` 后的路径对应唯一受保护脚本，产品修复正常通过 | 失败，快照文件数为 0 而非 1 |
| `parenthesized_relative_script_is_protected` | 括号内的 `cd` 同样识别 | 失败，快照文件数为 0 而非 1 |
| `empty_protection_fails_at_locate_without_spending_fix_round` | 无任何合格测试时在 `Locate` 协议失败，说明定位责任、test/spec 规则和复现脚本路径；不写空快照、不启动修复轮 | 失败，实际在 `Fix` 协议失败 |
| `legacy_empty_snapshot_still_fails_at_fix` | 注入旧版本空快照，保持修复阶段原有错误文案 | 通过 |

夹具通过真实临时 Git 仓库、调度器和 shell 会话跑定位、复现、产品标记修复及复核；不访问网络，不依赖 Node，不使用固定 sleep。使用 `.sh` 是为了剔除 Node 环境依赖，它与任务卡 `.mjs` 走同一 `script` 识别分支；脚本相对于自身路径读取产品标记，切换目录不影响脚本功能。两个路径用例和独立空保护用例分别限制两处根因，避免仅修路径识别便误以为完成全部需求。

离线执行得到退出码 101：5 个用例中 2 通过、3 因上述业务断言失败，无忽略用例。最终测试的重复执行记录保存在 `.autome/output/cd-repro-run2.log`、`.autome/output/cd-repro-run3.log`；首轮构建记录在 `.autome/output/cd-repro-run1.log`。未运行全量门禁，本轮有意保留失败测试。

## 根因分析

1. `crates/automed/src/scheduler.rs:1306` 的 `regression_protection` 将文件分为命名测试、内联测试和复现脚本。`apps/web/scripts/x-regression.sh` 不符合目录或文件名测试规则，也没有内联测试段，只能走脚本分支。
2. `scheduler.rs:1342` 直接执行 `command.match_indices(path)`。仓库路径为 `apps/web/scripts/x-regression.sh`，命令中却只有 `cd apps/web` 和 `scripts/x-regression.sh`，所以无匹配；词边界判断尚未执行，括号也不会修复路径缺失。
3. `scheduler.rs:1470` 定位分支无条件追加 `bug.regression_snapshot`，没有拒绝 `files` 为空。`scheduler.rs:1480` 才在修复分支检查空集合，导致无效定位产物先进入修复、消耗轮次，再将协议错误归到修复。
4. `scheduler.rs:1157` 已将保护守卫错误转为 `BugSessionOutcome::GuardFailed`；`crates/autome-domain/src/bug_loop.rs:204` 会在当前节点按协议失败处理。因此早期拒绝可复用现有定位产物不合格路径，无需新增流程。本轮测试选择现有 `Locate` 失败路径。
5. 既有 `bug_regression_guard_rejects_modified_or_deleted_tests` 中的空保护夹具明确期待修复阶段失败。修复轮需把该既有夹具调整为新定位契约，并保留本轮新增的旧空快照兼容测试；否则旧断言与任务卡期望冲突。常规 tests 目录用例走命名分支，未覆盖这次的路径条件。

## 修复说明

第 2/3 轮修复：

- 单跑 automed Clippy 重现 `scheduler.rs` 的 `collapsible_if`；合并子 shell 目录恢复的条件判断，不改变解析语义。
- Bug 流程子集修复前 39 通过、2 失败：空保护集提前拒绝，覆盖了不可执行命令的复现未命中处理，也使第二次未修改脚本的定位无法累计复现未命中次数。
- 空保护集只记录所属定位会话，不保存空快照；宿主命令有效复现后才将其判为 `Locate` 协议失败，仍不启动修复、不消耗修复轮次。命令成功、不可执行或超时继续走原有复现未命中流程；按会话 ID 匹配，避免旧空集记录污染新定位。旧空快照在修复阶段的拒绝保持不变。
- 本轮未修改任何测试。原复现命令通过（5 通过、0 失败、0 忽略）；automed Clippy（`cargo clippy --offline -p automed --all-targets -- -D warnings`）通过。日志位于 `.autome/output/fix2-repro.log`、`.autome/output/fix2-clippy-final.log`；修复前日志为 `fix2-clippy-before.log`、`fix2-bug-before.log`。
- 直接相关子集 `cargo test --offline -p automed --test end_to_end bug_ -- --nocapture` 41 通过、0 失败、0 忽略，包含修复前失败的两个复现未命中用例及全部回归保护用例；`cargo test --offline -p automed --lib repro_` 5 通过、0 失败、0 忽略。日志：`.autome/output/fix2-bug-after.log`、`.autome/output/fix2-unit.log`。`cargo fmt --all -- --check` 与 `git diff --check` 通过。
- 未运行全量门禁 `sh scripts/ci-local.sh`，由宿主在合并前统一执行；本轮等第 2/3 轮复核：通过。
- 复现命令独立重跑：`bug_cd_repro_` 5 个用例通过；`cargo test --offline --test end_to_end bug_` 41 通过、0 失败、0 忽略；`--lib repro_` 5 通过。未跑全量门禁。
- 提交 159410e 仅改 `scheduler.rs`，未触碰任何测试：定位阶段不再无条件拒绝，改为记录带会话 ID 的 `bug.regression_empty`，待宿主确认真实复现（非 0、非 126/127、未超时）后才判 `Locate` 协议失败；命令成功/不可执行/超时仍走原复现未命中流程，修复了上轮拒绝过早破坏该流程的两个用例。空集仍不启动修复、不耗修复轮次。
- 根因仍成立，回归测试未删除/跳过/弱化，改动范围最小（另含 clippy 的 collapsible_if 等价改写）。

第 1/3 轮修复：

- `scheduler.rs` 增加字面量复现路径解析，按 `cd` 累积工作目录，识别引号、相对路径及括号子 shell 的目录作用域；保留原有完整路径匹配。此处仅静态识别，不执行 shell 或展开变量。
- 定位阶段在保存快照前拒绝空保护集，错误说明定位责任、test/spec 命名与复现脚本路径；保留修复阶段对旧空快照的兼容拒绝。
- 仅更新既有空保护夹具的失败节点断言为 `Locate`；定位轮新增的 5 个回归用例未修改、删除、跳过或弱化。另增路径解析单测，覆盖连续 cd、引号、父目录、子 shell 恢复及路径不匹配。
- 原复现命令修复前退出码 101（2 通过、3 失败），修复后退出码 0（5 通过、0 忽略）。路径解析单测 1 通过。日志：`.autome/output/fix1-before.log`、`.autome/output/fix1-after-final.log`、`.autome/output/fix1-unit.log`。
- 直接相关测试 `cargo test --offline -p automed --test end_to_end bug_regression_guard_` 全部通过（8 通过、0 忽略），覆盖修改/删除/重命名/模式变更拒绝、内联及追加测试保护、多仓库、重试与旧快照兼容。日志：`.autome/output/fix1-guards.log`。`cargo fmt --all` 和 `git diff --check` 通过。
- 未运行全量门禁 `sh scripts/ci-local.sh`，由宿主在合并前执行；等独立复核（第 1/3 轮）：通过。

- 复现命令独立重跑：5 个 `bug_cd_repro_` 用例全部通过；`bug_regression_guard_` 8 个通过；automed 库内单测 762 通过（含新增路径解析单测）。未跑全量门禁。
- 根因成立：路径只按命令原文匹配，且定位阶段不拒绝空保护集；两处在 `scheduler.rs` 均有对应修复，原完整路径匹配保留，不会收窄既有行为。
- 回归测试：定位提交的 5 个用例在修复提交中未被改动；仅把既有空保护夹具的失败节点断言由 Fix 改为 Locate，与新契约一致，且旧空快照在修复阶段的拒绝由新用例 `legacy_empty_snapshot_still_fails_at_fix` 保住，不算弱化。
- 范围：仅改 `scheduler.rs`（静态 cd 路径解析 + 定位空集拒绝）与测试，无无关改动。解析器为静态字面量识别，不执行 shell，`cd` 参数含变量或多参数时忽略，保守不误保护。

## 复核意见

待独立复核。
