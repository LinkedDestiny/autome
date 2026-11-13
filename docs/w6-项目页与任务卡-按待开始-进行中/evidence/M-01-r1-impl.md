# M-01 实现轮 #1

结论：验收通过，申请待审。本轮只推进 M-01；无上一轮审计、无待搬入的审计回归检查。

## 实现与范围

- 登记 `/projects/:id`，经网关 `project.get` 加载四列；归属只调用 `columnOf`，列标题取 `COLUMN_LABELS`，数字 id 倒序、完成时间倒序。
- 全部/需求/Bug 用组件状态筛选，不触发写入、不改变原始数据与分母；完成任务含边界 14 天，旧任务或缺完成时间默认折叠，`done=all` 展开，可收起。
- 返回统一 loader 时钟，每分钟更新本地时间；看板高度受外壳约束，四列内部滚动，沿用 animal-island token。
- 铃铛项目目标开放；完整卡片和开始按钮属于 M-02/M-05，本轮复用未修改的 OverviewCard。
- 必要设计修正：原设计要求翻转项目目标，同时声称其它已有测试不改；实跑发现 bell-view 的旧测试硬编码“项目不可访问”。仅同步该用例的默认目标断言，保留任务未开放、无归属、编码与路由一致性检查，用例数仍为 8。设计测试改动说明已同步。

## 命令与结果

1. 首次基线命令因没有 node_modules，npx 尝试联网失败，未执行测试。`cd apps/web && npm ci --offline` 随后使用本地缓存安装成功，无新增依赖/锁文件改动。日志 `.autome/output/m01-install.txt`。
2. 基线：恢复 routes.ts、bell-targets.ts、styles.test.ts 的 HEAD 内容运行 `cd apps/web && npx vitest run test/styles.test.ts test/bell-view.test.ts test/live-routes.test.ts test/task-columns.test.ts`，随后还原实现文件。4 文件、31 用例通过。日志 `.autome/output/m01-baseline.txt`。新增路由文件/CSS仍在，但未加入新测试；31 为已有验收用例基线。
3. 验收命令：`cd apps/web && npx vitest run test/board.test.ts test/styles.test.ts test/bell-view.test.ts test/live-routes.test.ts test/task-columns.test.ts`。
   - 首次 40 总数、38 通过、2 失败、0 跳过：铃铛旧预期冲突；新 SSR 收起链接断言没容纳 React Router 的 data-discover 属性。日志 `.autome/output/m01-acceptance.txt`。
   - 修正后 40/40 通过；负向对照还原后的最终复跑也为 5 文件、40 用例、40 通过、0 失败、0 跳过。最终日志 `.autome/output/m01-final.txt`。
   - 相对基线 **31 → 40，+9**：board 新增 8，styles 10 → 11；bell-view 8、live-routes 7、task-columns 6 不变。无上一实现轮基线，本次以前述 HEAD 已有用例为基线。
4. 设计三条负向对照逐一临时注入，执行对应 `npx vitest run test/board.test.ts` 或 `test/bell-view.test.ts`，finally 全部还原：
   - paused 错归 running：board (1)/(4)/(8) 失败，退出 1；`.autome/output/m01-negative-paused.txt`。
   - 筛选同步缩小 total：board (4) 失败，退出 1；`.autome/output/m01-negative-denominator.txt`。
   - 项目通知仍 unavailable：默认项目链接与真实路由一致性两项失败，退出 1；`.autome/output/m01-negative-bell.txt`。
   - 这些是同一用例的变异复跑，不计入唯一用例总数增量。
5. `git diff --check` 通过。未运行全量门禁、Rust 检查、全量前端测试、lint/typecheck/build，按协议交宿主；不将本轮子集结果冒充宿主快检通过。

## 标待审前自审清单

### 1. 情形表逐行

- draft、queued、带 resume 的 queued → 待开始：board (1)，通过。
- active implement/gate → 进行中；四个等待节点 → 等你处理：board (1)，通过；task-columns (1)/(6) 另覆盖全部节点。
- paused/stopped/failed → 等你处理；done → 已完成；cancelled/未知/空状态排除：board (1)，通过。
- 列顺序/中文、数字 id 排序、完成时间排序：board (1)/(2)/(3)，通过。
- 三种类型筛选保持归属与分母、不修改原始列：board (4)，通过；录制项目夹具计数交叉核对 board (5)，通过。
- 四列标题、三按钮、新建链接、默认不渲染旧卡、展开/收起：board (6)，通过。
- 样式 token 与四列内部滚动：styles 11 项，通过；不等于浏览器布局实测。

### 2. 失败分支

- 内核离线 503、项目读取拒绝 409：board (7) 原样显示中文、不抛异常；网关未接入也显示内联错误，通过。
- 无任务输入：board (1)/(6) 返回四个零计数及空列文案，通过。
- 独立读盘、超时、外部网络连接：不适用，M-01 不新增这些通道，统一消费网关失败结果；无上传/写授权操作。

### 3. 全部迁移

- all/feature/bug 三种筛选的 3×3 组合：board (4) 均从未过滤的列派生，计数和源数据保持，全部通过。
- 折叠→展开→收起：board (6) 分别校验旧卡隐藏、出现、再隐藏，链接切换正确。
- 任务状态迁移执行不适用：M-01 只读状态；十四种状态形状全覆盖，真实事件驱动迁移由 M-05 验收。

### 4. 输入域边界

- 恰好 14 天归 recent；14 天 + 1 秒归 older；null/空完成时间归 older；跨年、闰日边界：board (3)，通过。
- 空列表、单任务、空/未知状态、T-010 前导零与 T-2/T-100 数字顺序：board (1)，通过。
- 无业务最大数量/日期上限声明，不虚构最大值验收；四列 20+ 卡的真实滚动测量仍是 H-01。

## 未取证与交接

真实浏览器视觉与 1512×944 scrollHeight 测量未执行，H-01/H-07 保持人工验收；本轮只有 CSS 静态与 SSR 证据。完整任务卡、上传、新建与事件迁移未提前实施。下一步由审计轮独立复验 M-01；下一实现目标 M-02。
