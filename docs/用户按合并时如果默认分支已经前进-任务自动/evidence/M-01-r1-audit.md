# M-01 r1 独立复验

- 验收命令：domain 299 通过、automed lib 548 通过；全工作区 `cargo test --workspace` 0 失败（e2e 50）。
- 负向对照（均临时改源码、跑测试、还原；`git status` 干净）：
  - 授权位在 `(Rebase,Ok)` / `(MergeGate,Passed)` 不生效 → domain 3 例失败；
  - `Behind` 改回进 `AwaitMerge` → D1 失败；
  - 延续规则不看当前状态（`in_merge_stretch` 去掉）→ D9、D8 失败；
  - `check_merge` 先查落后再查脏 → G1、G2 失败；
  - `git merge` 失败标成 `Behind` → G3 失败。
- 结论：五条对照全被拦住，验收有判别力；无产品缺陷。
