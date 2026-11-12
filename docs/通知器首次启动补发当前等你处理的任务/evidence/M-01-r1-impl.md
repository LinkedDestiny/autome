# M-01 实现轮 #1 证据

范围：仅 `apps/web/server/store.ts`、`apps/web/test/store.test.ts` 及本轮任务记录。无上一轮审计，故无需搬入审计发现的回归检查。未推进 M-02/M-03。

## 实现

- 原第 1 级迁移保持不变；追加 source、notifier_task_state 与有条件的 notifier_seed 迁移。
- commit 原事务增加任务状态 upsert；backfill、seedTaskStates、resetLedger 都以一个事务提交；返回实际新增通知。
- list 显式选择原九列；NotificationInput/Notification 不变。resetLedger 保持两参数调用兼容，清理状态及播种游标，可选写入新账本补发。
- 按 D8 调整既有建库、注入迁移、迁移失败、新版本拒绝的版本断言；新增 S1–S4。notifier.test.ts 未改。

## 命令与结果

- 初次 npx 因依赖未安装而联网失败；随后 `cd apps/web && npm ci` 成功（退出 0），记录 `.autome/output/m01-install.txt`。不更改依赖清单和锁文件。
- 在当前 worktree 临时还原 HEAD 的 store.ts/store.test.ts 跑基线，随后恢复实现：`cd apps/web && npx vitest run test/store.test.ts test/notifier.test.ts`。25/25 通过：store 14、notifier 11。原始输出 `.autome/output/m01-baseline.txt`。
- 最终验收命令：`cd apps/web && ([ -d node_modules ] || npm ci) && npx vitest run test/store.test.ts test/notifier.test.ts && npm run lint && npm run typecheck`。29/29 通过：store 18、notifier 11；失败 0、跳过 0，lint 零警告、typecheck 通过。输出 `.autome/output/m01-acceptance-final.txt`。
- 与上一轮基线的差：此前无实现轮，使用本轮实跑的改动前基线 25；本轮总数 29，净增 4（S1–S4），无删除或跳过。多个分支在这四个用例内验证。
- `git diff --check` 通过；`git diff --quiet -- apps/web/test/notifier.test.ts` 通过。

## 标待审之前的自审清单

### 1. 情形表逐行

M-01 对应 D1、D7 与 S1–S4；D3 竞态情形表属于 M-02，本轮不适用。

| 情形 | 检查与结果 |
|---|---|
| 新建库 | 既有建库用例：五表、source 列、版本 2、WAL、无播种游标，通过 |
| v1 有 notifier 游标 | S1：播种游标等于原游标 7，旧通知仍一条且 source=event，通过 |
| v1 无 notifier 但有 other 游标 | S1：other 保持 7，不产生 notifier_seed，通过 |
| 列表保持九列 | S1 精确比对键集合，通过 |
| 状态读取、upsert、重开 | S2：缺失为 undefined，写入对象后关闭重开仍深相等，更新为 null，通过 |
| commit 唯一键冲突 | S2：返回 undefined，仍更新状态；原去重用例继续通过 |
| 多条补发 | S3：两任务同序号均插入，source=backfill，返回行与 list 一致；重复调用返回空数组，通过 |
| 播种不通知 | S1：状态写入、seed 删除、旧通知数不变；空集合可调用，通过 |
| 换账本 | S4 与原 resetLedger 用例：旧序号置空，清理状态及 seed，保留已读和偏好；可选补发返回新行，两参数调用兼容，通过 |

### 2. 失败分支

- commit 游标更新失败：S2 触发器阻断更新，新增通知、状态改变和游标全部回滚，通过。
- backfill 游标首次插入失败：S3 触发器阻断插入，两条通知、两条状态和游标均未留下，通过。
- seed 删除失败：S1 触发器阻断删除，状态写入回滚，seed 保留，移除触发器后重试成功。
- resetLedger 游标更新失败：S4 旧通知序号、旧状态、seed 均保留，新补发及新状态均回滚，通过。
- 迁移 SQL 失败、库版本过新：既有用例验证整级回滚及中文错误，通过。
- 拒绝授权、快照读失败、超时、外部服务不可达：不适用；M-01 仅提供本地同步存储，RPC 分支属于 M-02。

### 3. 全部迁移

- schema 0→2（新库、空 schema_version）、1→2（有/无 notifier）、2→2（重开）、2→注入级成功、注入级失败回滚、较新库拒绝：既有迁移用例及 S1 全部通过。
- 任务状态：缺失→对象→重开读取→失败更新保持原值→成功更新 null；S2 通过。
- 播种：seed 存在→失败仍存在且无状态→成功删除并有状态；seed 不存在/空批次调用；S1 通过。
- 换账本：旧状态→失败保留→清空并补入新状态→无补发清空；S4 通过。
- 通知器 W→X→W 等业务迁移不适用本里程碑，留给 M-02 的持久化去重逻辑及 N1–N6。

### 4. 输入域边界

- 空补发集合、空播种集合：S1/S3 通过；单条补发：S4；多任务共享序号及重复键：S3。
- seq=0：S3；Number.MAX_SAFE_INTEGER、空 task_id、空字符串状态：S2；对象/null 状态与缺失状态区分：S2。
- 原 list 的 limit=0/1/超过上限及 NULL task_id 去重用例保持通过。
- 前导零、闰日、跨年：不适用，存储接口不解析日期或数字字符串，日期由内部生成。

## 设计指定负向对照

执行 `node .autome/output/m01-mutations.cjs`，每次仅变异一个位置并单跑对应项目用例，脚本 finally 恢复源码。四项均产生预期的 1 个失败（其余 17 条因 -t 过滤未运行）；这些过滤运行不计入验收总数。

| 变异 | 检查结果 | 原始输出 |
|---|---|---|
| 播种 INSERT 去掉 WHERE | S1 other 分支错误生成 seed=7，失败 | `.autome/output/m01-mutation-seed-where.txt` |
| list 恢复 SELECT * | S1 键集合多出 source，失败 | `.autome/output/m01-mutation-list-star.txt` |
| 补发写在游标事务之外 | S3 回滚后残留通知，失败 | `.autome/output/m01-mutation-partial-backfill.txt` |
| resetLedger 不清任务状态 | S4 旧状态残留，失败 | `.autome/output/m01-mutation-stale-reset.txt` |

恢复后完整重跑最终验收，29/29、lint、typecheck 均通过。全量 npm test、build、真实账本验收属于 M-03，本轮未跑。宿主门禁交 Autome 执行。

结论：M-01 可标待审，等待审计独立复验；无阻塞。
