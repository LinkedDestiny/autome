# M-16 实现证据 · 实现 #17

## 范围与修复

对应设计「I 协议副本完整性警告」、I-E1–I-E3、I-T1、X-3。上一轮审计 M-15 通过，无退回检查需要搬入。

- `ProtocolFiles::task_copy()` 统一定义副本文件集合；版本身份与写盘都使用它，保留 `contract.toml`，排除 `evals/`，完整 seed 仍保留供 eval 使用。
- 新冻结任务记录一次 `protocol.hash_scheme`；可验证旧 ref 使用 seed 的可信副本哈希迁移，不吞掉升级前真实编辑；不可验证旧 ref 保留 tag、按现有副本重建基线，事件明确 `verified: false` 与证据限制。
- 基线与迁移事件在同一 SQLite 事务内发布；只比较最后一条完整性告警的 `actual_hash`，重复会话不重复报，新的改动继续报。未改副本的会话 ref 与任务一致，改过副本的会话按实际内容归因。
- 没改协议正文、固定副本、schema 版本、其它里程碑状态；`protocol.versions` / `protocol.get` 沿用 current/ref 的同一口径。
- 既有协议冻结 e2e 的版本断言从 v8 更新为当前 v9：本任务 M-04 已更新 seed，原断言在本轮相关检查中失败；没有弱化哈希一致性断言。

## 先红后绿与提交

测试独立提交：`6fdc07e5`（不含产品修复），提交前 `cargo fmt --all -- --check` 退出 0，输出 `.autome/output/m16-fmt-check.txt`。

| 命令 | 修复前 | 修复后 | 原始输出 |
|---|---|---|---|
| `cargo test -p automed -- protocol_integrity_ the_version_in_force` | 库 2 个：1 通过、1 失败；Cargo 因库失败未继续 e2e | 8 个通过，0 失败、0 忽略 | `.autome/output/m16-red.txt`、`m16-green.txt` |
| `cargo test -p automed --test end_to_end protocol_integrity_` | 6 个失败，0 忽略 | 6 个通过，0 失败、0 忽略 | `.autome/output/m16-e2e-red.txt`、`m16-e2e-green.txt` |

红灯判据：副本 hash 与 current 不一致；未编辑仍有告警；改动后的两会话累计 5 条而非 1 条（含之前 3 条误报）；可验证旧副本已编辑时两会话报 2 条而非 1 条；其它旧任务仍误报。夹具调试期间修正了 rerun 的 stopped 前置状态、旧账本 ref 的直接写入，以上红灯输出来自修正后、产品修复前版本，均为行为断言失败。

## 标待审前自审清单

1. **情形表逐行**：
   - 新冻结未编辑：`…untouched_copy_records_no_warning`，4 次真实会话无告警，全部会话 ref 等于任务；scheme 只一条、reason=frozen。
   - 新冻结后编辑：`…edited_copy_warns_exactly_once_over_two_sessions`，后续两会话只一条告警、两会话均归因到实际 hash。
   - 可验证旧 ref 未编辑：`…legacy_ref_is_migrated_once_without_warning`，迁移一次、verified=true、previous 保留、无告警。
   - 可验证旧 ref 已编辑：`…legacy_ref_with_an_edited_copy_still_warns`，可信预期基线保持为 current，编辑仍告警一次。
   - 不可验证旧 ref：`…unverifiable_legacy_ref_rebuilds_the_baseline_and_says_so`，保留旧 tag、基线等于现有副本、verified=false、限制文字存在、无误报。
   - 迁移后再改：`…edit_after_migration_still_warns`，空文件新增后告警一次，再删除原文件后累计两条；重复会话不加告警，scheme 不增加。
2. **失败分支**：可信旧基线不匹配与无法验证旧基线分别由上两类用例执行，均通过。拒绝授权、超时、外部服务不可达不适用：本设计没有新增这些分支；读盘错误沿用原有返回路径，本里程碑未点名新增处理。
3. **全部迁移**：无基线→frozen；无 scheme 的可信旧 ref→verified 迁移（相同/不同副本）；不可信旧 ref→unverified 迁移；已有 scheme→相同 hash 无告警；不同 hash→首次告警；重复不同 hash→去重；再次改变→新告警。上述六个 e2e 全部执行。迁移不改写副本文件。
4. **输入域边界**：库用例覆盖空集合、空内容 contract、`evals/` 精确前缀排除和 `evals-other.md` 保留；e2e 覆盖正文修改、空文件新增、文件删除、历史 tag 与当前 tag、连续两会话。日期/数值最大值不适用：这里是文件集合与内容 hash，没有日期或数值输入域。

## 相关子集、基线与限制

- `cargo test -p autome-domain protocol::`：22 个通过（`.autome/output/m16-domain.txt`）。
- `cargo test -p automed --lib protocol::`：72 个，64 通过、8 失败（`.autome/output/m16-protocol.txt`）。将本轮产品修改临时还原至测试提交后，同命令 72 个、63 通过、9 失败：多出的一个失败是本轮新增 hash 回归，另外 8 个完全相同（`.autome/output/m16-protocol-baseline.txt`）。产品修改随后完整恢复。
- 上述 8 个既有失败均源于 D-1 引用 `evals/audit-checks-operator-notes/` 缺失；未改协议以绕过检查，记录为合并前既有阻塞，不属于 M-16 验收失败。
- `cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：原断言要求 v8，而实际 v9；更新版本断言后复跑。输出 `.autome/output/m16-existing.txt`、`m16-existing-green.txt`。
- 本轮验收 **8 个**，相比上一轮 M-15 验收基线 **18 个，差 -10**，原因是换为本里程碑过滤范围，不能视为删测试。同一 M-16 范围的原基线只有既有版本身份用例 1 个，本轮新增库 1 + e2e 6，**净增 7**。
- 合并本轮相关子集并去掉重复执行，独立用例 **101 个**（automed 协议库 72 + 新 e2e 6 + 既有冻结 e2e 1 + domain 22）；同范围修改前基线 94，净增 7。相对上一轮的 18 个则差 +83，来自范围变化与新增用例，不作全量覆盖比较。
- 未执行 `scripts/ci-local.sh`、宿主快检组合、全项目测试或付费行为 eval；未访问真实账本。所有 e2e 使用项目原有临时目录、临时账本与真实包装脚本，完成后收割会话。

最终核对：既有冻结 e2e 更新后 1/1 通过；相关子集合计 101 个独立用例，93 通过、8 个已验证为既有失败、0 忽略。最终格式检查 `cargo fmt --all -- --check` 与 `git diff --check` 均退出 0。M-16 验收 8/8，四项自审已执行，标记待审，状态 `implementation-round: 17/80`。本轮产品与文档提交到当前分支，不推送、不启动下一会话。
