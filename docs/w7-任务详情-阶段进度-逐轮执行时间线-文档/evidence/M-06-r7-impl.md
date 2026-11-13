# M-06 实现轮 7/30

结论：实现已落地，M-06 保持开放。验收功能部分通过，但监听与进程检查受沙箱 EPERM 阻塞，不能标待审。没有改内核、门禁、协议、其它 worktree 或远端。

## 实现范围

- 新增改动模型与卡片：提交数、文件清单、增删行数、提交标题；只删不增的疑似删除计数及两种醒目阈值；rebase、未提交路径、纳入 Backlog 提示。
- 合并进度按实际任务状态显示，只有 done 才将“已完成”标为当前；合并段失败显示原文。提交 action 仍返回“已提交合并”。
- 新增待你决定面板：未消费条目、当前表态、carried；Backlog 纳入/忽略/撤销，争议裁定/撤销；禁用写入遵循 WriteGate，裁定上限取 MAX_PROSE_LENGTH。
- 路由重验扩为八个前缀；保留时间线位置与 key，右栏 GET 查询参数切换文档/改动/表态；文档链接显式携带 tab=docs，避免合并段默认标签吞掉文档选择。
- 两栏定高与内部滚动沿用现有 token。新增真实内核合并与实时测试，沿用进程清理机制，未绕过环境失败。

## 命令、结果与总数

1. 监听探针 `node -e 'const s=require("net").createServer();s.on("error",e=>{console.error(e.code);process.exitCode=1});s.listen(0,"127.0.0.1",()=>s.close())'`：退出 1，EPERM。
2. 里程碑验收：`cd apps/web && npx vitest run test/task-changes.test.ts test/task-decisions.test.ts test/task-merge-real.test.ts test/task-live-real.test.ts test/task-page.test.ts test/styles.test.ts`：退出 1，**6 文件、33 用例：29 通过、1 失败、3 未执行**。原始输出 `.autome/output/m06-acceptance-final.log`。
   - task-merge-real 的 3 个功能用例全过，收尾用例失败于 `spawnSync ps EPERM`，afterAll 同样被拦。功能证据包括真实 changes/rebase 提示、B-01/D-01 经 action 更新与撤销、main_check 门禁挂住时不是 done、释放后进入 done 和事件先后顺序。
   - task-live-real 的 beforeAll 探针报 `listen EPERM`，3 条测试被 Vitest 标 skipped，不是主动添加 skip；afterAll 的 ps 也被拦。WebSocket 时延与八类真实事件矩阵未取证。
   - 上轮 M-05 验收基线为 **110**，本轮 **33，差 -77**：切换了里程碑验收文件集合，没有删除 M-05 测试。同一 M-06 范围在本轮前已有 task-page 3 + styles 12 = **15**；本轮新增 **18**（changes 5、decisions 3、merge-real 4、live-real 3、page 3）。比设计预期 32 多 **1**，是完整路由三个标签与文档链接接线用例。
3. 直接相关子集：`cd apps/web && npx vitest run test/task-changes.test.ts test/task-decisions.test.ts test/task-page.test.ts test/styles.test.ts test/labels.test.ts test/task-docs-view.test.ts test/task-detail-data.test.ts test/task-action.test.ts`：退出 0，**8 文件、47 用例全过**，见 `.autome/output/m06-related-final.log`。本子集本轮前 36，本轮增加 11（changes 5、decisions 3、page 3）；与验收集合有重叠，不将两者相加冒充用例总数。
4. `git diff --check`：退出 0。

本轮没有跑 ci-local、宿主快检命令或 Web 全量，遵守本轮用户限定；lint/typecheck 由宿主快检执行，不能写为本轮已通过。没有跑 main 版本兼容复验（设计要求审计轮执行）。真实浏览器 H-01..H-07 未取证，仍由人工清单承担。

## 标待审之前的四项自审（已执行，阻塞项如实保留）

