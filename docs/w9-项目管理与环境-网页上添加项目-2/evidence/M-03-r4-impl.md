# M-03 · 实现 #4

结论：实现与非监听测试完成，HTTP 验收受沙箱监听权限阻塞；保持「开放」，不标待审。implementation-round：4/30。

## 实现范围

- 设置页通过 project.list 找项目，显示步骤、会话状态、两份产物与独立保存表单；完成从当前步骤最多 advance 三次，跳过调用一次 skip，展示最终 init commit 或无改动结果。错误原文显示，结束后收起操作，离线禁用写按钮。
- 服务端按最新合法 session prompt 读取 exit/pid，支持十秒启动窗口与进程存活判断；loader 恢复 watch，ticker 发布一次 onboarding.ended，根路由订阅后重载。固定路径只来自内核项目，不接受浏览器路径。
- 增加项目页设置链接、集中中文文案、限高与内部滚动样式。未推进 M-04 配置/移除等后续里程碑。
- 上一轮审计 M-02 已通过，未退回 M-03，无需搬运审计缺陷检查或执行对应红绿复现。

## 命令与证据

1. 里程碑原样验收：`cd apps/web && npx vitest run test/onboarding-watch.test.ts test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/w9-http.test.ts test/w9-layout.test.ts test/board-live.test.ts test/load-context.test.ts test/labels.test.ts test/styles.test.ts`。
   9 文件、60 用例：55 通过、5 失败、0 跳过。5 个失败均在 HTTP fixture 启动阶段出现 `listen EPERM: operation not permitted 127.0.0.1`，尚未进入 HTTP 断言。原始输出 `.autome/output/m03-acceptance.txt`。
2. 非监听子集：同上删除 `test/w9-http.test.ts`；8 文件、55 用例全过，0 跳过。原始输出 `.autome/output/m03-final-subset.txt`。最后补强 loader 恢复 watch 与消息类型后单跑 `npx vitest run test/settings-onboarding.test.ts`：7/7，输出 `.autome/output/m03-settings-final.txt`。
3. 初次直接相关单测子集（watch/settings/layout/labels/styles）34/34，输出 `.autome/output/m03-unit.txt`。重复运行不叠加用例总数。
4. `git diff --check` 通过。未运行 `sh scripts/ci-local.sh`、全量 Web 测试、宿主 fmt/clippy/lint/typecheck。H-01 浏览器视觉与实际尺寸验收未取证；监听限制下仅完成 SSR 和布局静态检查。

用例总数 **60**，相对上一轮 M-02 的 **59**，差 **+1**：公共子集原有 25 例（HTTP 4、layout 1、labels 8、styles 12）；本轮不跑添加页/命令面板/外壳/通知视图等 34 例，改跑 watch 5、settings 7、settings-real 4、board-live 7、load-context 10 共 33 例，并给 HTTP 和 layout 各增 1 例，合计 25 - 0 + 33 + 2 = 60。没有删除原测试或以 skip 避开监听失败。

## 标待审前四项自审（已执行，尚有环境缺口）

1. **情形表逐行**：设计 §4 无独立表格，逐项对照：step 3/4/5 的说明、completed/skipped 收起按钮（settings #1）；运行、保存两份产物、完成后提交（real #1）；跳过提交（real #2）；会话 none/running/ended/interrupted 与最新会话选择（watch #1–5、settings #5）；运行中禁用 run、离线全部五个写按钮禁用（settings #5）；两份路径限制与字符计数、拒绝原文（settings #3–4）；项目页设置入口（settings #6）；扩展消息订阅、取消与无关消息负对照（settings #7）；恢复 watch（settings #1）；布局限制（layout #2）均通过。HTTP 安全头/长表单已编入 w9-http #5，实际请求尚未取证。
2. **失败分支**：拒绝授权、读盘失败、超时、内核不可达通过 loader 与 save/skip/finish 的错误替身逐条检查原文（settings #1–2）；真实网关拒绝超长且内核零调用（settings #4），真实磁盘内容不变（real #3）；读 exit 遇目录模拟实际文件读取失败（watch #4）；空串保存合法且原样（settings #3）；非法路径不写；第二次 advance 失败立即停止（settings #2）；工作区 init commit 报 not a git repository，重新加载为 skipped（real #2）。HTTP Host/Origin 拒绝仍受监听权限阻塞，不宣称通过。
3. **全部迁移**：无会话→运行→退出→通知一次（real #1/watch）；运行→中断→通知一次（watch #5）；死亡 pid 下启动窗口内→运行，窗口外→中断（watch #3）；恢复服务后 loader 重新 watch（settings #1）；step 3→4→5→completed、从 step 4/5 重试到 completed（settings #2）；in_progress→skipped（real #2）；中途 advance 错误停留并返回原文（settings #2）；completed 后再次 run/finish 拒绝（real #1）；project.onboarding 账本驱动持久 router 的 step 3→4（real #4）；保存→loader 重读/完成提交内容（real #1）。结束后不再发通知（watch #5）。
4. **输入域边界**：内容 0、1、262144 字符合法，262145 字符被真实网关拒绝（settings #3–4/real #3）；两份合法路径及空路径、相对逃逸、未知路径拒绝；合法 32 位小写十六进制 session id、非法文件名忽略；单个/多个会话、无目录、存活/死亡 pid、9999/10000/10001 毫秒边界（watch #1–5）。本里程碑无日期、金额或用户输入数字域，前导零/闰日/跨年不适用。

## 下一步与限制

在允许本机监听的环境运行上述原样 60 例验收，核实正常表单和 `.data` 的超长保存确实到达 write-gate；通过后才标待审。本轮不修改协议格式，不增 reopen，不自行启动下一会话。工作区 init commit 内核缺陷沿用 B-01，未修改 crates。
