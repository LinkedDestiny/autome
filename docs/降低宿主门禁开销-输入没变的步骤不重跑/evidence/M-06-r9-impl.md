# M-06 · 实现 #9 · 清理重试修复，验收环境阻塞

结论：保持 **开放**，不标待审。`implementation-round: 9/30`，reopen 保持 3，convergence-mode 保持 milestone-review。本轮没有运行项目全量门禁或宿主快检；只运行 M-06 命令与改动相关子集。

## 修复与同类检查

- `w9Kernel.close()` 原先在 `await core.stop()` 后立即 `rmSync`，现改为等待 `fs/promises.rm`，参数 `recursive/force` 保留，`maxRetries: 5, retryDelay: 100`。清理耗尽预算仍拒绝，不把持续 ENOTEMPTY 当作成功。HOME 在删除前恢复。
- `AutomedSidecar.stop()` 本身已监听直接子进程 exit；因此不能把“未等待直接内核退出”当作已证实根因。残留后代、其它写入者或文件系统短暂竞态尚不能区分。**写入者未定位**，本轮采取审计允许的有界重试，不声称根因根治。
- 同类扫描：`.autome/output/m06r9/cleanup-inventory.txt`、`core-stop-files.txt`。对 board-detail-real、board-live、task-new-real、notifier-backfill-real、notifier-real、shell-live、gateway、install-alignment 的停止内核后删根目录加入同样 5 次 / 100 ms 有界重试。alerts-real、w9-http、settings-onboarding-real 共用 w9 close；task-detail-real 的 process-leak teardown 已要求进程为空才删除，并保留清理失败报错。未把纯文件夹具、测试中途删除信号文件等非同类位置批量改写。
- 新增 `w9-cleanup.test.ts` 3 条契约回归并归 kernel：等待 stop 后才删除且等待删除完成；持续 ENOTEMPTY 原样上抛并保留根目录；启动失败同样等待清理。这是故障注入契约检查，**不是实际文件系统竞态的确定性复现**。

## 审计检查搬入与红绿记录

1. 改动前原样执行 `sh .autome/output/m06c/bench.sh`。旧配置路径 `apps/web/vitest.old.config.ts` 已不存在，old1 启动失败；new1 出现进程检查失败。确认 `ps` 被沙箱拒绝后中止原脚本（退出 130），没有得到 ENOTEMPTY 原症状复现。日志：`.autome/output/m06r9/red-bench.log`、原脚本固定输出的 `.autome/output/m06c/old1.log`、`new1.log`；这次运行覆盖了原脚本同名日志，历史结果以已提交的 r8 审计证据为准。
2. 原脚本逐字搬到 `scripts/test/fixtures/web-full-benchmark-audit.sh`，`cmp .autome/output/m06c/bench.sh scripts/test/fixtures/web-full-benchmark-audit.sh` 退出 0。
3. 可长期执行的入口 `sh scripts/test/web-full-benchmark.sh` 保留交替旧/新各 3 次、随后新配置连续 5 次的检查；旧配置原文固定在 `apps/web/test/fixtures/vitest-serial.config.ts`。输出使用新的独立目录，失败退出非零；每次记录墙钟、前后 load、用例数，私有 TMPDIR，结束两秒后检查含 TMPDIR 的进程与后代。原检查不能因末尾 echo 成功而掩盖失败；新入口还拒绝报告 skipped 的运行。
4. 改后执行新入口：在 `ps` 前置检查处退出 1，见 `.autome/output/m06r9/benchmark-green-attempt.log`；**原审计判别检查的绿证据未取得**。不以契约测试的绿替代连续全量。
5. 新增契约回归，修复前运行 `(cd apps/web && npx vitest run test/w9-cleanup.test.ts)`：3 条全部失败（未调用异步 rm、持续错误未传播、启动失败清理未等待），见 `red-cleanup.log`；修复后它随下述两个子集执行，两次均 3/3 通过。

## 命令与结果、数量基线

原始输出均在 `.autome/output/m06r9/`，不存入任务文档目录。

