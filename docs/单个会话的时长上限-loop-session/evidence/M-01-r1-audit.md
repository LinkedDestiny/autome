# M-01 r1 审计复验

结论：通过。

## 验收命令（自己跑）
- `cargo test -p autome-domain session_minutes`：3 通过（基线 0，新增 3）。
- `cargo test -p automed --lib session_minutes`：5 通过（基线 0，新增 5）。
- `grep -c session_minutes README.md`：1（≥1），内容为 `[loop]` 样例 + 一行 `session_minutes = 120`。
- 用例总数 8，与契约一致。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| validate 改成 `== 0` | domain 用例失败 |
| resolve 忽略项目覆盖 | domain 用例失败 |
| save_global 不写 session_minutes | automed lib 失败 |
| 自造：config_set_loop 全局支丢掉 session_minutes | 失败 |
| 自造：config_set_loop 项目支丢掉 session_minutes | 失败 |

## 自造检查
读 diff 核对：旧 JSON 缺字段回落 120（用例含）、u32::MAX 无上界、`4`/`1`/`0` 均被拒、负数 TOML 值 clamp 为 0 后被 validate 拒绝。未发现产品缺陷。
