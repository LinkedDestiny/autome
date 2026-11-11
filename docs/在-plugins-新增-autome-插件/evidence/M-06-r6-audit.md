# M-06 审计 #6 独立复验

- 验收 `sh plugins/autome/tests/verify-cli.sh readme`：V7–V10 共 4/4 通过；`run.sh all` 59/59（与 M-05 基线一致，无增减）。
- 负向对照（临时改 README 示例后跑 readme 组，逐个 `git checkout` 还原，工作区干净）：`id→task_id`、`state draft→pending`、`started false→true`、`kind→bug`、`branch→x/测试`、`queue_position null→1`、`archived false→true`，七种全部 V7 与 V10 失败（2/4）。值层面的篡改均被拒绝，验收命令能区分错误实现。
- 规范化范围仅 id/path/各时间戳/doc_dir，符合设计；未发现产品缺陷或验证缺口。
- 结论：通过。
