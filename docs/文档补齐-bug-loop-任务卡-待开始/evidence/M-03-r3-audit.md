# M-03 审计复验（实现 #3 对应）

结论：通过。

- 验收命令：`M-03 PASS 155/155`，退出码 0。
- 全量门禁 `sh scripts/ci-local.sh`：9 步全部 ok，`ci-local exit 0`（用时 605s）。此前实现轮的 Electron SIGABRT / 监听 EPERM 阻塞本轮未复现。
- 用例总数：cargo 合计 1200（25+14+384+648+106+16+7），web 301。相对 merge-base，`crates/ apps/ scripts/` 零改动（git diff --stat 为空），仅 README 与 technical-design 追加共 115 行、删除 0，故用例总数与基线相同。
- 负向对照（均已还原，工作区干净）：
  - 把 `same_failure_limit = 3` 改成 4 → `删除或改写了 1 行` + `缺 same_failure_limit = 3`，FAIL 153/155。
  - 命令行区间把 `autome rerun` 改拼 → `缺 autome rerun`，FAIL 154/155。
  - 新增块前加 `# hi` 行 → `多出的标题行`，FAIL 154/155。
- 逐行核对 R-01..R-14：README 新增文字与锚点处代码语义一致；R-04 标题回退（无 `# ` 时取第一非空行并去掉前导 `#`/空白）对照 dispatch.rs `task_create` 一致；R-13/R-14 与 bug_loop.rs / scheduler.rs 一致。V-01..V-05 的已有句子未改。
- 人工项 H-01 已在清单，不阻塞。
