# M-03 实现 #4

结论：待审（仅依据设计 M-03 的 listen EPERM 环境例外交审计复验；真实链路未取证，不宣称验收全绿）。implementation-round：4/25。

## 修复与审计回归

- `test/logtail-live.test.ts` 的 beforeAll 两处就绪等待改为 Vitest `vi.waitFor`；首次 `startTask()` 移到第 1 条测试体，所有 `expect.poll` 均在测试体或其调用链内执行。
- 项目注册及 onboarding skip 移到 beforeAll，只执行一次；两次 startTask 复用 projectId，各建独立任务。没有修改产品实现或削弱断言。
- 审计使用的原始验收检查已在项目测试体系中，本轮保留三条测试的产品断言原文（3000ms、全文与连续偏移、lsof 正向对照、bus.close 入口资源归零）。审计临时副本已删除且未提供逐字源码，只有构造说明；不虚称复制了不存在的源码。本轮按审计允许的“把 startTask 放进测试体”方案修复其同类调用。
- 修改前先原样运行审计命令：发现 21 条、18 通过、3 skipped，失败原因是更早的 `listen EPERM`，没有进入 expect.poll 缺陷点；这是环境阻塞，不算缺陷红灯复现。修改后同命令仍在监听处失败，不能记作绿灯。
- 再做 `main.ts` 删除 `await logtail.close()` 的负向对照，退出 1，仍是监听 EPERM，18 通过、3 skipped，无法判别关闭缺陷。已用 finally 原样恢复 main.ts，`git diff -- apps/web/server/main.ts` 为空。intervalMs/句柄泄漏变体本轮未重跑：同一监听前置不可达，无法产生有效判别；审计仍需在可监听环境复验三项。

## 命令与结果

原始输出均在 `.autome/output/`，以下退出码为实际子命令结果。

| 命令 | 退出码 | 结果与输出 |
|---|---|---|
| `cargo build -p automed -p autome-cli` | 0 | `m03-r4-build.txt` |
| 修复前 `npm test --prefix apps/web -- test/logtail-live.test.ts test/core-sidecar.test.ts` | 1 | 21 条，18 通过、3 skipped；`m03-r4-red.txt`，监听 EPERM |
| 修复后同一验收命令 | 1 | 21 条，18 通过、3 skipped；`m03-r4-final.txt`，监听 EPERM |
| `npm test --prefix apps/web -- test/logtail.test.ts test/eventbus.test.ts` | 0 | 43 条全部通过；`m03-r4-unit.txt` |
| `npm run lint --prefix apps/web` | 0 | `m03-r4-lint.txt` |
| `npm run typecheck --prefix apps/web` | 0 | `m03-r4-types.txt` |
| Node net.createServer 监听 `127.0.0.1:0` 探针 | 1 | EPERM；`m03-r4-listen.txt` |
| 缺少 logtail.close 的变体验收命令 | 1 | 21 条，18 通过、3 skipped；`m03-r4-negative-close.txt`，环境阻塞，非有效负向证据 |

去重用例总数 **64**（21 验收 + 43 直接相关回归），61 通过、3 环境阻塞。相对上一实现轮 64 条基线差 **0**：修复既有测试夹具，未增删用例。相对上一审计的 21 条执行范围多 **43**，来自重新执行既有 logtail 21 条与 eventbus 22 条，不是新增测试。M-03 验收命令自身仍为 21 条，与上一审计差 0。重复运行和变体不重复计数。未跑全量门禁或宿主快检；production build 由 live 测试 beforeAll 执行，成功后才到监听失败处。

## 标待审之前的自审清单

1. **情形表逐行**
   - M-03 第 1 条：running → offset 0 → 初始行 → marker，保留 3000ms 时限和耗时打印；真实运行被 EPERM 阻塞，无实测时延。logtail 第 21 条定时跟随通过，不替代端到端。
   - 第 2 条：stop → scheduler.tick → finished 尾行 → end → 全文/偏移一致 → stats/lsof；真实链路未执行。logtail 第 6、7、13 条尾段及释放通过。
   - 第 3 条：path 拒绝 → 同项目第二任务 → 活跃订阅 → service.close → bus.close 入口已释放；夹具已修复，真实链路及负向对照被 EPERM 阻塞。logtail 第 2、13 条通过。
2. **失败分支**
   - 客户端路径/非法 id、非法内核路径：logtail 第 2、3 条通过；真实 /ws 路径拒绝未取证。
   - open/stat/read 失败：第 18 条通过；删除、缺文件、轮转：第 8、9 条通过。
   - not_found/offline/kernel 拒绝与生命周期查询失败后恢复：第 10、14 条通过；core-sidecar 18 条通过，覆盖既有超时/进程退出路径。
   - 空历史：第 7 条通过；外部服务不可达不适用（本里程碑无外部服务）；本机监听被拒已用独立探针确认。
3. **全部迁移**
   - 无订阅 → 活跃、重复替换、4/5 限额、退订：第 1、4、12 条通过。
   - 活跃 → 追加；查询失败 → 继续跟随 → 恢复结束：第 14、21 条通过。
   - running/历史 → ended；缺文件等待 → 文件出现；deleted/rotated/error → 清理：第 6–9、18 条通过。
   - 活跃/在途 → 退订、断连、替换、close；断连 → next 续传：第 11–13、15、19 条通过。
   - 真服务 close → logtail.close → bus.close：保留入口前断言，但本轮未取得运行证据。
4. **输入域边界**
   - offset 缺省、0、超文件长度；空文件；150 KiB 中文、1 MiB 多轮；残缺 UTF-8：第 1、5、7、8、16、17、20 条通过。
   - 第 4/5 个订阅、连接隔离、非法 id/路径字段：第 2、4 条通过。
   - 真实 session id、中文与随机 marker 的端到端检查受监听限制未执行。本轮夹具修复不新增输入域，日期/前导零等不适用。

## 交审计项

在允许监听的环境运行验收命令，要求 21/21 且 skipped 0，再取得三种负向对照红灯。尤其核对第二次 startTask 不再 project.add，且关闭断言在 bus.close 前检验，不被断连清理掩盖。本轮不推进 M-04。
