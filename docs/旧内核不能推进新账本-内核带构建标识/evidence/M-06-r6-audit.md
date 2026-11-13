# M-06 审计 #6 独立复验

结论：通过。

## 验收命令
`cargo build -p autome-cli` 成功；`install-alignment.test.ts` 2 例通过、0 跳过（darwin）；6 个 grep 均返回 0（输出 `.autome/output/m06-a.txt`）。

## 负向对照（均注入后还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| package-cli.sh 忽略 `--binary` | 2 例均失败 |
| `--out` 不生效 | 2 例均失败 |
| resolveAutomedBinary 不查 PATH | 第 2 例失败 |
| README 去掉 `launchctl bootout` | grep 返回 1 |
| 计划 §7.3 去掉 install-cli | grep 返回 1 |

「`autome core` 不存在」一项未单独注入：第 2 例用真实 `CoreSupervisor` 以 `autome core` 握手并核对 build_id，已由 PATH 对照证明测试走完整链路。

## 判别检查
测试 PATH 排除 cargo，故忽略 `--binary` 会立刻失败而非慢构建；未新造额外检查。
