# M-08 r8 审计证据

- 负向对照（先做）：README 副本去掉「sessions do not run the full gate themselves」→ 循环打印「README 缺: …」退出 1；技术设计副本去掉「会话不再各自兜底」→ 打印「技术设计缺: 会话不再各自兜底」退出 1。验收命令拦得住错误实现。副本在 .autome/output/ 且已删，未改仓库文件。
- 真实验收：两组 grep 全部命中退出 0；`cargo fmt --all -- --check` 退出 0。
- 内容核对：相对 main 只改 README.md 与 docs/development/technical-design.md（纯文档，无代码变更）。README 写明每任务一次、合并前才暴露、多退回一次实现轮、会话不跑全量门禁、Bug 修复轮只跑复现命令、MergeGate 仍跑两者、MainCheck 不变；技术设计 §8.1 新段含 每个任务一次/合并前才暴露/多退回一次实现轮/会话不再各自兜底/只跑复现命令，且与 M-01..M-07 行为描述一致。
- 宿主门禁：本轮简报没有「## 宿主门禁」段（未取得该行），全量由宿主 `sh scripts/ci-local.sh` 在合并前门禁跑；本里程碑无代码改动，不影响。
- 人工项 H-01/H-02 已在设计文档「人工验收清单」，由用户合并前确认。
- 结论：通过。
