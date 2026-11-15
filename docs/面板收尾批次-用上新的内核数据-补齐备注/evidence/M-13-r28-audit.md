# M-13 r28 集成复验（审计 #20）

范围：`8cf6562`（apps/web/playwright.config.ts、apps/web/test/ci-config.test.ts）。

1. 改动只把 `globalTimeout` 360_000→720_000，单例 90_000、assertion 10_000、workers=1、retries=0 不变；无 rebase 丢失提交（简报 0 个）。
2. 测试 diff：只新增一条用例，无断言被削弱；上限断言要求 < ci.yml e2e 的 15 分钟，720 s 满足。
3. 冲突标记：`git grep` 对 8cf6562（排除文档目录）为空。
4. M-13 验收命令：build 0；3 文件 11 用例全过（task-detail-ssr/payload/test-groups）。
5. 负向对照：把 globalTimeout 改回 360_000，ci-config 新用例失败（12 过 1 败）；已 `git checkout` 还原，工作区干净。
6. 未跑：29 例浏览器全套（耗时 >6 分钟，由宿主合并前门禁复验，风险节 R-1 已写明）。

结论：通过，里程碑表不动。
