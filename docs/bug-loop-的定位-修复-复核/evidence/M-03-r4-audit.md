# M-03 r4 独立复验（审计 #3）

结论：通过。

## 负向对照（先于验收命令）
- `bug_attachment_name` 改成返回存储原值：launcher 3 个单测失败（list / render_only / keep_braces），prompt_rendering 4 个失败（3 份 inputs 金样 + 交叉用例）。还原后恢复。
- fix.md 的 before_merge 分支去掉 `{inputs}`：交叉用例 `bug_inputs_and_gate_modes_preserve_user_data_independently` 失败（金样不覆盖该分支，只有交叉用例拦得住）。还原后恢复。
- 两处改动均已还原，`git status` 干净。

## 验收命令（自跑）
- `git merge-base --is-ancestor main HEAD`：成功
- launcher 70、prompt_rendering 28、protocol:: 67、end_to_end bug_loop 18：全部通过
- `protocol eval protocol/`：8 通过 / 0 不通过 / 1 警告
- `cargo fmt --check`：通过；插件 evals 5/5
- 对 main 的金样差异：只新增 3 份 `*-inputs.md`，既有金样与需求 Loop 金样无改动；protocol 仅 CHANGELOG 与三模板。
- `cargo test --workspace`:唯一失败为与本任务无关的时序不稳定用例，见下方补记。

## cargo test --workspace 补记
全量结果：仅 `scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 失败，其余全过。单独重跑 3 次：2 败 1 过（断言位置不同，属时序不稳定）。该测试与 scheduler.rs 相对 main 均无改动，本任务只动 launcher / protocol / 测试文件，与它无因果关系；记 Backlog B-06，不退回里程碑。
