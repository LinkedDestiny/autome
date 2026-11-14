# M-01 · 实现 #1 · 审计复验

## 验收命令（自己跑）
- `node --test scripts/test/ci-summary.test.mjs`：7/7 通过。
- `cargo test -p autome-domain --test gate_failure_window`：6 通过（基线 5，+1）。
- `npx vitest run test/ci-config.test.ts`：9 通过（基线 8，+1）。
- `cargo fmt --all -- --check`：退出 0。快检（clippy、web lint/typecheck）宿主已在 68bb43ea 通过。

## 负向对照（逐条注入、跑、`git checkout` 还原；最终 `git status` 干净）
| 注入 | 结果 |
|---|---|
| summary.mjs 的 clip 改按字符数 | 用例 ⑦ 失败 ✔ |
| summary.mjs 的块预算改按字符数 | 用例 ⑦ 失败 ✔ |
| ci-summary 用例 ① 的 cargo 命令去掉 `--no-fail-fast` | 用例 ① 失败 ✔ |
| ci-local.sh / ci.yml 去掉 `--no-fail-fast` | `ci-config` 新增用例失败 ✔（ci-summary 不管这一条，由 ci-config 守） |
| gate_failure_window 新用例里用例行改成 `FAILED  …` 开头 | 新用例失败 ✔ |
| gate_failure_window 新用例去掉 `==> 汇总` | 新用例失败 ✔ |
| lib.sh 把 `==> 汇总` 改名 | ci-summary 7 个全红 ✔ |

## 自造判别检查
对照设计 A.2/A.3 复核 summary.mjs：多步骤失败共用 2400 字节/20 行全局预算，「另有 N 个」N = total − listed；`FAILED  <名>` 与 `ok  <名>` 行格式逐字不变。未发现与设计不符的产品行为。已知小处（不构成缺陷，不进验收）：cargo 的 `Doc-tests` 段没有 `Running` 行，目标名会沿用上一个目标。
