# M-04 · 实现 #5

结论：实现完成，验收受环境阻塞；M-04 保持开放，reopen 仍为 0。没有标待审或已完成。

## 实现与修复

- 设置 loader 使用 `project.list` 定位项目，独立读取 `config.get` / `config.validate`；配置失败只影响配置卡。
- 配置按实际返回展开；角色按真实 `roles[].config` 展示，null 显示未配置，未知字段保留原键；不补缺省值。新增真实内核 fixture 和可重复抓取逻辑。
- `admin.configPaths` 使用 AUTOME_HOME 与项目目录，家目录内路径缩写为 `~/`，支持 macOS HOME 符号链接；路径前缀相似但不属于 HOME 的目录不缩写。
- 移除卡说明仅取消注册、不会删除文件并显示目录；客户端禁用不匹配输入，服务端再次比较 project.list 的 display_name，成功重定向 `/`，拒绝原文返回。已有 M-01 写白名单与根路由事件规则复用。
- Onboarding 既有测试适应新增移除按钮、配置读调用及 action 重定向联合类型，未删用例或放松 Onboarding 行为断言。
- 上一轮审计 M-03 通过，无需搬入缺陷检查。开发中发现旧按钮计数仍为 5 的一处断言，本轮新增移除按钮后更新为 6，再跑通过。

## 命令、结果和基线

1. 指定验收：`cd apps/web && npx vitest run test/settings-config.test.ts test/settings-real.test.ts test/w9-http.test.ts test/labels.test.ts test/styles.test.ts`。
   最终输出 `.autome/output/m04-accept-final.txt`：5 文件、37 例，31 通过、6 失败、0 跳过。6 个失败全部在 HTTP 文件启动监听时发生 `listen EPERM: operation not permitted 127.0.0.1`，尚未进入请求断言；不能算 HTTP 安全验收通过。此前首次同命令结果一致，见 `m04-accept.txt`。
2. 直接相关子集：`cd apps/web && npx vitest run test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/w9-layout.test.ts`。
   初次 13 例中 1 个按钮计数断言失败，见 `.autome/output/m04-related.txt`；修正后纳入下一条命令通过。
3. 直接相关验证汇总：`cd apps/web && npx vitest run test/settings-config.test.ts test/settings-real.test.ts test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/w9-layout.test.ts test/labels.test.ts test/styles.test.ts`。
   `.autome/output/m04-final-direct.txt`：7 文件 44 例全部通过，0 跳过。随后将角色内部 null 文案统一为「未配置」，最终指定验收再次验证配置相关用例通过。
4. `git diff --check` 通过。未执行全量门禁、cargo fmt/clippy、前端 lint/typecheck，交给宿主快检。

用例总数：本轮指定验收 37，指定的 M-04 基线为 37，差 0；相同范围在本轮改动前为 25（HTTP 5、labels 8、styles 12），增加 12（settings-config 7、settings-real 4、HTTP 1）。计入直接相关子集，本轮去重总数 50，44 通过、6 环境失败、0 跳过。上一轮 M-03 基线 60，本轮差 -10：不运行 onboarding-watch 5、board-live 7、load-context 10，共少 22；新增本轮 12，合计 -10。不是删测试或隐藏跳过。

## 标待审之前的自审清单（本轮执行，因 HTTP 未通过不报待审）

1. **情形表逐行**
   - §5 配置递归值、角色、null、未知字段、缺省字段：settings-config #1、settings-real #1 通过；真实 resolved 与 fixture 全等，渲染每行与值一致。
   - validate 警告逐条原文：settings-config #2 与真实 session_minutes=1 检查通过。
   - 两份路径与 HOME 内外边界：settings-config #3 通过。
   - config.get 失败仍保留其他卡：settings-config #4、settings-real #4 通过，坏 TOML 下实际移除成功。
   - 移除名字二次确认、目录说明、只读配置、写门禁：settings-config #5/#6 通过。
   - 未结束草稿任务拒绝原文：settings-real #2 通过。
   - 取消任务后移除、重定向、project.removed 让持久 memory router 根 loader 项目数减 1：settings-real #3 通过。
   - 仓库相对路径、文件大小、sha1（含 .git）前后相同：settings-real #3 通过；新源码删除调用静态守卫 #7 通过。
2. **失败分支**
   - 配置读取拒绝授权、读盘失败、超时、内核不可达：settings-config #4 分别执行并检查原文和卡片隔离，通过。
   - 名字错误及空输入：#5 检查不调用写入口，通过；真实未结束任务拒绝、坏 TOML：settings-real #2/#4 通过。
   - HTTP 坏 Host / Origin：新增真实请求检查覆盖 settings 和 settings.data，两种头均保留项目的断言已入项目测试；本环境监听受阻，未取证。
3. **全部迁移**
   - 输入空/不等 → 按钮禁用、服务端拒绝且零写入；名字相等 → 移除写调用、成功重定向：settings-config #5 通过。
   - 已注册且任务未结束 → 拒绝并保持注册；任务取消 → 可移除 → 账本事件刷新为无项目：settings-real #2/#3 通过。
   - 配置有效 → 改为无效数值 → 重新加载出现警告；坏 TOML → 配置错误而其他卡可用：settings-real #1/#4 通过。
   - 本轮无新增后台状态机；Onboarding 既有状态迁移用 11 个直接相关用例复验通过。
4. **输入域边界**
   - 名字空串、前/后空白、不完整、大小写差异拒绝；中文完全相同接受；不 trim 或转换大小写：#5 通过。
   - 配置 null、空串、0、false、未知键（含 toString）、缺省字段：#1 通过。
   - 路径 HOME 内、HOME 根、相似前缀外部目录、AUTOME_HOME 外置：#3 通过。
   - 日期/闰日/跨年/数值最大值：不适用，本页只读内核配置，不解析这些输入；project_id 的边界沿用 M-01 写门禁，本轮不新增输入域。

## 负向对照

在原文件上临时变异，逐项运行对应项目测试并在 finally 还原：
- 移除名字核对置为 false：settings-config #5 红；`m04-negative-name.txt`。
- 强加缺省 gate_run=each_round：settings-config #1 红；`m04-negative-default.txt`。
- 移除成功不 redirect：settings-real #3/#4 红；`m04-negative-redirect.txt`。
- 设置 loader 先依赖 project.get 成功：settings-real #4 红；`m04-negative-project-get.txt`。
- 还原后相关 44 例绿，最终验收中的配置/真实内核 11 例绿。没有遗留变异。

## 环境与下一步

fixture 首次尝试 `node --import tsx` 发现仓库无 tsx，改用 Node 原生 TypeScript 执行并成功采集；没有安装依赖。
浏览器截图与 H-01 人工尺寸验收未取证：本沙箱连临时 HTTP 服务都不能监听。已做 SSR 和既有 styles/layout 静态检查，不把它们称为真实浏览器验收。
下一轮在允许监听的宿主环境补跑指定 37 例验收；通过后执行/复核自审清单再标待审。当前并非协议失败。
