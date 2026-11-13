# 审计 · 第 3 轮 · M-03

结论：**实现缺陷**，M-03 退回 `开放`，reopen 1，领域 logtail-live。
证据：docs/w8-实时会话日志/evidence/M-03-r3-audit.md
- 失败命令：`npm test -- test/logtail-live.test.ts test/core-sidecar.test.ts`，退出码 1；尾部：`Error: expect.poll() must be called inside a test ❯ test/logtail-live.test.ts:66`，3 skipped。
- 根因：测试文件在 beforeAll（及其调用的 startTask）里用了 `expect.poll`；第 3 条还重复 `project.add` 报「该目录已经是一个项目」。
- 产品行为：绕开这两处后，第 1、2 条在真实内核上通过，intervalMs 与句柄泄漏两个负向对照都能变红。
- 判别检查：临时副本已删除，构造方式写在证据文件里。
