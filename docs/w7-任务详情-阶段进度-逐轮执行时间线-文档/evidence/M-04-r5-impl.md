# M-04 · 实现 #5

结论：功能已实现；验收的进程收尾检查被沙箱 `spawnSync ps EPERM` 阻塞，M-04 保持「开放」，不标待审。implementation-round 为 5/30。

## 修改与修复

- 新增 Markdown 子集解析器和 React 渲染组件：标题、段落、列表及一层嵌套、表格、围栏、引用、分隔线、行内格式；原始 HTML 作为文字，链接仅允许 http/https/mailto，图片不加载，无 HTML 字符串注入。
- loader 服务端解析文档；任务卡、固定文档、证据分组经 GET 名称链接切换。显示缺文档、缺任务卡、读盘错误和截断说明；只在任务 slug 对应的设计文档「## 里程碑」下高亮当前项。没有改动按本次内核清单取路径的安全边界。
- 首次真实测试错误地修改磁盘任务卡后期待 brief 更新；实测 brief 为创建时保存的正文。已删除该写盘行为，直接对既有真实夹具的 `<b>raw</b>` 和 javascript 链接断言。最初 14/16 通过，修正后功能测试通过。
- 定向类型检查发现文档联合类型的可选字段访问问题，显式声明 loader 的文档结果字段后通过。
- 上一轮审计 M-03 已通过，没有审计退回缺陷，故「原样搬入审计检查并红绿复现」不适用。

## 命令、结果与基线

1. 里程碑命令：`cd apps/web && npx vitest run test/markdown.test.ts test/task-docs-view.test.ts test/task-docs-real.test.ts`。
   总计 16 例，15 通过、1 失败、0 跳过，另有同原因 afterAll 错误；失败仅为「收尾无存活进程」调用 ps 被拒。日志 `.autome/output/m04-acceptance-final.txt`。真实内核的安全渲染、高亮与拒绝路径两例通过。没有弱化或跳过进程检查。
2. 关联子集：`cd apps/web && npx vitest run test/task-docs.test.ts test/task-detail-data.test.ts test/task-page.test.ts`：12 例全部通过。日志 `.autome/output/m04-related.txt`。
3. 本轮不同用例共 28 个（27 通过、1 环境失败），相比上一轮 M-03 验收 37 例少 9；原因是按本轮范围换为文档 16 例加取数/路由 12 例，没有删除上一轮任何测试。相同文档子集此前为 12 个既有用例，本轮新增 16 个，增量 +16；契约要求的 M-04 总数 16 无偏差。重复运行和负向对照不重复计数。
4. 两项负向对照均返回退出码 1：把白名单替换为只拒绝小写 javascript 前缀，markdown 链接测试失败；把所有表格都标为里程碑表，task-docs-view 高亮范围测试失败。修改均在 finally 中还原，日志 `.autome/output/m04-negative-protocol.txt`、`m04-negative-highlight.txt`；最终验收重跑正常源码。
5. 对新增 Markdown、文档组件、loader 和三份新测试执行定向 `npx eslint <这八个文件>` 通过；定向 `npx tsc --ignoreConfig --noEmit --strict --skipLibCheck --target es2023 --module nodenext --moduleResolution nodenext --jsx react-jsx --allowImportingTsExtensions <这八个文件>` 通过。日志 `.autome/output/m04-eslint.txt`、`m04-types.txt`。首次 tsc 指定文件要求显式 ignoreConfig，已按提示重跑。`git diff --check` 通过。
6. 未运行全量 CI、宿主快检、全量 test/build。未做浏览器视觉验收，H-06 等原有人工项保留，本轮仅声明服务端渲染证据。

## 标待审之前的自审清单（本轮执行结果；因环境失败未标待审）

1. **情形表逐行**：任务卡首位与固定文档顺序、证据分组、Bug 空清单说明、完成任务空清单说明均由 docs-view 第 1 例通过；所有约定 Markdown 块级/行内子集由 markdown 第 1、2、7 例通过；原始 HTML、协议白名单、图片由第 3–6 例通过；只高亮设计文档指定章节当前行由 docs-view 第 2 例和真实第 1 例通过；null brief 回退、空字符串保留由 docs-view 第 3 例及既有 detail-data 第 3 例通过；GET 编码名称由 docs-view 第 4 例通过；截断文案与未闭合围栏由 docs-view 第 3 例、markdown 第 8 例通过。
2. **失败分支**：路径形状拒绝（五种，真实与单测）、未知/空名字不读盘、文件消失、非普通文件/符号链接、非法上限、错误原文展示均通过已有 task-docs 和新 docs-view；内核离线/超时由关联 detail-data 既有用例通过。无外部 Markdown 服务，不适用外部服务不可达。进程清理失败分支实际触发 EPERM，已记录环境阻塞，没有声称无残留进程；core.stop 已先执行，临时夹具目录因无法完成进程核对而保留。
3. **全部迁移**：文档无独立状态机，选择由 URL 与本次 loader 决定；任务卡→设计/其它文档→任务卡的各个选中分支由 docs-view 第 2 例检查，只有设计文档高亮；读取成功、名字拒绝、清单缺失、文件消失分别由关联读取测试和视图错误测试通过。证据组按当前名称展开，数据随 loader 重验更新；真实鼠标切换未取证，保留 H-06。
4. **输入域边界**：空 Markdown、单行、空表格单元格、未闭合围栏、大小写/空白/控制字符协议、相对路径/锚点、Unicode 名称与 URL 编码均通过；既有读取测试覆盖空文件、正好 200 KB、超限换行截断及多字节尾部不损坏。日期、闰日、跨年不适用：本里程碑不解析日期。

## 下一步

由可执行 ps 的审计/后续环境复跑原验收命令并确认进程清理；通过前 M-04 保持开放。不得把当前证据等同于整项验收通过。
