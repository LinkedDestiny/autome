# M-02 独立复验（审计 #2，对应实现 #2）

## 验收命令
`cargo test -p automed --test end_to_end older_kernel && cargo test -p automed --lib kernel_mark`：e2e 7 个 + 库 3 个，共 10 个通过，0 失败。

## 负向对照（均已 `git checkout -q crates` 还原，工作区干净）
| 缺陷注入 | 结果 |
|---|---|
| 写命令不做比较也不 claim | 7 个全红（含第 2 号） |
| claim 的 Same 分支也写 updated_at | 第 6 号红 |
| 取锁出错时不带锁继续 | 第 7 号红 |
| 比较与 claim 挪到 tick_locked、排在 reclaim/recover 之后 | 第 1、4、7 号红 |
| 写命令拒绝改回 `Internal` 错误码 | 第 2 号红 |

## 自造检查
读 `kernel_guard`、`scheduler::tick`、`dispatch` 的实现：tick 先 check 后取锁再 claim，写命令同样；非读方法一律按写处理，覆盖未来新增方法。未发现缺口。

## 结论
通过。无实现缺陷、无验证缺口。
