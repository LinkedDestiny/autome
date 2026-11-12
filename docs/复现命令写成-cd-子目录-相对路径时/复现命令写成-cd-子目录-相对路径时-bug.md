# cd 子目录后的复现脚本漏入回归保护集

status: 已定位
fix-round: 0/3
复现命令: cd crates/automed && cargo test --offline --test end_to_end bug_cd_repro_ -- --nocapture
根因: regression_protection 仅在命令原文匹配仓库相对路径，未解析 cd 改变的目录；定位阶段无条件保存空保护快照，直到修复阶段才检查为空。
复核: 待复核

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

定位轮未修改产品代码。仅新增回归测试及本文，待后续修复轮处理。

## 复核意见

待独立复核。
