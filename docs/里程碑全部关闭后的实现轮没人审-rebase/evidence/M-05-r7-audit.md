# 审计 #6 · 集成复验证据（M-05 / rebase 后）

独立复验，不采信实现 #7 的说法。

| 检查 | 结果 |
|---|---|
| `cargo run -p automed -- protocol eval protocol/` | 退出 0 |
| `cargo test -p automed --test prompt_rendering` | 40 通过（与实现轮一致） |
| `cargo test -p automed --lib integration` | 34 通过 |
| `cargo test -p automed --test end_to_end integration_e2e` / `before_merge` | 4 / 11 通过 |
| 版本单测 `the_version_in_force…` | 1 通过 |
| `## protocol/v` 标题数 = 8；`^## 集成复验$` 1 处；技术设计含「集成复验」2 处 | 成立 |
| 逐提交冲突标记扫描（非文档文件，36 个提交） | 无输出 |

负向对照：把 `## 集成复验` 改为 `#### 集成复验`，grep 退出 1，验收链会失败；已还原。另试把 v8 标题改名，标题计数仍为 8（改名不改数量，计数门本身拦不住；实际由版本单测与 eval 承担，未再单独注入验证，已还原）。工作区无被跟踪文件改动。

结论：通过，里程碑表不变（全部 `已完成`）。构造的检查为一次性，丢弃。
