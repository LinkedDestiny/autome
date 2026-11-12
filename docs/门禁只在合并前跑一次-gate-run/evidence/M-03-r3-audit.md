# M-03 r3 审计证据 · 审计 #3

结论：通过。

## 验收命令（自己跑）
- `bug_loop` e2e 16 通过（基线 14 + 2）；`scheduler` lib 91 通过；`gate::` 12 通过——与契约预期一致。
- 全量 `cargo test --workspace` 全绿（lib 660、e2e 118、domain 390 等），无失败。

## 负向对照（逐个注入、跑 `before_merge` 过滤的 lib 与 `bug_loop_before_merge` e2e、git checkout 还原）
| 注入 | 结果 |
|---|---|
| M1 FixGate 命令不看 `gate_run`（照旧拼两者） | lib 2 例失败、e2e ① 失败 |
| M2 结果命令不看 `.repro-only` | lib 3 例失败（record_repro / reuse / crash_window） |
| M3 `.repro-only` 对所有 FixGate 都写 | lib 2 例失败（含 legacy 记账用例、reuse） |
| M4 `Job::clear` 清单加 `.repro-only` | gate 单测 + lib 3 例失败 |

四组均被拦住；还原后 `git status` 干净。M2–M4 在 e2e 层不红，由契约点名的 scheduler/gate 单测负责，符合契约分工。

## 自造检查
即上表四组注入；未发现产品缺陷、验证缺口。
