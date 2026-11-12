# M-02 实现轮 #2

本轮只推进 M-02。上一轮审计通过 M-01，没有退回缺陷，因此没有需要逐字移植的审计回归检查。

## 实现与修复说明

- 将 desktop Layer 1 的全部 token 原值迁入 tokens.css，复制 Nunito 和 OFL；两份 CSS 标注 animal-island-ui、CC BY-NC 4.0 来源。不迁移暗色主题。
- 根 links 接入样式表；Shell 提供项目栏、顶栏、横幅行和受高度约束的内容区。新建入口按本里程碑约定暂时禁用；项目计数与搜索面板留给 M-04/M-05。
- 铃铛仅把 16 处内联样式替换为类名，定位改为顶栏内 relative，弹层仍 absolute、60vh 内滚动。订阅、客户端、已读和系统通知逻辑未改。
- liveBanner 按重连、内核离线、未知 core 时 loader 错误的顺序显示一条横幅；writeGate 只允许 open + connected。WriteGateProvider 可供后续页面复用。
- useKernelRecovery 在 connected 且 loader 离线条件的上升沿调用 revalidateAll，持续失败不循环；App 接入真实 core 和 kernel.ok。404 使用中文未开放页与返回总览链接。
- 既有 bell-view 的徽标断言改查类名，定位与滚动断言改查 CSS；live-banner 从 top:0 改查类名。测试数量不变；root 测试的 hydration 接线已由 M-01 完成。
- 首次定向测试 27/30：两个新测试的 desktop 相对路径多上一级；Vitest 默认将 CSS URL 返回为空，links 用例因此注入明确 URL 替身。修正后 30/30。真实 CSS/字体接线另由开发/生产 SSR 与 build 检查，不以 URL 替身证明打包。
- 设计预期数量统一为静态 316、运行展开 330。修正 unknown-core 负向对照的预期：它使 live-status 第 3、4 条失败，SSR connecting 用例保持禁用，不能要求后者变红。

## 验收命令与结果

原始日志和临时产物均在 `.autome/output/w5-m02-*`，这里仅记结论。

| 命令 | 退出码 | 结果 / 日志后缀 |
|---|---|---|
| `npm run lint --prefix apps/web` | 0 | lint-final.txt |
| `npm run typecheck --prefix apps/web` | 0 | typecheck.txt |
| `npm test --prefix apps/web -- test/styles.test.ts test/live-status.test.ts test/shell.test.ts test/bell-view.test.ts test/live-banner.test.ts` | 0 | 5 文件、30/30，无跳过；target.txt |
| `npm run build --prefix apps/web` | 0 | build.txt |
| `ls apps/web/build/client/assets \| grep -c 'Nunito.*\.ttf'` | 0 | 输出 1；字体 276932 字节 |
| `npm test --prefix apps/desktop` | 0 | 总数 104、103 passed、1 skipped；desktop.txt |
| `git diff --check` | 0 | 无空白错误 |

| `npm test --prefix apps/web` | 1 | 330：300 passed、11 failed、19 skipped；full.txt，156.34 秒 |
| `npm test --prefix apps/web -- --exclude test/server.test.ts --exclude test/gateway-http.test.ts --exclude test/eventbus-live.test.ts --exclude test/notifier-live.test.ts --reporter=json --outputFile=../../.autome/output/w5-m02-runnable.json` | 0 | 33 文件、300/300，0 skipped；runnable.json |

全量阻塞范围：gateway-http 的 11 条因 listen EPERM 失败；server 的 9 条、eventbus-live 的 7 条、notifier-live 的 3 条因启动阶段不能监听而跳过（合计 19）。原始错误及精确名称见 full.txt，另有 watcher EMFILE。排除版只定位环境限制，不能替代全量门禁。结果与上一轮实现环境的阻塞范围相同；上一轮审计已在其环境通过这些测试，本轮仍须独立复验。

**结论：M-02 定向验收通过，提交待审；全量门禁受环境限制，未声称全绿。**

