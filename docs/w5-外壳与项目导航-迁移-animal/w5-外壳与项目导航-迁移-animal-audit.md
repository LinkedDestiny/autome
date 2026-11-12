# 实现审计（覆盖写）

## M-05 · 审计 #6 · 通过
- 4 条负向对照（mac 接受 ctrl、down/up 不跳禁用项、过滤只匹配标题、新建候选不读写判定）各使 1–2 条用例变红，还原后树干净；验收 2 文件 11/11，Web 全量 43 文件 364/364（skipped 0，基线 357+7），lint/typecheck/build 退出码 0，desktop 165/165，宿主门禁通过。
- 新造检查为上述注入，通过，丢弃。副标题多匹配记入 Backlog B-06。
- 证据：docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r6-audit.md
