# M-01 r1 审计复验

- 验收命令全跑：build + lib build_id 15 + build_rerun 4 + cli build_id 2 = 21 通过，0 失败 0 忽略。
- 负向对照 A（build.rs 不发仓库路径的 rerun-if-changed）：build_rerun 3 个失败（worktree、packed-ref、同秒父子排序），已还原。
- 负向对照 B（未知来源可当最新）：`unknown_is_older`、`override_only_older` 失败，已还原。
- 自造检查：当前 worktree 里 `automed --version` 与 `autome --version` 都是 `0001803-20261113T135844Z-1a77e020acf1`，等于 git 算出的 HEAD（count 1803、sha 1a77e020acf1）；临时空提交后增量构建标识变为 `0001804-…-4474e9153aa1`（跟上 HEAD、更大），随后 reset 还原。
- 工作区无遗留改动；未跑全量门禁。
