# M-02 r2 独立复验

- 验收命令：automed lib 554 通过、end_to_end 52 通过；`cargo test --workspace` 0 失败（domain 299、cli 4、prompt 10、real_cli 7、bin 12）。
- 负向对照（均临时改源码、跑测试、还原；`git status` 干净）：
  - 阶段一禁用且阶段二落后一律 Blocked（现状）→ S1、S2、S5、E1 失败；
  - 阶段一把任何被拒都当落后 → S4、S7 及现有脏工作树用例失败；
  - 阶段一不记录非落后拒绝 → S7 失败；阶段二不记录非落后拒绝 → S4、S7 及现有脏工作树用例失败；
  - `Behind` 直接进 `Active{Rebase}` 不经 `Requeued` → S1、S2、S5 失败。
- 结论：对照全被拦住，验收有判别力；无产品缺陷。E2 在 Behind 变异下未单跑，由 S1/S2/E1 覆盖同路径。
