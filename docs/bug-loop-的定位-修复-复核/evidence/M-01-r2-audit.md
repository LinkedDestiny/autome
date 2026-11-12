# M-01 r2 审计证据（独立复验）

## 验收命令（自己跑）
- launcher 62 / prompt_rendering 19 / protocol:: 61 全部通过；静态门 8 项通过、0 不通过、CHANGELOG 8 条；
  `git diff --name-status 83c80e9` 恰好 7 行（3×A 金样，4×M：CHANGELOG 与三个模板）。
- `cargo test --workspace` 全绿（652/384/101/25/19/13/7 …，0 失败）。

## 负向对照（逐条注入、复验、还原；还原后 git status 干净）
| 注入 | 结果 |
|---|---|
| Bug 侧附件用存储原值 | 单测 1、3、4 失败；三份新金样失败 |
| 整理轮也取文件名 | 单测 5 失败 |
| fix.md 的 `{inputs}` 独占一行 | `fix` 既有金样 + `fix-inputs` 金样失败 |
| 先替换 `{inputs}` 再检查占位符（全模板替换） | 单测 4 失败（首次注入写成空操作，已更正后复测） |
| CHANGELOG 去掉 v3 | 版本断言失败 |
验收命令能拦住上述错误实现。测试取值用绝对路径（含空格），与真实入口对齐。

## 结论
通过。门禁 ci-local 中桌面（Electron SIGABRT）/Web 环境阻塞属 M-02 范围，不影响 M-01。