1. **情形表逐行**：
   - changes 可用清单、提交与行数、提交标题：真实回包单测与真实内核通过。
   - available=false / 无 changes：空态通过；读取拒绝、离线、超时、EACCES 原文渲染通过。
   - 疑似删除计数及阈值：单测通过，二进制 0/0 不误算；文案注明清空文件也可能计入。
   - rebase_first、repos.behind 两条来源：单测通过，真实落后分支通过；mergeable=false 不承诺自动合并且显示阻挡路径。
   - pending_decisions.included：模型与真实 action 写回后的卡片提示通过。
   - 未消费条目、四种 disposition、carried、消费后隐藏：逐项渲染通过。
   - Backlog 三种按钮、争议裁定/撤销、离线禁用、下一轮生效：通过；撤销带 formNoValidate，避免空裁定阻挡撤销。
   - 八种前缀本任务/其它任务、project 事件不匹配：纯重验测试通过；真实 WebSocket 矩阵未执行（EPERM）。
   - 四个现有入口与路由、两栏/内部滚动 CSS、完整路由三个标签及文档选中：通过。真实尺寸与滚动保持未取证（H-03/H-04）。
2. **失败分支**：changes 拒绝授权、读失败、内核离线、超时均以原文渲染测试通过；裁定空串/空白/超长不调用网关通过；合并每个节点失败保留 failureText 且不提前完成通过。真实外部网络服务不适用，本页没有新增外部服务调用。实际监听与 ps 的环境拒绝已保留为失败，不改断言或跳过。
3. **全部迁移**：await_merge → rebase/merge_gate/merging → main_check/cleanup → done 的节点映射逐项通过；上述每个合并节点 → failed 逐项通过。真实合并路径在 main_check 挂住、释放后 done 通过。Backlog none/include/ignore、争议 none/ruled 的显示和表单值检查通过，真实 include/ruled → none 通过；表态不会中断当前轮。八前缀 × 本任务/其它任务已跑纯函数，真实通道仍阻塞。未改内核状态机。
4. **输入域边界**：删除数/总数 0/0、1/1、2/4、2/5、4/10、5/11 及二进制 0/0 通过；裁定空串、空白、8000、8001 字符通过；空条目列表通过；无 tab、有效 tab、doc 参数与合并默认标签通过。前导零、闰日、跨年不适用：本轮未增加数值/日期文本解析入口。

## 缺陷判别与自查修复

上一审计为通过，无退回实现缺陷，无必须逐字移植的红绿检查。B-08 是审计记下的验证增强项，本轮不改变已关闭的 M-05。

自查先发现 TSX 中出现“合并”“进行中”而触发 labels 守卫，已移至 .ts 文案/模型，最终 47 条相关子集全绿。另在完整路由验证中锁住合并段的文档链接，避免默认 changes 标签盖过文档选择。

三项负向对照（脚本均 finally 恢复原文件；本轮未提交前正常存在实现 diff，不能声称 git status 为空）：

- 将 main_check 提前判作 done：`npx vitest run test/task-changes.test.ts`，5 条中 1 失败，退出 1；输出 `.autome/output/m06-negative-early-done.log`。
- 将前缀退回 task/session：`npx vitest run test/task-page.test.ts`，6 条中 1 失败，退出 1；输出 `.autome/output/m06-negative-missing-prefixes.log`。
- 删除时间线 overflow-y：同上，6 条中 1 失败，退出 1；输出 `.autome/output/m06-negative-timeline-overflow.log`。

还原后 47 条相关子集通过；验收仍仅剩环境阻塞。真实 main_check 的提前完成负向注入未另跑，当前以模型负向与真实正向证明区分，不扩大声称范围。

## 后续

在允许监听 127.0.0.1 和执行 ps 的会话原样复跑 M-06 验收；取到全部通过证据后再标待审。进程收尾先关闭服务与 core，再由既有清理器检查；ps 被拦时临时目录保留，不声称“无残留进程”。
