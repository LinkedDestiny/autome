# M-05 r7 审计复验

结论：通过。

## 验收命令（自己跑）
- config_io::tests::gate_quick_session 3 通过；launcher::tests::round_gate_line 3 通过；prompt_rendering 49（M-04 后基线 48 + 1）；end_to_end gate_quick_round_start 2、gate_deadline 1 通过；ci-quick.test.mjs 18 通过（15 + 2 + 共享输入回归 1）。

## 负向对照（均已还原，git status 干净）
- `{round_start}` 校验改成「多于 1 个才报错」→ `gate_quick_session_exactly_one_slot` 红。
- Bug FixGate 的截止时间按快检超时算 → `gate_deadline_uses_feature_quick_and_bug_full_timeout` 红。
- 简报里的起点被改成截断的提交号（与 `session.started.start_heads` 不一致）→ `gate_quick_round_start` 两条均红。
- 起点取在简报提交之后：读 `scheduler.rs` 确认 `start_heads` 在 `write_brief` 之前取（简报提交在后），验-4 用例比对三处同值。

## 自造判别检查
- 分别改 `fixtures/gate-history-t98.txt`、`merge_queue_cases.inc`、`fake_harness_ok.sh`，对真实仓库跑 `ci-quick.sh --plan`：候选只有 lib、build_rerun、prompt_rendering、real_cli，无 `--tests`，并输出「未跑 … end_to_end（共享测试输入改动，留给合并前门禁）」。三次均如此。

## 人工验收项
无。全量门禁由宿主合并前运行，本轮未跑。
