# M-01 实现轮 #2（快检修复）

本轮只修复宿主快检退回，M-01 保持待审，不推进 M-02。无上一轮审计文件；本次缺陷由已有项目 typecheck 检查捕获，不另造重复的类型检查脚本。

## 修复与回归

- `Result.body.error` 是可选字段，不能直接赋给必填的 `BoardData.error`。用空值合并提供 `BOARD_READ_ERROR`，中文集中在 labels.ts；已有错误原文（包括空字符串）不被替换，不改网关契约。
- board 测试的两个同类网关替身都改为显式 `KernelGateway` 类型，补齐 write（调用立即抛错），移除类型断言；不使用 unknown 双重断言规避检查。
- 在原 board (7) 内增加缺省 error 的页面断言。原 503/409 与未接入网关检查保留。

## 命令与结果

1. 红：原样运行失败检查 `npm run typecheck --prefix apps/web`，复现 TS2322 和 TS2352 两处诊断，日志 `.autome/output/m01-r2-typecheck-red.txt`。未修改检查命令；它已经在项目 package.json 的 typecheck 中持久化。
2. 绿：运行用户指定快检 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`，退出 0。最终代码再次运行同一命令，日志 `.autome/output/m01-r2-quickcheck.txt`。提交后以 Autome 重跑结果为准。
3. M-01 验收：`cd apps/web && npx vitest run test/board.test.ts test/styles.test.ts test/bell-view.test.ts test/live-routes.test.ts test/task-columns.test.ts`，5 文件、总数 40，40 通过、0 失败、0 跳过。日志 `.autome/output/m01-r2-acceptance.txt`。
4. 相对上一实现轮最终基线 **40 → 40，差 0**：board 8、styles 11、bell-view 8、live-routes 7、task-columns 6；新增边界断言并入 board (7)，无删减或跳过用例。重复运行不累加唯一用例数。快检属于静态检查，不计作测试用例。
5. `git diff --check` 通过；未跑 `sh scripts/ci-local.sh`、build 或全量测试。

## 自审清单

### 1. 情形表逐行

- draft / queued / queued resume：board (1) 与 task-columns 验证待开始，通过。
- active implement / gate：board (1) 验证进行中，通过。
- 四个等待节点、paused / stopped / failed：board (1) 验证等你处理，通过。
- done：board (1)/(3) 验证已完成；cancelled / 未知 / 空状态不显示，通过。
- 列名、列序、数字 id 与完成时间排序：board (1)/(2)/(3)，通过。
- 全部、需求、Bug 筛选不改变归属与分母：board (4)/(5)，通过。
- 标题、筛选按钮、新建链接、历史折叠展开：board (6)，通过。
- 内部滚动与样式约束：styles 11 项通过；不是浏览器布局证据。

### 2. 设计点名的失败分支

- 503 内核离线、409 项目读取拒绝原文及未接入网关：board (7)，通过；新增无 error 时中文回退断言，通过。
- 空任务输入：board (1)/(6)，通过。
- 独立读盘、超时、上传及写授权不适用：M-01 无新增此类通道，消费网关返回结果。

### 3. 全部状态迁移

- all / feature / bug 的 3×3 筛选组合：board (4)，通过。
- 折叠→展开→收起：board (6)，通过。
- 内核状态写迁移不适用：本轮只读看板；真实事件迁移属于 M-05。

### 4. 输入域边界

- 14 天整、14 天加 1 秒、null / 空完成日期、跨年、闰日：board (3)，通过。
- 空列表、单元素、未知 / 空状态、T-010 前导零排序：board (1)，通过。
- 网关错误字段 undefined：board (7)，通过。
- 最大数量不适用：M-01 没有业务数量上限声明；真实浏览器滚动与视觉仍由 H-01/H-07 人工验收。

## 交接

M-01 保持待审，implementation-round 为 2/25；由宿主复跑快检并调度审计。无新里程碑实现，无全量门禁证据。
