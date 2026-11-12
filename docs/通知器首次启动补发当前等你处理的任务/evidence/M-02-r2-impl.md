# M-02 · 实现 #2

## 范围与实现

本轮仅推进 M-02。事件路径从面板库读取最近状态，将状态变化（包括离开）、通知和游标原子提交；补发复用现有规则与五键推送。首次启动和换账本均执行最多三次序号—快照—序号对齐；已有 v1 库从旧游标之前的历史播种，不从当前快照或通知行推断。新增共享 dashboard 替身，顶层五键及 waiting 六键依据 dispatch.rs 的 dashboard_get；真实接口核对留给 M-03。

上一轮审计 M-01 通过，无退回缺陷，无需搬入审计红绿检查。开发首跑 N5 因测试夹具重复关闭数据库失败；改为在打开当前 store 之前创建真正 v1 库，后续通过，未改产品逻辑绕过失败。

## 验收命令与结果

- `cd apps/web && npx vitest run test/store.test.ts test/notifier.test.ts test/notifier-rules.test.ts test/notifier-api.test.ts --reporter=verbose`：54 个全部通过，0 失败、0 跳过。分项 store 18、notifier 17、rules 12、api 7。原始输出 `.autome/output/m02-final-tests.txt`。
- 同范围上一轮基线 48（18 + 11 + 12 + 7），本轮 **+6**，全部来自新增 N1–N6；既有首次启动用例改写，其余十条保留。上一轮 M-01 验收子集为 29，本轮相同两文件为 35（+6）；本轮增加验收 rules/api 的 19 条不计为新增测试。参数化 rules 的运行数为 12，不能用顶层 test 数 5 替代。
- `npm run lint`、`npm run typecheck`：通过；输出 `.autome/output/m02-final-lint.txt`、`.autome/output/m02-final-typecheck.txt`。
- `git diff --check`：通过。
- `git diff main --stat -- apps/web/server apps/web/app`：main 已前进，出现本分支未包含的并行 UI 改动，不能作为本任务改动范围的判据。原始结果 `.autome/output/m02-main-stat.txt`；改用共同祖先 `404b48156c8c2df1485b2b6788d5156b511754c8`，`git diff $(git merge-base main HEAD) --stat -- apps/web/server apps/web/app` 仅列 store.ts、notifier.ts，符合范围。rules/api 测试与 main 无差异。
- 未运行 M-03 的真实账本、全量测试和 build；未取得人工浏览器/系统横幅证据。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行

D3 竞态表六行由 N3 覆盖：稳定 W 得 1；快照后 X→W 得 2；X 后进入 W 得 1；状态先写、事件后写得 1；快照前离开重取后得 0；快照后 W1→W2 得 2。评审两条反例（merge_gate→await_merge、W1→X→W2）重取后仅 1 条、序号 3；三次不稳定不落库不推送，下一次恢复补 1 条。

D8：首次启动改写为只补当前等待，历史 done 不补，空账本游标 0；其他十条原测试断言保留。N1 同时覆盖五种等待、done/cancelled/implement 排除；不调用 task.get；来源 backfill、标题/正文/项目字段和五键消息均检查。

D4 三任务：游标前 W + 游标后同 W 不发；游标前 X + 游标后 W 发；游标前 W→X + 游标后 W 发。跨分页播种，无 dashboard 调用，无 backfill 行，播种标记删除。N6 换账本旧行序号 NULL、新补发行序号等于重取后的 2。

### 2. 失败分支

- 带 code 拒绝：既有用例断言仍生成空补齐字段通知；无 code 离线/超时：保留重试位置，恢复后通知一次。
- 三次快照调用中任一步抛错或 close：N4 逐个注入，游标缺失、0 行、0 推送；重开恢复成功。
- 首次补发游标写失败：N4 触发器验证多条通知与状态全部回滚；解除后两条正常补发。
- 播种分页读失败、close、删除 seed 写失败：N5 逐个验证不留部分状态、seed 保留；重启恢复后不重发。
- 换账本快照失败：N6 验证旧游标/状态/通知序号保留，恢复后再对齐并补发；播种发现换账本也转入补发。
- 空输入：首次启动空账本通过。外部服务不可达使用 RPC 替身拒绝验证，不声称真实断网验收。

### 3. 全部迁移

- 未初始化→补发成功→事件续拉；未初始化→失败/关闭→重开重试：首次用例与 N3/N4。
- W→W（含重启和 JSON 键顺序变化）不发且保留进入序号：N1。
- W→X→W、W1→W2→W1，分别在线及停服期间：N2；中间状态逐一为 implement、await_info、await_design_approval、not_a_bug、failed。
- 旧库待播种→成功删除标记；失败/关闭→保留标记→重试；待播种→换账本：N5。
- 已初始化→换账本→快照失败保留旧状态→对齐补发：N6；既有换账本用例继续检查缓存清理。
- project.alert 独立事件路径及唯一索引、并发 pull 合并、close 后停止调用：原用例通过；项目不进入任务状态机。

### 4. 输入域边界

空账本 seq=0、单任务、多个等待、分页 pageSize=2/跨页、MAX_SAFE_INTEGER 序号、带前导零的任务 id T-001、JSON 键顺序不同均通过。状态域覆盖五种等待及 done/cancelled/implement。日期、闰日、跨年不适用：本里程碑不解析日期输入。无新增接受任意空 RPC 返回的约定，RPC 形状依据真实协议。

## 负向对照

脚本 `.autome/output/m02-negative.cjs` 临时变更产品源码，逐次运行指定 N 用例并在 finally 还原。四项均 exit 1 且为 AssertionError：不读持久化状态→N1；不记录无通知的离开→N2；去掉 L1=L0 确认→N3；不播种→N5。日志在 `.autome/output/m02-negative-*.txt`，汇总 `.autome/output/m02-negative-summary.txt`。恢复后再跑完整 M-02 验收。未宣称执行设计中其余 mutation 变体。

结论：M-02 待审；M-03 保持开放。
