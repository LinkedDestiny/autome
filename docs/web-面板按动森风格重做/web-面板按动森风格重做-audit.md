# 审计结论 · M-11 · 实现缺陷（截图未替换）

领域：README截图交付；reopen 1。

- 验收命令第一段失败：`docs/images/` 只有旧三张，缺 `loop.png`、`skills.png`；readme-images 3 条失败（ENOENT）。违反 G-3 与设计 G-Q3。
- 第二段 `e2e/layout.spec.ts` 14 条全过（无缺陷）。
- 审计已在本机跑通截图脚本：`npm run screenshots --prefix apps/web -- --out .autome/output/screens-M11`，产物 `.autome/output/screens-M11/{dashboard,project,task,loop,skills}.png`，均 1512×944。按 G-Q3 交接，下一实现轮把它们复制进 `docs/images/`（不需监听端口），跑验收与快检。
- 判别检查即验收命令本身（当前应红，复制后应绿）。

详见 evidence/M-11-r14-audit.md。
