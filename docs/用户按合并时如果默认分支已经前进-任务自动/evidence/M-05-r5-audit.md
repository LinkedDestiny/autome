# M-05 审计 #5 独立复验

结论：通过。

- 验收命令全量：二进制计数 autome bin 13、cli 4、domain 299、automed lib 555、e2e 56、prompt_rendering 10、real_cli 7，全部通过；clippy=0、fmt=0、现有 e2e 删除行 0。
- 首次全量跑中 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 失败一次（gate.rs 本任务未改动，5 秒等待的时序抖动），用 `--no-fail-fast` 重跑全绿，555 通过。与本任务无关。
- 负向对照 1：`mergeable` 改回 `blocking.is_empty() && on_top` → E6 `the_merge_panel_offers_the_merge_when_only_main_has_moved` FAILED，旧用例仍过。能拦住，已还原。
- 负向对照 2：`event_line` 不认识 `merge.rebased`（匹配键改名）→ R1 FAILED。能拦住，已还原。
- 负向对照 3：改现有 e2e 断言会使 `git diff 2cca95b` 删除行计数非 0；实测为 0。
- 还原后 `git status` 无产品改动。
