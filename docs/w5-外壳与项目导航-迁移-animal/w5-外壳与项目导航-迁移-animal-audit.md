# 实现审计（覆盖写）

## M-04 · 审计 #5 · 通过
- 3 条负向对照（计数绕过 columnOf、根 shouldRevalidate 去掉 `task.`、taskHref 不编码）全部使对应用例变红，还原后树干净；验收 5 文件 30/30（含监听用例，本环境可监听），Web 全量 357/357，lint/typecheck/build/desktop 测试退出码均为 0。
- 证据：docs/w5-外壳与项目导航-迁移-animal/evidence/M-04-r5-audit.md
- 新造检查：无（负向对照即判别检查）。
