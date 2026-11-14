# M-06 · 实现 #6 · 部分实现，保持开放

## 结论与改动

未取得 M-06 全部通过证据，不标待审。三组显式成员（unit 65、kernel 28、exclusive 18，共 111 文件）已实现；kernel 暂取设计允许的 4 workers，尚未用性能实测定准。三个 project 同 groupOrder，exclusive 文件串行。globalSetup 统一构建，六个自构建套件改为 ensureBuilt，嵌套 runner 继承同一构建标记。快检配置回归同步断言三个 project 与 globalSetup。README、技术设计同步。

pidfile 包装脚本修复参数丢失、版本探针覆盖 pid；加入项目脚本回归。共享构建加入一次构建、嵌套复用及失败不发布标记的回归。上轮审计 M-05 通过，没有必须搬运的退回检查。

## 命令与结果

原始日志均位于 `.autome/output/`，以下是结论而非原始输出转存。

| 命令（仓库根执行，注明 cwd 者除外） | 结果 | 日志 |
|---|---|---|
| `cd apps/web && npx vitest run test/test-groups.test.ts test/core-sidecar.test.ts test/eventbus-live.test.ts test/logtail-live-cleanup.test.ts` | 退出 1；4 文件、30 用例：22 通过、1 失败、7 因 beforeAll 失败未执行；eventbus 另有套件级失败 | m06-acceptance.txt |
| `cd apps/web && npx vitest run --project unit` | 退出 1；65 文件、569 用例：568 通过、1 失败 | m06-unit.txt |
| `node --test scripts/test/ci-quick.test.mjs` | 退出 0；13/13，含实际 related 执行 ci-config 的 9 项 | m06-quick.txt |
| `node --test scripts/test/web-build.test.mjs scripts/test/web-pidfile.test.mjs` | 退出 0；4/4 | m06-fixtures.txt |
| `cd apps/web && npx vitest run test/styles.test.ts --fileParallelism=false` | 退出 1；12 项中 11 通过、1 失败，串行同样失败 | m06-styles-serial.txt |
| 对本轮 11 个 Web TS 文件执行 `npx eslint <文件清单>` | 退出 0；无诊断 | m06-eslint.txt |
| `git diff --check` | 退出 0 | 无输出 |

用例口径：上述主要测试集合去重总数 **616**（30 + 569 + 13 + 4），共 607 通过、2 失败、7 未执行；重复的单文件检查及 ci-quick 内嵌 ci-config 不重复计数。同一范围上一版有 608 项，本轮 **+8**：Web 分组守卫 +4、pidfile 回归 +2、共享构建回归 +2。上一轮 M-05 的 19 个 Rust 用例属于另一范围，本轮未重跑，不能把 616 与 19 当覆盖率变化比较。M-06 指定子集由 26 增到 30（+4）。未执行全量 Web，840 只是设计预期，不声称实测总数。

阻塞：eventbus 启动日志明确为 `listen EPERM: operation not permitted 127.0.0.1`；cleanup 明确为 `spawnSync ps EPERM`。独立 `ps -axo pid=,ppid=,command=` 同样被沙箱拒绝。不能获取残留进程证据。unit 唯一失败是既有 `task-timeline.tsx:10` 的内联 `style` 违反 styles 守卫；HEAD 已有该代码，串行复跑一致，本轮未更改或放宽它。

## 标待审之前的自审清单（已逐项执行，未全通过）

1. **情形表逐行**：D.1 unit 默认并行、kernel 4 workers、exclusive fileParallelism=false、同 groupOrder 与全文件唯一归属，4 个分组守卫通过。共享构建及嵌套复用通过脚本检查。D.2 空闲/负载前后各三次、连续五次全绿和无残留尚未完成，不能按通过处理。D.3 参数/版本分支通过，ENOTEMPTY 因环境限制未定位。
2. **失败分支**：共享构建命令返回 23 时向外抛错且不写标记，回归通过；eventbus 监听失败、cleanup 进程读取失败已真实执行并记录。设计要求的去掉 globalSetup 后干净 build 的 gateway 对照未跑：当前环境即使保留构建也不能监听，无法形成有鉴别力的对照。20 次旧夹具/20 次新夹具及 lsof 写入者诊断未跑，不能用重复 EPERM 冒充复现。拒绝授权/外部服务不可达不适用，本改动无相关接口。
3. **全部迁移**：构建无标记→成功→有标记→再次调用/嵌套 runner 沿用，及无标记→失败→仍无标记均通过。pidfile 不存在→版本探针保持不存在、已有 pid→版本探针保持原值、正常启动→写真实 exec pid 均通过。内核 offline→connected 属未执行的 eventbus 7 项，未取得证据。
4. **输入域边界**：文件归属全覆盖、重复数量检查通过；参数含空格、空串原样传递通过；`--version` 与无既有 pid/有既有 pid 两种边界通过。单文件运行 test-groups 成功。日期、数值上限不适用，本轮不引入此类输入。

## 负向对照与 ENOTEMPTY 结论

- 将 eventbus-live 从 exclusive 移入 unit：分组守卫 3 绿 1 红；恢复后 4 绿。日志 m06-groups-red.txt / m06-groups-green.txt；改动已完全恢复。
- 用 HEAD 原始 pidfile 脚本运行项目 `web-pidfile.test.mjs`：2 红（版本探针写 pid、参数丢失）；修复后 2 绿。日志 m06-pidfile-red.txt / m06-pidfile-green.txt。
- 已证实夹具错误，**尚未证实 ENOTEMPTY 的写入者**。保留原有有界 maxRetries/retryDelay；不把静态因果假设当根因证据。

## 性能、剩余工作与配置

检查环境时 load average 为 7.60，后续为 6.56，且 ps 被永久权限边界拒绝；未等待 30 分钟，因为即使负载下降也不能证明无其它进程、完成残留检查或监听。未采集空闲/负载前后各 3 次，未做连续 5 次全绿，未声明 V-5 通过。剩余工作：在可监听且可执行 ps/lsof 的宿主完成 D.2 / D.3；确认已有 styles 失败的处理边界；按实测定准 workers，再报待审。未运行项目全量门禁或宿主快检。

建议配置（操作者合并后应用，本轮未修改 `.autome/` 配置）：

```toml
[gate]
command = "sh scripts/ci-local.sh"
run = "before_merge"
timeout_minutes = 30
quick = 'if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi'
quick_timeout_minutes = 10
```
