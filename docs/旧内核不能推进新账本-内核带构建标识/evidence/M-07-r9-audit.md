# M-07 r9 审计复验

- 验收命令：`npm run test --prefix apps/web -- test/ci-config.test.ts && sh -n scripts/ci-local.sh` → 8 通过 / 0 失败，语法检查退出 0。
- 负向对照（对 scripts/ci-local.sh 临时注入，已用 git checkout 还原，git status 干净）：
  - A 删除 `AUTOME_HOME="$ledger_home"` 赋值 → 1 failed | 7 passed。
  - B 删除 `trap 'rm -rf …'` 清理 → 1 failed | 7 passed。
  - C 安装命令后加 `|| true` 吞失败 → 1 failed | 7 passed。
- 上一轮缺陷（mktemp 忽略 TMPDIR）：当前用例在本机（macOS）通过，不再假定 scratch 在 TMPDIR 下。
- 脚本现状：install-cli.sh 同时带临时 PREFIX 与 AUTOME_HOME，trap 覆盖 EXIT/HUP/INT/TERM，安装退出码经 set -e 保留。
- 未跑 ci-local.sh --package（约束 6）。
