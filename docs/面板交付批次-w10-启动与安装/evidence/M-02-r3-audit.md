# M-02 · 实现轮 r3 · 审计 #3 独立复验

结论：通过。

## 验收命令（自己跑）
- `cargo test -p autome-cli --test cli web_`：7 条通过。
- `cargo test -p autome-cli --bin autome usage`：4 条通过，含 `usage_lists_web`。
- vitest 三文件：24 条通过，skipped 0（login-path 13 + login-path-real 2 + web-service 9；契约写 22，web-service 实有 9 条，多出的 2 条是补充用例）。ps EPERM 在本轮环境里未复现，login-path-real 的真实服务用例通过。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| web::run 开头创建 AUTOME_HOME | `web_does_not_open_ledger` 失败 |
| 转发时丢掉 `-x` | `web_forwards_arguments` 失败 |
| main.ts 不再把 `corePathEnv()` 传给 kernelEnv | login-path-real 第 1 条失败 |
| service install 去掉 bootout | web-service「重复 install…先 bootout 再 bootstrap」失败 |

## 新造检查
仅上述缺陷注入，不留存。无实现缺陷、无验证缺口。
