# 审计 #7 · 集成复验证据（M-05 / 实现 #8 门禁修复）

独立复验，不采信实现 #8 的说法。本轮固定协议为 v6 副本，无集成块；按 #6 先例做集成复验。

| 检查 | 结果 |
|---|---|
| `cargo run -p automed -- protocol eval protocol/` | 退出 0 |
| `cargo test -p automed --lib protocol::` | 70 通过 |
| 宿主失败用例 `a_code_sample_that_is_not_a_status_block_is_not_parsed` | 1 通过（原用例未改） |
| `cargo test -p automed --test prompt_rendering` | 40 通过（与基线相同） |
| M-05 grep 链（v 标题 8、`## 集成复验$`、技术设计） | 退出 0 |
| 实现 #8 的 diff（`protocol/` 仅 session-protocol.md 文案精简 + CHANGELOG J-3 一句） | 规则含义未变，未改测试/契约区 |
| 冲突标记扫描（非 docs 文件） | 无输出；`git diff --check` 退出 0 |

负向对照：向 `protocol/session-protocol.md` 追加 200 字节，`protocol eval` 报「1 项不通过」（大小预算），已 `git checkout` 还原，工作区无被跟踪文件改动。

结论：通过，里程碑表不变（全部 `已完成`）。构造的检查为一次性，丢弃。
