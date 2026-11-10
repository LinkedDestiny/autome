# M-04 复验 · 审计 #4

## 负向对照（先做）
1. `home.tsx` 加未使用变量，跑整套 `scripts/ci-local.sh`：Web lint `FAILED`，Web typecheck/test/build 仍各自 `ok`，总退出码 1。已还原。
2. 把 ci-local 的四行 `run 'Web …'` 合成一条 `&&` 链：`test/ci-config.test.ts` 的「本地四行独立 run…」用例失败（1 failed | 5 passed）。已还原。

## 验收命令
`rm -rf apps/web/node_modules && sh scripts/ci-local.sh`：exit=0，118s；Format/Clippy/Test/End to end/Adapter flags/Desktop/Contract/Web install/lint/typecheck/test/build 全部 ok。Web test：5 文件、38 用例（基线 32，新增 ci-config 6）。
Web 各步耗时：install 12s、lint 1s、typecheck 3s、test 6s、build 1s，远低于 30 分钟。
另核对 ci.yml：web job 有 timeout-minutes 15、macos-14、setup-node '22'、npm ci 与四步；各 job 都有 timeout-minutes。
本环境能监听本地端口，实现轮的 EPERM/EMFILE 阻塞未复现；宿主门禁同样通过。

## 结论
通过。工作区还原后无残留改动。
