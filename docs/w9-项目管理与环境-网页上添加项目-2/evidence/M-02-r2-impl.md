# M-02 实现轮 #2

结论：实现已落地，验收受监听权限阻塞，M-02 保持开放，不能标待审。implementation-round 为 2/30。

## 改动与修复

- 添加 `/projects/new`：路径补全、GET 检查、probe 事实提示、工作区与文档仓库选择、确认 action、按跳过选项跳转、内核拒绝原文和 skip 部分成功提示。
- 路径变化使确认表单消失；补全 150ms 防抖，旧响应与卸载后的响应不更新界面。部分成功后隐藏确认表单，避免重复注册。
- 原生选择按钮只在 hydration 后检测到桥接函数时显示；普通 SSR 不输出按钮。
- 侧栏底部和命令面板加入添加项目入口，沿用 tokens 和写门禁。环境入口按设计预留，目标页归 M-06；设置页归 M-03。
- 修改旧 palette 测试仅因候选末尾增加添加入口，空查询默认可见；保留原有任务排序与筛选断言。
- 上轮审计为 M-01 通过，无退回缺陷，无需搬运红绿回归。首跑发现测试误把 project.list 的包装项当项目本体，已按真实回包 `.project` 修正；补齐 Router 8 测试参数 url/pattern，并清除未使用参数。

## 命令与结果

验收命令（实际运行三次，最终结果如下）：

`cd apps/web && npx vitest run test/add-project.test.ts test/add-project-real.test.ts test/w9-http.test.ts test/w9-layout.test.ts test/palette.test.ts test/shell.test.ts test/labels.test.ts test/styles.test.ts test/bell-view.test.ts`

- 9 个文件，59 个用例：55 通过，4 失败，0 跳过；8 文件通过。失败全在 w9-http，原因相同：`listen EPERM: operation not permitted 127.0.0.1`，请求尚未发出，不是产品拒绝结果。
- 分项：add-project 11、add-project-real 4、w9-http 4、w9-layout 1、palette 7、shell 4、labels 8、styles 12、bell-view 8。
- 与设计的本里程碑基线 58 相比 +1，新增补全防抖/乱序/取消/网络失败用例。所选既有文件原有 42 个，本轮新增 17 个，没有删除/跳过用例。与上轮 M-01 的 120 相比 -61，因为本轮更换为 M-02 验收子集，不能解释为全项目测试减少。
- 原始输出：`.autome/output/m02-tests.txt`（首跑）、`.autome/output/m02-tests-final.txt`（最终）。
- `cd apps/web && npx tsc --noEmit`：通过，输出 `.autome/output/m02-tsc-final.txt`。
- 对本轮 13 个改动 TS/TSX 文件运行 `npx eslint <files>`：通过，输出 `.autome/output/m02-eslint.txt`；文件清单为新路由、三个新组件模块、sidebar-footer、palette-model、shell、admin-labels，以及 add-project/add-project-real/w9-layout/w9-http/palette 测试（实际命令见会话工具记录）。
- `git diff --check`：通过。未运行全量门禁、cargo 检查、全项目 lint/typecheck 脚本、build；宿主快检仍需执行。

## 标待审之前的四项自审（已执行，未满足项明确保留）

1. **情形表逐行**：不存在目录、普通非仓库、仓库根、含成员目录四种提示通过 add-project #1；工作区默认值、单成员下拉通过 #2；无 probe/改路径/一致路径通过 #3；规范后只读 probe 通过 #4；不跳过/跳过两个跳转通过 #5 和真实内核 #1/#3；原文拒绝通过 #6 和真实内核 #4；部分成功原文、两个链接、禁止重复添加通过 #7；原生桥接与 SSR 隐藏通过 #8；空/非空侧栏与命令面板通过 #9；离线检查/确认禁用及原因通过 #10；补全截断回包、乱序保护通过 #11。真实工作区 app/docs/lib、建议 docs、添加后看板 loader 通过真实内核 #2。布局静态规则通过 w9-layout；真实视口检查 H-01/H-02 未取证。
2. **失败分支**：网关返回拒绝授权、读盘失败、超时、内核离线原文逐个喂入 loader，#4 通过；补全网络不可达 #11 通过；空路径、逃逸、NUL、双点输入在真实 fixture 的路径守卫中拒绝；重复添加真实内核拒绝通过；skip 失败用替身验证部分成功且不重定向。Host/Origin HTTP 四用例因监听 EPERM 未执行到断言，故不能标待审。原生系统目录授权弹窗不适用：本轮只预留 W11 桥接，不实现系统选择器。
3. **全部迁移**：未检查→检查成功→可确认、修改输入→不可确认、检查失败→原文；确认成功→设置页/项目页、确认失败→留页、部分成功→留页且隐藏重复提交，各有上述测试；补全等待→发起→成功/错误、下一输入取消计时并忽略旧响应、卸载取消均通过 #11；工作区默认选中/不选中的 SSR 与请求分支均覆盖。真实鼠标切换及点补全填入仍属 H-02，未取证。
4. **域边界**：真实 fixture 检查空串、相对路径、绝对路径逃逸、NUL、`..`、513 字符拒绝，512 字符路径守卫接受；不存在目录 probe exists=false；无成员和单成员分别覆盖默认值/下拉；候选 0/1/50/51 边界沿用 palette 测试通过。前导零、闰日、跨年不适用：本页不解析数字/日期；目录补全服务端 50 项与符号链接守卫已由 M-01 审计关闭，本轮不重跑前轮全子集。

## 待继续与限制

下一轮先在允许监听本机端口的环境执行同一验收命令，取得全部 59 例通过证据再标待审。当前权限策略不可提权；未改安全中间件，也没有跳过失败用例。未做浏览器视觉验证，静态布局检查不替代 H-01/H-02。不启动下一会话。
