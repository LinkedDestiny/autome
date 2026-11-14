# M-03 r4 审计证据

- 验收命令：vitest 4 文件 25 passed、skipped 0；`sh -n` 无输出。本机 listen/ps 可用，实现轮的环境阻塞不复现。
- 负向对照：binary.ts 把 `AUTOME_BUNDLED_CORE` 改名 → core-binary 2 条 + install-alignment 第 2 条失败；`node_ok=1` 恒真 → 第 3 条失败。均已还原，`git status` 干净。其余对照（serve 选模式、dev 依赖、--binary 构建、ci.yml 顺序）本轮未逐一注入。
- 判别检查（发现缺陷）：用 stage-panel + package-cli 造包后在空 PREFIX 下用 /bin/sh 安装，stderr 出现 `line 188: panel_dest�: unbound variable`，面板已落盘但成功消息与 PATH 提示丢失。最小复现 `.autome/output/a4-mb.sh`；检查脚本 `.autome/output/a4-install-msg-check.sh`，当前 rc=1，修复后应 OK。
- 全量 vitest 输出里该错误出现 4 次，用例仍绿——说明测试不看 stderr。
