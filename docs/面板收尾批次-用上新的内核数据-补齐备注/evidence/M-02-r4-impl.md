# M-02 实现轮 #4：快检修复

结论：修复项目配置错误区的内联样式，M-02 保持待审，不推进新里程碑。implementation-round 为 4/65。

## 根因与修复

宿主在 aecfb524eb 的快检发现 test/styles.test.ts 的「TSX 没有内联样式」失败，违反任务 §6.9。全 app 搜索仅 projects.$id.tsx 一处命中：上一轮为错误原文添加的 maxHeight/overflow。将两条声明等值迁至 app.css 的既有类 .board-config-error（max-height: 120px; overflow: auto），保留原文、条件渲染、警告语义和 data-scroll-region。

失败检查已在项目测试体系中，原样保留，不复制另一个重复用例、不放宽断言。上一审计只关闭 M-01，没有 M-02 新造检查需要搬入。前轮未覆盖 styles 守卫，本轮补跑该文件与完整 unit 组。

## 命令、红绿与总数

原始输出在 .autome/output/，以下命令在 apps/web 下运行（构建和快检在仓库根）。

- 红：npx vitest run test/styles.test.ts。12 个用例，11 通过、1 失败，失败名称与宿主完全一致。日志 r4-styles-red.txt。
- 绿：npx vitest run test/styles.test.ts test/board.test.ts test/testids.test.ts。33/33；其中 styles 12/12，较红基线总数差 0，只将原失败转绿。日志 r4-targeted-green.txt。
- M-02 设计契约完整命令（cargo build -p automed -p autome-cli、九个指定测试文件及旧模块/推送不存在检查）退出 0，74/74，与上一轮 r3 最终契约 74 的差为 0，无新增、删除、跳过。日志 r4-accept.txt。
- npx vitest run --project unit：73 文件、663/663。日志 r4-unit.txt。上一轮记录的是相关 unit/kernel 213 用例，非完整 unit 基线；表面差 +450 来自选择范围不同，不能解释为新增测试。本轮未修改测试文件，各同范围用例总数变化均为 0。重叠测试不相加作为唯一用例总数。
- 用户给定快检命令原样执行：

```sh
if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi
```

退出 0，5 步通过、0 沿用、0 失败，用时 18 秒，日志 r4-quick.txt。脚本明确提示「未提供本轮起点，未跑改动相关的测试」，因此不能声称本次快检复现了宿主的相关测试选择；上面的完整 unit 和 M-02 验收提供独立测试证据。最终快检以 Autome 提交后重跑为准。未运行 ci-local 全量门禁。

## 标待审之前的自审清单

本轮保持既有待审状态，四项仍逐项核对：

1. **情形表逐行**：A-4 的四种 probe 拒绝及未知类型，A-5 无会话/运行/五种结束及磁盘兜底，A-6 全字段/默认/角色来源，A-7 总览/侧栏/看板/设置的坏配置隔离，均由契约 74 用例再次通过。直接修改的 A-7 错误区由 board 测试核对原文转义、卡片保留；styles 验证无内联样式；testids 验证滚动区白名单。CSS 两条声明与原数值等价。
2. **失败分支**：契约原有拒绝、读盘失败、超时状态、空输入、内核失败检查全过；本轮未新增调用或异常分支。外部第三方服务不可达不适用，未引入第三方服务。
3. **全部迁移**：契约再次覆盖 Onboarding 的无会话/运行/终态与 running 磁盘兜底、配置正常→错误→修复；样式迁移本身无新增状态机。项目错误条件与正常条件的渲染逻辑保持原样。
4. **输入域边界**：契约既有空串、过长正文、非法路径、配置空值/未知键、会话 id 与磁盘时间边界再次通过。本轮 CSS 不接受新输入域，日期/数字解析边界无新增适用项。

## 限制与清理

本轮为等值样式迁移，未启动浏览器做视觉验收，不能声称截图或真实页面几何已验收。真实内核用例沿用临时 HOME/AUTOME_HOME 和指定本检出二进制的夹具及 teardown；没有手工启动服务、浏览器或访问真实账本。git diff --check 通过。提交时设置 NO_CI=1，避免触发禁止本轮运行的全量门禁。

## 完整契约命令

- 验收命令：`cargo build -p automed -p autome-cli && (cd apps/web && npx vitest run test/add-project.test.ts test/settings-config.test.ts test/settings-onboarding.test.ts test/settings-onboarding-real.test.ts test/onboarding-disk.test.ts test/shell-data.test.ts test/board.test.ts test/overview.test.ts test/test-groups.test.ts) && test ! -e apps/web/server/onboarding-watch.ts && test -e apps/web/server/onboarding-disk.ts && ! grep -rqE "onboarding-watch|onboarding\.ended|createOnboardingWatch" apps/web/server apps/web/app apps/web/test`
