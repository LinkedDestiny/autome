# M-02 独立复验（审计 #2，对应实现轮 r3）

## 验收命令（自跑）
- `cargo build -p automed`、`npm ci`、四个指定文件：74/74 通过（本沙箱可监听，真实内核用例实际执行）。
- lint 0 警告、typecheck 通过；全量 `npm test`：34 文件、318/318 通过 = 基线 301 + 17，与契约一致。
- 宿主门禁 ci-local（提交 4a6392a）通过，仅作旁证。

## 负向对照（逐项注入，均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 白名单去掉 `project.probe` | 5 个用例红（含 A-03 真实内核用例） |
| `isHomeRelativePath` 恒真 | 4 个用例红（含 `/etc`、`~/../etc` 拒绝） |
| 路径例外按值（`~` 开头）而非按「方法+键」 | 2 个用例红 |
| 白名单去掉 `protocol.get` | 反向差集 2 个用例红 |
| 去掉有意不放行的 `env.detect` | 3 个用例红 |
| **真实内核注入**：`dispatch.rs` 的 `READ_METHODS` 加 `task.fake_read`（N 同步 18） | read-gate 2 个用例红，差集显示 `task.fake_read`（A-04 直接取证）；还原后全绿 |

## 结论
通过。无实现缺陷、无验证缺口。新造检查（上表注入）为一次性，不入库。
