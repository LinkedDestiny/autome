# M-01 r2 审计 #1 独立复验

结论：**通过**。（未读实现轮证据。）

## 验收命令（自己跑）
- `cargo test -p autome-domain`：388 通过 / 0 失败（基线 384 + D1–D4）。
- `cargo test -p automed --lib`：651 通过 / 0 失败（基线 647 + C1–C4）。
- 过滤 `merge_after`：domain 4、automed lib 4，与预期一致。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings` 干净。
- 宿主门禁 `sh scripts/ci-local.sh` 在 6f1d6105c4 上通过（简报引用，Desktop/Web 环境问题已不存在）。

## 负向对照（逐条注入、跑 `merge_after` 过滤、还原；`git status` 干净）
| 注入 | 结果 |
|---|---|
| `resolve()` 不拷贝 merge_after | D1 红 |
| `Failed` 判成 `Run` | D3 红 |
| 先判 verdict/无合并提交再判工作区 | D4 红 |
| 缺省超时 10→30 | C2 红 |
| 空白 `after` 仍被当命令（不判空） | C2 红 |
| `[merge]` 非表被静默接受 | C3 红 |
（过程中两次注入因正则写错未生效或编译失败，已修正后重跑，上表为有效结果。）

## 自造检查
读源码核对：`after` 非字符串报错先于判空；空白 after 不校验超时；超时按 `min(u32::MAX)` 截断；未配时 `merge_after = None`，零差异。无产品缺陷、无验证缺口。
