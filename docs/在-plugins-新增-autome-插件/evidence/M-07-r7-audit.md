# M-07 审计 #7 独立复验

结论：通过。

## 检查（自己构造，未读实现轮证据）
- 逐条提取 H-01/H-02/H-04，用固定串匹配 `export AUTOME_HOME=$(mktemp -d)` 与 `unset AUTOMED_DB_PATH`：三条均 2/2。
- 负向对照（在副本上篡改，设计文档未动）：H-02 删去 export → H-02=1；H-04 删去 unset → H-04=1；三条 export 都换成 `AUTOME_HOME=x` → 全部 1。均能被检出，验收判据有区分力。
- H-03/H-05/H-06 不涉及账本隔离，未要求。
- 项目测试体系：`run.sh all` 59/59（与 M-05 基线一致，无增减）；`verify-cli.sh skill readme` 6/6 + readme 组通过。
- 宿主门禁 ci-local.sh 通过（简报引用）。

## 说明
本里程碑是文档改动；人工验收项本身（H-01/02/04）仍由用户合并前确认，不属本里程碑关闭条件。
