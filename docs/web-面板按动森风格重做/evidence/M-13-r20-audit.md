# M-13 r20 审计复验

结论：通过。

- 验收命令（overview / overview-summary / overview-states-real / labels）：4 文件 54 用例全绿。
- 负向对照 1：把 `reviewDocuments.length > 0 &&` 改成 `false &&`（删文档入口及缺失提示）→ overview.test.ts 5 个用例失败。已还原。
- 负向对照 2：仅去掉缺失提示的 span → 4 个用例失败（含 review/adjudication 四种存在组合）。已还原，工作区 apps/ 无差异。
- 独立核对源码：入口由 `${slug}-review.md` / `-adjudication.md` 在受控文档清单中查得，链接为 `?tab=docs&doc=<name>`，经 loadTaskDetail 读取；用例覆盖绝对路径、`../`、空名被拒，页面不含 /private/ 路径，不出现「提了/采纳」编造数字；非批准等待态不显示入口。
- 新造检查：无额外，原有用例已区分两种错误实现。
- 宿主快检（fmt/clippy/lint/typecheck）在 195db547 通过。
- 无人工验收项。
