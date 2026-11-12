# M-04 独立复验（审计 #4）· 通过

- 验收命令：`cargo test -p autome-domain && cargo test -p automed --lib && cargo test -p autome-cli && cargo test -p automed --test prompt_rendering` 全绿。
  用例数：domain `--list` 397（运行 396 + 1 个 ignored 的文档测试；上一提交同口径 393 运行，差 +3 与契约一致，契约写的基线 394 是含 ignored 的口径）；
  automed lib 684（基线 679 + 5）；autome-cli 32+14=46（基线 42 + 4）；prompt_rendering 16（不变）。`gate_timeout_` 过滤：domain 3、automed 5。
- 负向对照 1：在 `kernel_timeout` 里加「tail 含 124 即返回 Some」，`gate_timeout_exit_124_alone_is_not_kernel_timeout` 失败；已还原。
- 负向对照 2：把 `gate.rs` 终止行文案「运行超过」改成「运行超时」，`gate_timeout_real_poll_tail_matches_domain`（真实 `poll_as` 杀作业）失败，其余 4 个仍绿，说明交叉核对确是该用例独担；已还原。
- 代码对照设计 §9 九处文案：launcher 两种 Loop 注入、ReproNotFailing（要求 why=timed_out 且终止行）、scheduler 警报与 render_gate_line、cli gate.result / status 门禁行 / gate_same_failure / bug.repro_missed 均按 kernel_timeout 或 TIMEOUT_SUMMARY 分支；非超时原文保留；判据取 tail 最后一个非空行，不接受准备命令的终止行、前导 0 以外的非数字、0。无 serde 形状改动。
- 结论：产品行为符合设计，无缺陷无验证缺口。工作树已还原（git status 干净）。
