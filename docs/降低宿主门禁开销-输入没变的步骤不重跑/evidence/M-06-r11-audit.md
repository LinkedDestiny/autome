# 审计 #10 · 集成复验 · M-06 · 通过

范围：`a8ba28c`（唯一提交，仅改 `crates/automed/tests/end_to_end.rs`）。

1. 改动：仅把一处 `setup_fixture(...)` 调用拆成多行，与 rustfmt 要求一致；token 序列不变，无行为变化。无 rebase 丢失提交。
2. 测试前提：该文件 diff 只有 +5/-1 的换行，参数（命令串、`sleep 2`）逐字不变，断言未削弱。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' a8ba28c -- . ':(exclude)docs/...'` 为空。
4. 验收：负向对照——把调用改回单行，`cargo fmt --all -- --check` 退出 1（能拦住）；还原后退出 0。`cargo test -p automed --test end_to_end setup_` 18 例全绿。M-06 的 Web 验收与本改动无关（未触及 Web），不重跑。

临时注入已用 `git checkout` 还原，工作区干净。新造检查：fmt 负向对照一项。