| 命令 / 日志 | 结果及用例总数 | 与基线之差 |
|---|---|---|
| `node --test scripts/test/web-groups.test.mjs` / `groups.log` | 退出 0，Node 1 条；内部真实运行三个 project 共 3 文件 / 28 Web 用例 | Node 1、内部 28 均与上一轮相同，差 0 |
| `(cd apps/web && npx vitest run test/w9-cleanup.test.ts test/test-groups.test.ts test/styles.test.ts test/core-sidecar.test.ts test/eventbus-live.test.ts test/logtail-live-cleanup.test.ts)` / `subset.log` | 退出 1；6 文件 / **45** 用例：37 通过、1 失败、7 因 eventbus beforeAll 失败未执行；eventbus suite 另计钩子失败 | M-06 原四文件 30 条 + styles 12 条 + 新 cleanup 3 条 = 45；相同选择集的改前数量 42，净增 3 |
| `(cd apps/web && npx vitest run test/w9-cleanup.test.ts test/test-groups.test.ts test/styles.test.ts test/alerts-real.test.ts test/board-detail-real.test.ts test/notifier-backfill-real.test.ts test/notifier-real.test.ts test/board-live.test.ts test/task-new-real.test.ts test/gateway.test.ts test/install-alignment.test.ts)` / `green-related.log` | 退出 0；11 文件 / **54** 用例全部通过 | 既有选择集 51 条 + 新 cleanup 3 条，净增 3 |
| `(cd apps/web && npx vitest run test/shell-live.test.ts)` / `shell-live.log` | 退出 1；1 文件 / **3** 用例，2 通过、1 失败：监听 127.0.0.1 EPERM | 既有 3，差 0 |
| 仅对本轮 12 个 TS 改动文件执行 `npx eslint <文件列表>` / `lint-changed.log` | 退出 0，无输出；文件列表为本轮 test-groups、w9-cleanup、w9-replies、vitest-serial 及上列八个同类测试文件 | 静态检查，不计测试用例 |
| `sh -n scripts/test/web-full-benchmark.sh`、原脚本 `cmp`、`git diff --check` | 均退出 0 | 静态检查，不计测试用例 |

以上成功结束的 Web 测试命令共报告 130 个用例出现次数，去除重叠的 cleanup 3、groups 4、styles 12 后为 **111 个不同 Web 用例**；对应改前选择集 108，新增 3。Node 外层用例另计 1，未变化。未将中止的全量计入这个总数。

上一审计全量基线为 **111 文件 / 840 用例**。本轮新增 1 文件 / 3 用例，因此全量**预期** 112 / 843；本轮未取得完整全量报告，不能写作实跑通过数。没有删除或主动跳过任何测试；上述 7 个 skipped 是真实监听失败造成的钩子级未执行。

## 自审清单（四项全部执行）

1. **情形表逐行**：D.1 unit/kernel 同组 4 workers、exclusive 随后串行、每文件唯一归组、构建 globalSetup、配置不按环境删减文件，均由 test-groups 4 条及真实三组启动回归通过；新增 cleanup 属 kernel。D.2 前后测量、连续 5 次、残留检查：阻塞，不能确认；空闲 H-01 保留，本轮没有完成 30 分钟空闲等待，也不复用 r8 的 0.37 比值当作本轮证明。D.3 参数透传和 --version 不写 pid 的夹具实现保留，但改前/改后各 20 次实验**未完成**：实际 eventbus 启动受 listen EPERM 阻塞，ps 也不可用。
2. **失败分支**：新增检查实际注入持续 ENOTEMPTY，确认拒绝并保留目录；注入启动失败，确认走同一清理路径。eventbus 的 Origin 拒绝等真实 HTTP 失败路径因 beforeAll 监听失败未执行，不能算通过。授权、外部服务不可达、读盘失败不是本次夹具修改新引入的接口分支，不适用；没有为绕过环境而改变测试条件。
3. **全部迁移**：停止等待 → 删除等待 → close 完成由异步受控 Promise 验证；删除失败 → close 拒绝和目录保留通过；启动失败 → stop → 删除 → 原启动错误上抛通过。直接成功清理由 54 条相关实测覆盖。没有改生产状态机或事件字段。真实后代退出/残留迁移因 ps 拒绝尚未复验。
4. **输入域边界**：没有新日期、金额、字符串输入域，不适用。清理相关边界检查了启动失败、删除未完成、持续 ENOTEMPTY；重试固定有限值为 5 / 100 ms，不接受用户输入。目录成员边界由唯一归组检查覆盖新增文件，未缩小测试全集。

## 剩余工作与限制

下一轮在允许监听和 ps 的环境执行新的基准入口，取得负载比值、连续 5 次 112/843 全绿、无残留，并补 D.3 的 20+20 次原位诊断。若仍有 ENOTEMPTY，按实际残留条目和写入者继续定位；当前有界重试不能证明无泄漏。H-01 空闲性能仍由合并前人工核对。本轮不标待审、不改为协议失败。
