# M-04 实现 #5

## 范围与修复

按 B-Q1/B-Q2 实现六张角色卡的紧凑布局：12px 14px 内边距、5px 间隔、顶行角色名与常显编辑提示、独立开关、模型与 Effort 同行、技能数与项目来源胶囊、停用说明两行省略。模型及停用说明保留完整 title。伸展按钮覆盖卡片，开关以 z-index: 1 保持独立；hover/focus-within 使用现有令牌反馈。范围导航沿用已实现的 seg 和 aria-current。

不修改 M-05 默认值或抽屉。上一轮 M-03 审计通过，没有退回检查需要搬运。

## 命令与结果

- 验收命令：`(cd apps/web && npx vitest run test/loop-page.test.ts test/ui-base.test.ts test/styles.test.ts test/testids.test.ts test/labels.test.ts)`。
- 同命令起点基线 67，修改后及负向对照还原后均为 71，增加 4：范围选中标记、模型同行与长输入、编辑按钮和独立开关、密度和交互反馈。文件分布为 loop-page 18、ui-base 12、styles 15、testids 15、labels 11，失败 0、跳过 0。上一轮 M-03 的 137 属于不同验收集合，不能直接当本轮基线。日志 `.autome/output/m04-baseline.txt`、`m04-accept.txt`、`m04-final.txt`。
- 三条负向对照逐个临时注入、运行 `cd apps/web && npx vitest run test/loop-page.test.ts`：Effort 移出 `.loop-role-model`、开关放进按钮、删除 hover 选择器，各 exit 1；均已恢复。日志 `.autome/output/m04-negative-{effort,nested-switch,hover}.txt`。恢复后验收 71/71。
- 旧引用检索：`rg -n 'loop-role|绑定技能' apps/web/test apps/web/e2e apps/web/scripts`，改前 5 行、改后 20 行，均为保留的选择器及新增断言；旧「绑定技能」卡片文案没有测试依赖。原始输出在 `.autome/output/m04-search-{before,after}.txt`。
- `git diff --check` 通过。
- e2e 原有「Loop 配置」用例增加全局/项目角色卡 ≤160px、模型与 Effort 中线一致断言；没有增删用例。依任务边界未运行浏览器或全量门禁，真实一屏观感仍由 H-02 验收。

## 标待审前自审清单

1. **情形表逐行**：B-Q1 顶行角色/编辑/开关（18 用例中的独立结构断言通过）；工具行保留 Claude Code/Codex；模型/Effort 同容器及 nowrap、ellipsis、flex none 断言通过；技能和项目来源胶囊渲染通过；停用说明两行 CSS 和完整 title 断言通过；内边距、行间距和行外边距源码检查通过。B-Q2 常显编辑/可访问名六角色逐一检查通过，伸展按钮 inset 0 与开关 z-index 1 检查通过，hover/focus-within 反馈及负向对照通过。
2. **失败分支**：已有验收中「离线失败明确且无配置卡」「没有网关时保存失败并无已保存项」「写入未授权时角色开关禁用」「冲突的两张卡标红且说明」均通过。本里程碑未引入读盘、超时、外部服务请求分支，这些不适用。
3. **全部迁移**：本轮是展示结构修改，不新增状态机；全局/项目两个范围静态渲染与当前项标记通过，enabled=true/false 两种展示通过；原启用回调、编辑打开、关闭与保存逻辑保留。真实点击范围往返与鼠标/键盘反馈由既有 e2e 与 H-02 确认，本轮不声称已完成浏览器交互验收。
4. **域边界**：模型空串、单字符、480 字符长模型均运行通过，空串显示 CLI 默认，长模型 title 无截断；默认 Effort、未知 Effort 保留由既有用例通过；六角色及零技能渲染通过。日期、前导零、数值最大值不适用：本轮无该类输入域变更。

## 宿主同命令快检

`sh scripts/ci-quick.sh --since 5cadaa7522f1fa81e89faf03d6fd2cbbd0cfa226` 已执行，80 秒，28 步中 26 步通过、2 步失败。Format、Clippy、Web lint、typecheck、build 通过。失败是 shell-live 的监听测试 `listen EPERM: operation not permitted 127.0.0.1`，以及 Web e2e 初始化和清理 `spawnSync ps EPERM`；浏览器整套未执行，不记作通过。此为指定快检自行选择的关联检查，本轮没有另跑 ci-local。日志 `.autome/output/m04-quick.txt`。宿主需在允许监听与进程查询的环境复验。
