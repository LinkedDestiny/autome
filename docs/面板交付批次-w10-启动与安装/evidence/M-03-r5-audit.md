# M-03 复验证据 · 审计 #5

- 验收命令：4 文件 26 条全过（install-alignment 6、含安装消息回归与无残留、2 条在本环境未被 ps EPERM 阻塞），`sh -n` 两脚本无输出。
- 负向对照：把 install-cli.sh:188 的 `${panel_dest}；` 改回 `$panel_dest；`，install-alignment 第 5 条失败（stderr 非空 `panel_dest�: unbound variable`），其余 5 条过；已还原，git status 干净。
- 同类扫描：两份脚本里 `$name` 后紧跟非 ASCII 仅余转义/注释处（package-cli.sh:93 为 `\$`，install-cli.sh:20 为注释），无漏网。
- 结论：缺陷已修，通过。
