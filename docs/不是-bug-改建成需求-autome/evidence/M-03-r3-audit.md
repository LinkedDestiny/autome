# M-03 · 审计复验

## 负向对照（临时注入，均已还原，git status 干净）
| 错误实现 | 结果 |
|---|---|
| validateOpSpecific 的 task.convert 分支失效 | 新用例红（35/36） |
| kind 校验放宽为只要求存在 | 新用例红 |
| 白名单去掉 task.convert | 新用例红 |
| start 不校验布尔 | 新用例红 |
| `specific` 里不登记 task.convert | 「全部 op 正路径可达」红 |

## 验收命令（本轮自己跑）
- `npm ci --prefix apps/web` 退出 0。
- `write-gate.test.ts`：36 通过（基线 35 + 1）。
- `npm test --prefix apps/web`：33 个文件、302 通过、0 失败。
- `cargo test --workspace`：11 个目标，1206 通过 / 0 失败 / 1 ignored，等于基线 1193 + 13。输出在 `.autome/output/a3-cargo.txt`。
- 宿主门禁（b27e4c6）`ci-local.sh` 退出码 0，作为旁证。

## 结论
通过。无实现缺陷，无验证缺口。实现轮所述「全量受阻」是其沙箱禁止本地监听所致，本环境不受限，全量已补齐。H-01 仍留人工验收清单。
