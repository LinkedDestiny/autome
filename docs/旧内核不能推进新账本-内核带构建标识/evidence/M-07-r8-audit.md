# M-07 r8 审计复验

## 验收命令
`npm run test --prefix apps/web -- test/ci-config.test.ts && sh -n scripts/ci-local.sh`

- 结果：8 例中 7 过、1 败（`CLI package 隔离 home 与 prefix…`），`sh -n` 退出 0。预期是 8 过。
- 失败断言：`expect(path.startsWith(resolve(sandbox,'tmp') + '/')).toBe(true)`（ci-config.test.ts 约 95 行）。
- 根因：本机（Darwin）`/usr/bin/mktemp -d` 不带模板时忽略 `TMPDIR`。实测 `TMPDIR=/tmp/zz mktemp -d` 仍落到 `/var/folders/.../T/tmp.XXXX`，`-t x` 同样。
  `ci-local.sh:121` 用的正是 `mktemp -d`，所以 scratch 不在测试沙箱的 `tmp` 里，断言必败。
- 产品侧：脚本为 `install-cli.sh` 设了独立 `PREFIX` 与 `AUTOME_HOME`，有 EXIT/HUP/INT/TERM trap，没发现隔离本身有缺陷。缺陷在新增用例对 `mktemp` 的平台假设，属项目测试体系不通过。

## 负向对照
验收命令自身已红，没法再用它分辨「去掉 AUTOME_HOME」「去掉清理」两种错误实现：用例在第一个路径断言就退出，后面的断言没有被执行过。我尝试写独立 shell 检查，脚本在 awk 提取片段处挂住，已停掉并删除，未得出结论。

## 结论
实现缺陷，退回 `开放`，reopen 加 1，领域 `package-isolation`。
下一轮：把用例里「路径在 TMPDIR 下」改为与平台无关的断言（路径互异、不等于调用方 `AUTOME_HOME`/`PREFIX`、结束后不存在）；改完再跑一遍负向对照（去掉 `AUTOME_HOME=` 赋值、去掉 trap、吞掉安装退出码），确认每种都使用例变红。
