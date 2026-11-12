# M-03 · 实现 #3

结论：真实账本新增及改写验收通过；沙箱拒绝 HTTP 监听，完整验收未通过，M-03 保持开放。implementation-round 为 3/15，不将环境问题记为协议失败。

## 改动与上一轮审计

- 新增 `test/notifier-backfill-real.test.ts` 四条顺序共享真实账本的 R1–R4：首次补发、重启同状态、停服期间驳回再进入、v1 升级播种。
- 新增 `fixtures/backfill-ledger.sh`，复用 w4 的假 CLI 编排；setup/redo/reject 均用真实 task.get 校验结束节点。w4 原夹具未修改。
- 改写 notifier-real 的第一条用例：保留七类 14 条事件通知断言，新面板补发恰 5 条，与真实 waiting 集合及事件路径的标题/正文/项目逐行比较，检查 source、序号和五键推送。
- 同状态事件仅注入测试临时真实账本，随后经 automed 的 events.since 读回，核对最大序号、唯一事件及与 task.get 深相等的状态；不是 RPC 替身。
- 上轮审计 M-02 通过，没有退回缺陷、没有要求保留的新检查，故审计检查逐字搬入及红绿复现不适用。

## 命令与结果

原始输出均在 `.autome/output/`，没有放进任务目录。

| 命令 | 结果 | 原始输出 |
|---|---|---|
| `cargo build -p automed -p autome-cli` | 通过 | m03-cargo.txt |
| `npx --prefix apps/web vitest run --root apps/web test/notifier-backfill-real.test.ts test/notifier-real.test.ts test/notifier-live.test.ts` | 总数 9，6 通过、3 跳过，1 文件失败；live 的 beforeAll 报 `listen EPERM: operation not permitted 127.0.0.1`，同时有 EMFILE watcher 提示 | m03-target.txt |
| `cd apps/web && npx vitest run test/notifier-backfill-real.test.ts` | 4/4 通过 | m03-new.txt |
| `cd apps/web && npx vitest run test/store.test.ts test/notifier.test.ts test/notifier-rules.test.ts test/notifier-api.test.ts` | 18 + 17 + 12 + 7 = 54/54 通过 | m03-self.txt |
| `npm test --prefix apps/web` | 总数 315：285 通过、11 失败、19 跳过；30 文件通过、4 文件失败，退出码 1 | m03-full.txt |
| `npm run lint --prefix apps/web` | 通过，零警告 | m03-lint.txt |
| `npm run typecheck --prefix apps/web` | 通过 | m03-types.txt |
| `npm run build --prefix apps/web` | 通过 | m03-build.txt |
| `bash -n apps/web/test/fixtures/backfill-ledger.sh` | 通过 | 无输出 |
| `bash apps/web/test/fixtures/backfill-ledger.sh "$PWD/.autome/output/m03-ledger" "$PWD" setup` | 通过，真实任务先到 await_design_approval 再到 await_merge | m03-fixture.txt |
| `git diff --check` | 通过 | 无输出 |

用例数以实际 vitest 运行报告为准：**315**。本轮比上一轮同一检出 **311 增加 4**，全部来自 R1–R4；旧的 notifier-real 仍为 2 条，未删除。相对本任务起点 **301 增加 14**（S1–S4 +4、N1–N6 +6、R1–R4 +4）。起点及上轮全量数量由当前实际总数和仅有上述新增反推，不声称在旧检出重跑过全量。54 条自审子集比上一轮 54 条增加 0；真实定向范围原为 5 条，本轮为 9 条，增加 4。另跑的 vitest list 静态清单只有 301 项，其中 rules 的参数化入口静态为 6 项、实际为 12 条，说明静态清单不能代替运行总数；不将该清单作为通过或基线证据。

全量失败范围：gateway-http 的 11 条均为 listen EPERM；notifier-live 的 3 条、eventbus-live 的 7 条、server 的 9 条在前置启动失败后跳过。后两者原始栈包含 watcher EMFILE 及监听 EPERM。未修改这些测试，未把跳过计为通过。

