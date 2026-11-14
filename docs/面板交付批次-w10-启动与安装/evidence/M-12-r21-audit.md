# M-12 审计复验（实现 r21）

结论：通过。
- 验收命令各段独立跑：prepare-web 5/5，skipped 0；packaging 7/7，skipped 0；`sh -n` 无输出。
- 干净目录：`git ls-files -co` 复制 apps/web，无 build/ 无 node_modules；`prepare-web.sh` 真实 `npm ci` + build 成功（退出 0），stage-panel 暂存后 `bin/autome-web.js`、`build/server/index.js` 均在。
- 负向对照（均已还原，git status 干净）：
  1. package.sh 去掉 prepare-web 调用 → packaging 失败 1 条；
  2. 有 node_modules 就跳过 build → prepare-web 4 条失败；
  3. 只看目录不比指纹 → 第 3 条失败；
  4. 先写指纹且 `npm ci || true` → 第 4 条失败。
- 自造检查：上面第 2 项初版变异（以 node_modules/.bin 判断）未被拦住，因假目录无 .bin，属变异本身偏离契约，换成契约原文的变异后被拦住。
