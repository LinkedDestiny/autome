# 审计 prompt 让审计轮「当场加强验收或补检查」，T-48 的范围守卫却把改了文档目录之外任何文件判成协议失败

## 现象

T-48（`5b504a0`）给审计、复盘、复核轮加了范围守卫 `guard_session_scope`（`crates/automed/src/scheduler.rs`）：会话提交相对会话开始的 diff 里只要有本任务文档目录之外的文件，就判协议失败，任务进入失败等人。

但审计轮的 prompt（`protocol/prompts/audit.md`）仍然写着「**验证缺口** → 当场加强验收或补检查，把临时的缺陷注入完全还原，立即复验」，并且没有任何一句告诉审计轮不能改文档目录之外的文件。两者互相矛盾：照 prompt 做的审计轮会被守卫判死。

已经发生的两类情况：

1. 守卫合并后的**第一个**审计轮就中了：2026-11-12 22:51，T-47 审计 #1 通过了 M-01，同时把 `crates/automed/src/config_io.rs` 里一个函数挪到测试模块之前以修 clippy `items_after_test_module`（提交 `6f75778`，纯移动、行为不变），任务随即「失败（审计）：协议失败：审计轮只能修改 docs/宿主门禁按机器排队-gate/ 下的文件，本轮改了：crates/automed/src/config_io.rs」。
2. 守卫之前的历史里，76 个审计提交有 2 个按 prompt 的「加强验收」改了测试文件：`e530595`（`apps/web/test/live-routes.test.ts`）、`8080b32`（`apps/web/test/gateway-http.test.ts`）。这种审计轮现在都会让任务失败。

宿主门禁关闭或只在合并前运行（T-61）时更容易发生：审计轮自己跑 fmt / clippy / 测试，看到失败会顺手修。

## 复现步骤

1. 需求任务的实现轮留下一处 clippy 报错（或一条放得过错误实现的验收）；
2. 审计轮按 prompt 当场修掉 / 补一条仓库里的测试并提交；
3. 任务进入「失败（审计）：协议失败：审计轮只能修改 … 下的文件」。

自动化复现：`crates/automed/tests/prompt_rendering.rs`（或 launcher 的金样测试）断言渲染出的审计、复盘、复核 prompt 里含有范围规则那句话——今天没有，所以失败。

## 期望行为

保留守卫（审计方改的代码没有人再审，违背 SAME-MODEL），让 prompt 与它一致：

- `protocol/prompts/audit.md`、`retro.md`、`fix-review.md` 明确写出：本轮**只能修改 `{doc_dir}/` 下的文件**；改了其它任何被跟踪的文件（产品代码、测试、格式化、lint 修复）Autome 会判协议失败。
- 审计 prompt 的三种结论相应改写：
  - 「验证缺口」的「当场加强验收或补检查」限定为两处：设计文档 `### M-xx 验收契约` 里的验收命令，以及 `.autome/output/` 下的临时检查（不进仓库）。需要进仓库的回归用例写进「## Backlog」或按实现缺陷退回，由实现轮补。
  - 项目测试体系（含格式、lint）不通过，按「实现缺陷」退回 `开放`，在审计结论里写明失败的命令与输出尾部；**不得自己修**。
- 守卫的报错文案末尾补一句怎么恢复：`autome rerun <任务> audit` 之前需要先处理越界的提交（保留或回退），并指向本轮会话开始的提交。
- 协议文本改了，按仓库规则处理协议版本与 `protocol eval` 用例。

## 实际行为

prompt 允许、守卫禁止；任务失败等人。

## 环境

main `782e261` 之后；`crates/automed/src/scheduler.rs` 的 `guard_session_scope`；`protocol/prompts/{audit,retro,fix-review}.md`；`crates/automed/tests/prompt_rendering.rs`。

## 线索（可选）

- T-48 的任务卡是按「审计、复盘、复核只写本任务文档」提的，当时没有核对审计 prompt 的「验证缺口」一条——矛盾出在任务卡，不在 T-48 的实现。
- 不在本任务范围、可以另提：守卫第一次命中时不直接失败，而是把分支退回会话开始的提交、带着「上一轮越界改了 X」的注入重派一次，第二次再失败。
