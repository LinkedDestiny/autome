# 审计结论 · M-03 · 审计 #4

结论：**实现缺陷**，退回 `开放`，reopen 1，领域 package。收敛模式 normal。

缺陷：`scripts/install-cli.sh:188` 写 `"$panel_dest；运行 autome web"`，`$panel_dest` 后紧跟全角「；」。macOS `/bin/sh`（bash 3.2）把多字节首字节并入变量名，`set -u` 下报 `panel_dest�: unbound variable`，脚本在此中止：「已安装面板」消息丢失、其后的 PATH 提示整段不执行，每次装面板都向 stderr 报错。违反任务 A-4（安装并提示 `autome web`）与 install-cli 既有的 PATH 提示行为。现有验收放过它是因为用例只断言面板文件在、HTTP 200，不看 stderr 与输出文本。修法：写成 `${panel_dest}；`。同类检查范围：两份脚本里 `$name` 后紧跟非 ASCII 字符仅此一处。

失败命令：`sh .autome/output/a4-install-msg-check.sh <tar.gz>` → `FAIL stderr: scripts/install-cli.sh: line 188: panel_dest�: unbound variable`，rc=1。
判别检查：`.autome/output/a4-install-msg-check.sh`（下一轮搬进 install-alignment：stderr 为空、stdout 含「已安装面板」与「不在 PATH」）。

验收命令本身：4 文件 25 条全过、skipped 0，`sh -n` 无输出；负向对照（binary.ts 不认 AUTOME_BUNDLED_CORE→3 条失败；node_ok 恒 1→第 3 条失败）有效，已还原。证据：docs/面板交付批次-w10-启动与安装/evidence/M-03-r4-audit.md
