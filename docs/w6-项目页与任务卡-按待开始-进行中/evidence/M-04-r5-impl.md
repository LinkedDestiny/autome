# M-04 实现 #5

结论：验收通过，申请待审。本轮仅推进 M-04，无上一轮审计退回项（M-03 已通过），因此无需要逐字搬运的审计缺陷检查。

## 修改

- 登记新建页；根 loader 提供项目名，无新增 loader。类型选择、Markdown 文本框、顺序上传/移除附件、隐藏编号、双提交按钮及错误原文显示均接入；写闸门、上传中、提交中及未移除的失败附件阻止提交。
- action 验证占位标题、UTF-8 字节、附件数量、存在性和清洗后重名；只经 gateway.write 创建，成功回项目页。不删暂存附件。
- 两套模板遵守 R-03；切换保留编辑正文并提供明确覆盖按钮。样式使用已有颜色/圆角 token、内部滚动。
- 补充新建页 CSS 守卫（已有 styles 文件增加 1 项），其它既有行为不改。

## 命令与用例基线

- 改前：`cd apps/web && npx vitest run test/styles.test.ts test/live-routes.test.ts`：18/18，通过，0 跳过；日志 `.autome/output/m04-baseline.txt`。
- M-04 验收：`cd apps/web && npx vitest run test/task-new.test.ts test/task-new-real.test.ts`：2 文件、12/12，通过，0 跳过，退出 0；日志 `.autome/output/m04-acceptance.txt`。该范围上一轮不存在，基线 0，增加 12（task-new 9、task-new-real 3）。与上一轮 M-03 的 37 项是不同验收范围，不能当成减少 25 项。
- 相关子集：`cd apps/web && npx vitest run test/styles.test.ts test/live-routes.test.ts`：19/19，通过，0 跳过，相对基线 +1（新建页滚动约束）；日志 `.autome/output/m04-related.txt`。本轮去重总数 31，相对同范围基线 18 增加 13。
- 新增/修改 TS 文件定向 ESLint 退出 0；`cd apps/web && npx tsc --noEmit` 退出 0；`git diff --check` 通过。未跑宿主快检组合或全量门禁，也未跑 Desktop/全量 Web。
- 真实内核首次完整成功组 setup/tests/cleanup 4653ms，验收复跑约 4.9 秒；用例及 setup timeout 120 秒，大于实测两倍。夹具停止自己创建的 active 任务、关闭 core 并清理隔离目录，不启动真实 Agent。

## 标待审前的四项自审

1. **情形表逐行**：task-new (1)/(2) 核对需求五节/Bug 六节和插件差异；(3) 原模板/空白/编辑正文在 feature、bug 所有方向切换；(4) 占位标题、超限、失效附件、重名；(5) 两类型 × start/draft 精确参数且无 request/doc_refs；(6) 拒绝文案 SSR 可见；(7) 表单、常量提示、关闭写闸门；(8) 成功重定向；(9) 未知项目。real (1) 进行中列和复制字节，(2) draft 待开始列且源文件仍在，(3) 内核标题错误。全部通过。
2. **失败分支**：400/409/503/504 原文返回与 SSR 展示均通过；空正文和缺少可用标题由真实内核拒绝；未知 kind/intent 不调用网关；附件不存在、重名、超字节、超个数均拒绝。写闸门关闭的提交按钮禁用且原因可见。上传的 Host/Origin、磁盘读取/流失败由已关闭 M-03 负责，本轮未修改上传服务，不重复其监听验收；本轮页面保留 uploadFile 错误并允许移除。人工授权弹窗不适用，本功能不请求系统权限。
3. **全部迁移**：feature↔bug 及同类型，对原模板/空白替换、编辑内容保留，四组合均实跑；start→进行中、draft→待开始经真实内核；成功→项目重定向，失败→原页错误经 action/SSR。显式覆盖按钮使用目标模板，上传 pending→成功编号/错误、移除按稳定 key 过滤，提交期间禁用编辑，已作源码逐支核对；没有浏览器事件交互取证，保留为 UI 人工验收边界，不将 SSR 等同鼠标验收。
4. **输入域边界**：UTF-8 65536 字节接受、多字节超过上限拒绝；附件 0/1/10 接受、11 拒绝；相同清洗后文件名拒绝；空白模板切换；空正文真实内核拒绝；未知项目明确提示。前导零/闰日/跨年不适用，本里程碑无日期或数字 ID 解析。

## 四项负向对照

用 `.autome/output/m04-mutations.mjs` 逐项注入后还原，原始输出分别在 `m04-negative-{template,switch,draft,delete}.txt`：

- Bug 最后一节改 `## 线索`：退出 1，task-new (2) 红。
- 类型切换无条件覆盖：退出 1，task-new (3) 红。
- draft 永远 false：退出 1，task-new (5)、real (2) 红。
- 成功后删除暂存文件：退出 1，real (2) 红。

四项全部还原后重新跑验收为 12/12 绿。最终提交后检查工作区为空；不把本轮尚未提交的正常修改误称为注入残留。

## 实测修正与限制

- 初跑测试 4 红/8 绿：两处 receive 测试调用形状写错；FormData 请求序列化将 LF 规范为 CRLF；普通无 Markdown 标题正文被内核接受。前两项修正测试对真实接口的假设，产品不改正文内容。
- `dispatch.rs:1370` 实际允许从首个非空行回退取标题；故缺标题真实检查使用 `###   `（与内核自身用例一致），返回设计约定的内核原文。没有为实现旧假设改动 crates 或新增网页限制。
- 未做真实浏览器布局测量/鼠标交互，H-02 保留人工验收；本轮仅 SSR、CSS 静态约束与真实内核证据。