**用例总数与基线差：**定向由既有 bell-view 8 + live-banner 2 共 10 增至 30，新增 styles 10、live-status 6、shell 4。Web 上轮审计运行基线 310，本轮实跑 330（+20）；静态枚举 296 → 316（+20），原有参数化展开的额外 14 不变。Desktop 104 → 104（+0），没有增删或改动 desktop 用例。

## 标待审之前的自审清单

1. **情形表逐行**：styles 10 条逐项执行——token 名值集合相等；字体 hash/OFL 相等与本地 URL；双文件署名；颜色只来自 token；黄色焦点环；圆角 token；主按钮独占 3D 阴影；无 emoji；无内联样式；外壳及四个内部滚动区 CSS。live-status 6 条覆盖横幅优先级、角色、写判定、未知 core、context 消费与恢复重验。shell 4 条验证项目栏/顶栏铃铛/子路由、links、未知路由和 SSR 禁用。修改的 bell-view/live-banner 共 10 条全部通过。真实浏览器视觉与滚动属于 H-01/H-02/H-05/H-06，本轮因预览监听 EPERM 未取证，静态 CSS 检查不冒充浏览器验收。
2. **失败分支**：外部服务不可达的 offline reason、loader 的拒绝授权/读盘失败/超时/空字符串错误均逐项输入并验证横幅原文；重连与 offline 并存时只出现重连横幅；core connected 时不会继续显示旧 loader 错误。未知 core 拒绝写；恢复重验仍失败只调用一次，不反复请求。未知任务路径实际经 createRoutesStub 渲染 404 未开放页。真实授权、磁盘失败注入不适用：本轮只呈现已有 loader/gateway 的错误，不新增读盘或授权行为；相应错误文字路径已实跑。
3. **全部迁移**：写判定遍历 connection（connecting/reconnecting/open）× core（未知/offline/connected）9 个组合；恢复谓词遍历 core 三态 × kernelOk 两态 6 个组合。真实 useKernelRecovery 与 createMemoryRouter 覆盖 offline→connected、初始 connected（首个 hello）、重验成功 false→true、重验失败 false→false、持续 connected 不重复、offline→connected 再恢复；成功后不多读，持续失败在下一次恢复才再读。Node 无 DOM，测试仅替代 effect/ref 调度和 useRevalidator 入口，真实 hook、revalidateAll、shouldRevalidate 和 loader 均执行；不声称真实浏览器生命周期验收。横幅测试覆盖 reconnecting 优先、offline、未知 core 的 loader 回退、connected 清除旧错误。
4. **域边界**：项目空列表（原 root 测试）与单项目（shell 测试）；铃铛徽标 0/1/99/100；core 缺失；空错误字符串；无 gateway 的既有 loader 检查均保留。字体内容按 sha256 比较。日期前导零、闰日、跨年、数值最大值不适用：本轮不新增日期或数值解析逻辑。

## 负向对照

`.autome/output/w5-m02-negative.mjs` 每项用 try/finally 原样还原，7 次退出码均为 1：

- 恢复 bell 内联白色背景：styles 第 4、9 条失败。
- shell overflow 改 auto：styles 第 10 条失败。
- 移除 project-list min-height：styles 第 10 条失败。
- open + 未知 core 允许写：live-status 第 3、4 条失败。
- needsKernelReload 恒 false：live-status 第 6 条失败。
- App 移除恢复 hook 调用：live-status 第 6 条失败。
- 删除 tile-peach：styles 第 1 条失败。

各项原始日志为 negative-<名称>.txt；negative-summary.txt 汇总退出码。临时突变已全部还原，随后运行全量/环境隔离测试与最终 lint。

## 环境与交接

Desktop 原有 `electron dom harness: real-DOM assertions against the actual renderer` 因缺少 Electron 二进制跳过；没有为了零跳过而修改测试或越界安装 desktop 依赖。生产预览监听 `127.0.0.1:43125` 遭 `listen EPERM`，日志 preview.txt，故未进行浏览器截图检查。此限制交审计环境复验，不声称全量零跳过或人工验收已通过。
