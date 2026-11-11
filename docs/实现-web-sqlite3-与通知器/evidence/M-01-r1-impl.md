# M-01 实现轮 #1

结论：本里程碑功能验收 17/17 通过，交独立审计；Web 已有监听测试被沙箱阻断，不能宣称全量通过。本轮无上一轮审计，也无需搬入审计回归用例。实现预算 1/25。

## 实现

- `server/store.ts` 使用内置 node:sqlite，独立文件 `<home>/state/web.sqlite3`，四表、WAL、5000ms busy_timeout、逐级事务迁移。已有空版本表按版本 0 处理；高版本拒绝且关闭连接。
- `commit` 同事务落通知与游标；表达式唯一索引覆盖 NULL task_id；已读仅更新未读；账本重置同事务清除旧序号和更新游标。提供列表、偏好与按任务未读统计。
- `store-warnings.ts` 按类型和文字前缀过滤；入口在版本检查之后、main.ts 导入之前安装。无新增依赖，不接线通知器、不推进其它里程碑。

## 命令与结果

命令均在仓库根单独执行，长输出在 `.autome/output/`；以下退出码为对应命令本身的退出码。

| 命令 | 退出码 | 结果与原始输出 |
|---|---|---|
| `npm test --prefix apps/web -- test/store.test.ts test/store-warnings.test.ts` | 0 | 2 文件、17 用例全过、0 跳过；`m01-target-final.txt` |
| `npm test --prefix apps/web` | 1 | 24 文件，242 用例：215 过、11 失败、16 跳过；`m01-web-final.txt` |
| `npm run lint --prefix apps/web` | 0 | 无错误/警告；`m01-lint-final.txt` |
| `npm run typecheck --prefix apps/web` | 0 | 通过；`m01-types-final.txt` |
| `npm test --prefix apps/desktop` | 0 | 104 用例，103 过、0 失败、1 基线跳过；`m01-desktop.txt` |

与上一轮设计实测基线相比：Web 225 → 242，净增 17，来源为 store 14 + warnings 3，文件 22 → 24；未删除或修改已有测试。Desktop 104 → 104，差 0，保留原有 1 跳过。Web 的 16 跳过来自 server/eventbus-live 的启动前置失败，不是主动设置 skip；gateway-http 11 例失败为 `listen EPERM: operation not permitted 127.0.0.1`，server/eventbus-live 启动日志还含 `File watcher error: EMFILE: too many open files, watch`。这 27 例未取得通过证据，需审计或宿主环境复验；本轮两次全量结果一致。没有将环境阻断标作协议失败，没有弱化测试。

额外实测 `/opt/homebrew/opt/node@22/bin/node --input-type=module -e "import { filterSqliteWarning } from './apps/web/server/store-warnings.ts'; filterSqliteWarning(process); await import('node:sqlite'); process.emitWarning('其他实验警告', 'ExperimentalWarning');"` 退出 0：实际 SQLite 导入告警被过滤，其他实验告警保留，见 `m01-node22.txt`。这不替代 H-06 完整服务器人工启动验收。

## 标待审之前的自审清单

### 1. 情形表逐行

对照方案 §2 与 M-01 验收契约，全部在项目测试体系执行：

| 情形 | 检查及结果 |
|---|---|
| 首次建库 | 四表列名逐列比对、版本 1、WAL、空通知/缺失游标；通过 |
| 重开 | 通知、游标、偏好保留；通过 |
| 逐级迁移 | 注入第二级建表；重跑使用无效 SQL 也不执行已完成级；空版本表从 0 升级；通过 |
| 失败回滚 | 第二级先建表再执行错误 SQL，表不存在且版本仍为 1；通过 |
| 高版本 | 版本 9 重开抛包含真实库路径的中文错误，版本及通知不变；通过 |
| 有任务去重 | 同键返回 undefined，游标仍前进；异任务/异序号可写；通过 |
| 无任务去重 | NULL task_id 同键仅一条且仍存 NULL；异序号可写；通过 |
| 原子提交 | UPDATE 游标触发器 RAISE，通知不落库、游标保持 0；解除后可重试；通过 |
| 列表与计数 | 201 条按 rowid 倒序，默认 50、上限 200、单条与空结果；通过 |
| 单条已读 | 只改选定条目，重复/未知/空 ids 不改，read_at 不覆盖；通过 |
| 全部已读 | 只返回实际改变 id，再次调用返回空、未读为 0；通过 |
| 账本更换 | 失败触发器证明事务回滚；成功后旧 seq 为 NULL，保留 id/已读，新同键可写；通过 |
| 偏好 | 缺失、空键空值、中文覆盖；通过 |
| 按任务未读 | kind 筛选、已读排除、NULL 任务排除、空 kind 集合；通过 |
| 告警 | 子进程安装/不安装对照、其他类型/代码保留、Error 形式过滤、入口安装顺序；通过 |

### 2. 设计点名的失败分支

逐级迁移 SQL 失败、高版本拒绝、游标写失败均有上述实际检查；账本重置写游标失败额外检查整事务回滚。拒绝授权、外部服务不可达、RPC 超时不适用：M-01 无授权或外部调用，属于后续里程碑。一般磁盘不可读未由本里程碑指定专门降级策略，SQLite 异常原样传播；不将这项称为已实跑的磁盘故障验收。空输入的 ids/kinds 和缺失偏好已跑。

### 3. 全部状态迁移

数据库：不存在→v1、空版本表→v1、v1→v1（重开）、v1→v2、v1→失败仍 v1、未来版本→拒绝保持原状均通过。
通知：不存在→未读；重复→保持原条目；未读→已读；已读→已读不覆写时间；单条/全部/空集合均通过。
游标：缺失→0、0→事件序号、重复通知时继续推进、更新失败保持原序号；账本重置成功/失败两路通过。
偏好：缺失→新值→覆盖值通过。无多通道状态机（不适用，本轮不接 WebSocket）。

### 4. 输入域边界

NULL task_id 与单元素通知、事件序号 0、list limit 缺省/0/1/2/200 上限（传 999 得 200）、201 条数据、空 ids/kinds、重复/不存在 id、空偏好键和值均通过。日期来自系统 ISO 时间而不是外部日期输入，前导零/闰日/跨年解析不适用。HTTP limit 的非法值校验属于 M-03，本轮不新增 HTTP 行为。

## 负向对照（真实变异，已还原）

在已提交实现上依次改动单一行为，各自运行 `npm test --prefix apps/web -- test/store.test.ts -t '<以下用例名片段>'`：

| 变异 | 用例名片段 | 退出码/证据 |
|---|---|---|
| 索引改为普通 `(kind, task_id, event_seq)` | `task_id 为 NULL` | 1，`m01-negative-dedupe.txt`，重复返回第二条而非 undefined |
| 移除未来版本拒绝检查 | `库比代码新` | 1，`m01-negative-version.txt`，预期抛错而未抛错 |
| 插入后 COMMIT，再 BEGIN 写游标 | `commit 注入游标写失败` | 1，`m01-negative-atomic.txt`，失败后残留通知 |

每次从原文生成变异，finally 写回原文。恢复后 `git status --porcelain > .autome/output/m01-restored-status.txt`，文件 0 字节（工作区干净）；然后再次运行两文件验收，17/17 通过、退出 0。上述负向用例筛选产生的其余未选中测试不计入全量跳过统计。

## 交接

实现代码已提交；证据与状态随后单独提交。M-01 标待审，等待独立审计；M-02 及之后保持开放。本轮不启动下一会话。阻塞仅为已有 Web 监听回归的沙箱限制，未修改 M-01 验收契约。
