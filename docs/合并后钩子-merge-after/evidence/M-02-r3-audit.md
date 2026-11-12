# M-02 r3 审计复验

- 验收命令原样执行，结果与预期数字一致（106 / 655 / 26+13；过滤 3 / 8）。
- 负向对照（均失败后已还原）：
  - AUTOME_MERGE_COMMIT 改成固定假值 → E1 失败
  - 钩子 cwd 改为 /tmp → E1 失败
  - 超时写死 30 → L1 失败
  - 去掉 started 补写 → L4、L2 失败
  - 作业存在也重启 → L2、L1、L4 失败
  - 去掉 concluded 防重入 → L2 失败
- 结论：通过。判别脚本 `.autome/output/mut.sh`。
