# 文档补齐：Bug Loop、任务类型与任务卡、待开始、reply、[bug]/[setup]/same_failure_limit
## 背景
K1/K2/T-27/T-28 已合并，但 `docs/development/technical-design.md` §5.1 只列需求节点，全文没有 Bug Loop、`kind`、`Draft`、`task start`、`task reply`、`[bug] fix_rounds`；README 只在 `[gate]` 一条里提到 `fix_gate`，`[setup]` 与 `same_failure_limit` 也要核对是否已写入。新用户和程序化建卡的 Agent 读不到这些。
## 要做什么
1. README：「命令行」一节补 `task new --kind bug --brief - --draft --json`、`task start`、`reply`、`extend`、`rerun`；配置一节补 `[bug] fix_rounds`、`[setup]`、`[gate] same_failure_limit`（核对 T-27/T-28 是否已写，缺则补）。
2. technical-design：§5 加 Bug Loop 的节点与转换（与 `bug_loop.rs` 对照，含复现命令退出码 126/127 不算失败、连续两次复现检查失败进入等你补充信息、合并前门禁总带复现命令），§8 加任务卡正文/附件/Draft，§14 更新。
3. 每处陈述给出代码位置，审计轮核对代码与文档一致。
## 不做什么
不改代码。
## 验收标准
`grep` 能在两份文档里找到上述关键词；审计轮逐条核对与代码一致；`sh scripts/ci-local.sh` 全绿（文档不影响）。
## 参考
`crates/autome-domain/src/bug_loop.rs`、`crates/automed/src/dispatch.rs`、`crates/autome-cli/src/main.rs` 的 usage 文本。
