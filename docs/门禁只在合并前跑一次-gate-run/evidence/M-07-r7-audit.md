# M-07 r7 审计复验

负向对照（先跑，已还原）：
- 撤掉 bug-protocol.md 第 40 行新句：grep 退出 1；`protocol::` 67 个里 11 个失败（含 the_seed_passes_the_static_and_parse_layers、running_against_the_seed_on_disk_is_clean）；`protocol eval` 报「bug-protocol.md 少了「修复门禁只跑复现命令…」」。
- G-6 改成 `behavioral` 而 `eval: null`：`protocol eval` 报「G-6：behavioral / retire 改动必须指明 eval 用例」。
- 新 REQUIRED 的 why 长度由现有 every_requirement_carries_a_reason 守护（67 个通过含它；本轮 why 为 60+ 字节）。

验收命令（还原后）：两条 grep 命中；automed lib `protocol::` 67 通过；`protocol eval` 退出 0、无「失败」，必含/禁含短语 89 条（M-06 为 88，多 1）；prompt_rendering 20 通过；loop-protocol.md / session-protocol.md 相对 main 无差异。

结论：通过。
