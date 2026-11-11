# M-07 实现轮 #8：补齐失败重试独立回归验收

本轮只推进 M-07，沿用 M-02 的实现与项目回归用例，不重复实现，不增删测试。上一轮审计 M-06 通过，没有退回缺陷，无需逐字迁入审计失败检查。专项基线为 11 例，本轮仍为 11 例，差值 0；离线和超时是同一用例内两个独立 fixture。

## 标待审前自审清单

1. **情形表逐行**：§4.5 / M-07 三种补齐结果逐项执行。无 code 离线、无 code 超时：前置 task.created 的 seq=1 已处理，失败事件 seq=2 未处理，持久游标保持 1、通知为空；恢复后重试该事件，再 pull 一次，恰 1 条通知、推送 1 次、游标 2。带 code 拒绝：任务与项目两种补齐均生成通知，project_id/body 为 NULL，后续事件继续处理。成功：通知写入后推送，唯一约束挡下不再推送。notifier 专项 11 例通过。
2. **失败分支**：无 code 离线与 timeout:true 超时、带 code 的 not_found 明确拒绝均执行通过；注入游标写盘失败时事务回滚，重启后只补一条；close 时在途补齐不写库。浏览器拒绝授权不适用：M-07 不调用浏览器权限 API。空输入由首次启动空账本用例覆盖；外部服务不可达由无 code 离线替身覆盖，不宣称真实网络故障验收。
3. **全部迁移**：未处理→补齐失败→游标停住→恢复→同一事件提交→再次 pull 不重推；未处理→带 code 拒绝→降级提交；已提交→重启不重发；写盘失败→回滚→重启补齐；账本更换→重置游标/状态/缓存；运行→close→拒绝后续操作。专项全部执行。提前记住状态的负向对照使“失败→恢复”迁移丢通知，现有测试能拒绝（下节）。不新增传输通道。
4. **输入域边界**：空账本初始化为 0；首次查询用 MAX_SAFE_INTEGER；失败事件前恰一条非通知事件；单条恢复只推一次；pageSize=2 下五条分页与并发 pull；相同 seq 去重、账本序号回退、项目告警无 task_id/正文为空均由专项通过。前导零、闰日、跨年不适用：本里程碑输入为数值事件序号与已解析状态，不解析日期或数字文本。

## 负向对照与恢复

命令：`npm test --prefix apps/web -- test/notifier.test.ts`。

- 临时在 `if (rule)` 前加入 `if (event.kind === 'task.updated') states.set(event.subject, payload.state);`，模拟补齐前提前更新最近状态。退出 1：11 例中 10 通过、1 失败，0 跳过。失败用例为“补齐无 code 的离线/超时错误保留游标，恢复后同一事件仍生成通知”，断言期望 1 条、实际 0 条。原始输出 `.autome/output/m07-early-state.log`。
- 使用 finally 原样恢复产品文件；未保留变异。还原后相同命令退出 0，11/11 通过，0 跳过，日志 `.autome/output/m07-target.log`。负向对照发生于循环第一个离线 fixture 即失败；还原后的通过执行了离线与超时两种 fixture。
- 写本轮文档前 `git status --short` 为空，确认没有残留产品改动。未发现需要修复的产品缺陷。

## 验收命令与结果

| 命令 | 退出码 | 结果及原始输出 |
|---|---|---|
| npm test --prefix apps/web -- test/notifier.test.ts | 0 | 11/11，0 跳过；.autome/output/m07-target.log |
| npm run lint --prefix apps/web | 0 | 无错误；.autome/output/m07-lint.log |
| npm run typecheck --prefix apps/web | 0 | 无错误；.autome/output/m07-typecheck.log |
| npm test --prefix apps/desktop | 0 | 总数 104，103 通过、1 基线跳过；.autome/output/m07-desktop.log |
| npm run build --prefix apps/web | 0 | 构建通过；.autome/output/m07-build.log |

Desktop 相对上一轮基线 104 差值 0，跳过保持基线的 1 例。Web 全量 `npm test --prefix apps/web` 退出 1：33 文件、301 例，271 通过、11 失败、19 跳过，日志 `.autome/output/m07-web.log`。相对上一轮审计基线 301 例差值 0，没有增删测试。gateway-http/notifier-live 遇 listen EPERM；eventbus-live/server 遇 watcher EMFILE，导致套件初始化失败或用例失败。没有修改测试规避环境限制；不能宣称全量通过，交审计在可监听环境独立复验。M-07 专项与负向对照通过，按既定环境处置标待审。