设计指定三文件顶层计数已经核对：`store 14 18`、`notifier 11 17`、`notifier-real 2 2`（m03-counts.txt）。当前 main 已前进、含多个并行任务文件差异，故全量基线按共同祖先和本任务变更核对，不能把移动后的 main 全量数直接加 14。相对共同祖先的既有测试改动仅 store/notifier/notifier-real 和共享替身，新加四条在独立文件。

## 标待审前四项自审（本轮执行后仍因环境阻塞保持开放）

1. **情形表逐行**
   - R1：唯一 await_merge，历史 await_design_approval 不补；恰 1 条、source/backfill、序号、正文、项目、五键推送、替身三层形状检查通过。
   - R2：关库后注入同状态新序号，重开后通知行及推送不变、游标前进，通过。
   - R3：redo-me 首次等待变为 2 条；停服期间真实 reject 离开到 design 再入等待，变为 3 条；旧 merge-me 不增加，通过。
   - R4：真实历史播种，两个同状态事件均不重发，状态深相等、seed 删除、游标前进，通过。
   - D8：七类计数仍是 14；fresh 恰 5，集合等于 waiting，每行文案同事件路径，无 done/project_alert，通过。
   - D3 竞态表六行分别由 N3 state-first/leave/after-enter/after-leave-enter/after-waiting 路径以及首次启动静态用例覆盖：稳定 W 为 1；W→X→W 为 2；X→W 为 1；状态先于事件为 1；快照前离开为 0；W1→W2 为 2。review、waiting-change、unstable 额外分支也通过。
2. **失败分支**
   - 拒绝授权：既有带 code 拒绝用例通过，仍产生空补齐字段通知；API 拒绝路径通过。
   - 读取/RPC 失败、超时、外部服务不可达：既有无 code 离线/超时及 N4 三个读取位置 error 分支通过；不推进游标、不半写、恢复可重试。未单独注入真实文件系统读错误，本里程碑没有新增读文件恢复逻辑。
   - 快照三次无法对齐：N3 unstable 检查无行/无游标/无推送及下次恢复，通过。
   - 写盘失败：N4 游标触发器、S2/S3/S4 原子回滚检查通过。
   - 关闭：N4 三次读取位置 close 及既有在途补齐关闭检查通过。
   - 空输入：空账本首次初始化、空集合存储过滤通过。
   - 真实 HTTP 服务监听：live 被沙箱 EPERM 拒绝，未取得通过证据，不绕过也不修改现有测试。
3. **全部状态迁移**
   - 无游标→补发完成→持久化后重启同状态：R1/R2 与 N1 通过。
   - W→X→W 在线及离线：N2 与 R3 通过；W1→W2→W1：N2 通过。
   - v1 有游标→播种→正常拉取：R4/N5 通过；v1 无 notifier 游标（含 other 游标）→不设 seed：S1 通过。
   - 旧账本→新账本、状态清理和新补发：既有换账本、N6/S4 通过。
   - 补发失败/关闭→无提交→重启重试：N4 通过；已提交→重启：R2 通过。
   - 通知器只有同一 publish 通道；五键消息真实补发验证通过，HTTP/WebSocket 端到端三条因监听权限未通过。
4. **输入域边界**
   - 空账本/游标 0：首次启动用例通过；单元素：R1；多任务：fresh 5 条及 N4；最大安全整数读取最新序号：R1 和 sameStateEvent 实际 RPC 使用，通过。
   - 新旧模式版本边界：S1 与 R4，缺游标/已有游标/other 游标均覆盖；同状态更大序号及离开再进入：R2/R3。
   - 前导零、闰日、跨年、日期空串不适用：本功能不解析用户日期或数字字符串；事件序号由内核整数提供。

## 剩余工作

在允许本地监听的执行环境重跑设计 M-03 完整验收命令；成功后才能标待审。浏览器横幅、系统授权及已读体验仍属于设计的人工验收清单，本轮未声称完成。宿主 ci-local 由 Autome 执行。
