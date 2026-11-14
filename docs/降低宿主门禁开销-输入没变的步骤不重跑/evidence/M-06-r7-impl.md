# M-06 · 实现 #7 · 修复分组启动，保持开放

## 结论与修复

修复审计 #6 证实的启动缺陷，尚未取得 M-06 全部验收证据，不标待审。Vitest 5.1.0 的本地实现 `dist/chunks/index.Di8au7q4.js` 的调度分组要求同 groupOrder 有相同有效 maxWorkers；串行文件另行调度，原先“三组同组所以同时启动”的前提不成立。采用原生配置：unit/kernel 同组共用 4 workers，exclusive 下一组串行。更新设计 D.1 与技术设计，不声称新调度已经达到 50% 目标，不调低 D.2 验收标准。

所有 111 个 Web 文件归属不变，无删测或条件跳过。`test-groups` 仍是 4 项，改强 worker 一致性和 exclusive 后置断言。新增 `scripts/test/web-groups.test.mjs` 自动纳入既有 Scripts 步骤的 `node --test scripts/test/*.test.mjs`：真实启动三个 project 的既有文件 paths、ci-config、load-context，要求成功退出、三个 project 均出现、3 文件通过且没有 no tests。它不递归启动自身，也不需要监听端口。

审计检查 `.autome/output/m06a/full-run-check.sh` **逐字**复制为 `scripts/test/web-full-run-check.sh`，`cmp` 退出 0。原检查不是严格的全绿断言：管道最终只检查 `Test Files .*passed`，部分文件通过也可能返回 0；本轮另加的自动回归要求真实 runner 退出 0。保留原检查，不把它的布尔值当作 840 项全绿证明。

## 命令与结果

原始输出：`.autome/output/m06r7/`。未运行 `ci-local.sh`、宿主快检组合、全量 Rust 门禁。

| 命令 | 结果与日志 |
|---|---|
| `sh .autome/output/m06a/full-run-check.sh`（修前原样） | 退出 1；red-original.txt。沙箱还拒绝 tee 打开 /dev/stderr，故用下一行获取原始诊断 |
| `cd apps/web && npm run test`（修前） | 退出 1；red-direct.txt：不同 maxWorkers / 相同 groupOrder，Test Files no tests |
| `node --test scripts/test/web-groups.test.mjs`，临时恢复 HEAD 原配置 | 退出 1，1 项红；regression-red.txt：同一配置冲突，随后恢复修复配置 |
| 同一回归命令，修复配置 | 退出 0，1 项绿，内嵌 3 文件 28 个 Web 用例全绿；regression-green.txt。最终版本使用 verbose reporter，避免默认 reporter 隐藏成功文件名 |
| `cd apps/web && npx vitest run test/test-groups.test.ts test/core-sidecar.test.ts test/eventbus-live.test.ts test/logtail-live-cleanup.test.ts` | 退出 1；4 文件 30 项，22 通过、1 失败、7 因 beforeAll 失败未执行；acceptance.txt |
| `sh scripts/test/web-full-run-check.sh`（修后原样） | 退出 0（原判据绿）；full-check-after.txt。tee 被拒绝，未保存全量明细与总数；此管道不证明全量全绿 |
| `node --test scripts/test/ci-quick.test.mjs` | 退出 0，13 项全绿（89.74 s）；quick.txt。含实际 related 配置/运行验证 |
| `node --test scripts/test/web-build.test.mjs scripts/test/web-pidfile.test.mjs` | 退出 0，4 项全绿；fixtures.txt |
| `cd apps/web && npx eslint vitest.config.ts test/test-groups.test.ts` | 退出 0；eslint.txt |
| `node --check scripts/test/web-groups.test.mjs`、`sh -n scripts/test/web-full-run-check.sh`、`git diff --check` | 均退出 0 |

指定子集总数 **30**，与审计 #6 / 实现 #6 基线差 **0**；回归启动的 **28** 项均为既有测试，新增项目脚本回归 **1** 项。没有拿启动成功冒充全量通过。

本轮可核对的主要测试集合去重 **76 项**（30 + 28 + 1 + 4 + 13）：68 通过、1 失败、7 未执行。与上一实现轮报告的 616 项相差 **-540**：本轮没有重跑 569 项 unit 全组，改为跨 project 的 28 项既有用例及新增 1 项脚本回归（-569 + 28 + 1）。这是命令选择变化，非删测；同范围新增测试只有 1 项。红绿重复运行不重复计数，ci-quick 内嵌 ci-config 与显式三组运行去重，原审计管道未保留全量计数，不计作可核对全量覆盖。

## 标待审之前的自审清单（逐项执行，未全通过）

1. **情形表逐行**：D.1 unit/kernel 有限并行与相同 worker 上限、exclusive 后置且串行、111 文件唯一归属、资源测试不进 unit、配置不按环境增减文件，由 4 个守卫验证通过；真实 runner 跨三组 28 项通过，避免再次只读配置对象漏检。共享预构建/嵌套复用的 2 项脚本回归通过。D.2 两种口径的交替基准、连续五次全绿及无残留未取得；不以纸面推算替代测量。D.3 参数转发/版本探针的 2 项通过，但 ENOTEMPTY 写入者未定位。
2. **失败分支**：旧配置启动失败→新配置实际运行，通过红绿回归验证；共享构建失败不发布标记，通过 web-build 验证。eventbus 的真实监听分支报 `listen EPERM 127.0.0.1`，并有 EMFILE watcher 诊断；cleanup 明确 `spawnSync ps EPERM`。独立 TCP 探针同样 EPERM，独立 ps 退出 127。授权拒绝、远端不可达不适用，本次不引入相关接口。干净 build 去掉 globalSetup 的负向对照本轮未重跑（审计 #6 已验证）；不能声称本轮完成该项。
3. **全部迁移**：无构建标记→成功发布→同进程/嵌套沿用，以及无标记→失败→不发布，由 web-build 两项通过；pidfile 不存在/存在→版本探针保持、普通启动→参数原样 exec，由 web-pidfile 两项通过。调度配置覆盖并行组→串行组，并以实际三项目运行检查可执行性；内核连接迁移受监听限制未验收，未绕过它。
4. **输入域边界**：111 文件全覆盖且无重复，单 project 运行与跨三个 project 运行均有通过证据；旧的不同 worker 上限为拒绝边界，修后同上限为合法对照。参数空串/含空格及版本探针有/无既有 pid 边界由脚本回归通过。日期、数值最大值不适用，本轮未引入此类业务输入。

## 环境、性能与 ENOTEMPTY

本轮独立测得 load average 5.96 / 6.70 / 6.97；ps 被沙箱拒绝，127.0.0.1 监听被拒绝。没有等待 30 分钟采空闲数据：即使负载下降，固定权限边界仍阻止完整验收。未执行没有鉴别力的重复 EPERM 五连跑、旧新夹具各 20 次及性能比值；无残留也不能确认。保持开放，交由后续具备权限的环境继续，未声称 V-5 通过。

ENOTEMPTY 写入者未定位，保留有界重试。已有证据排除了“包装器正确转发参数”和“版本探针不覆写 pid”这两个旧假设：包装器旧实现确有错误，现有回归证实已修复；但不能因此推断它就是 ENOTEMPTY 根因。

建议配置仍沿用 M-06-r6-impl.md 的 `[gate]` 原文（command/run/timeout_minutes/quick/quick_timeout_minutes 不变），由操作者合并后应用。本轮不修改 `.autome/` 配置，不启动下一会话。
