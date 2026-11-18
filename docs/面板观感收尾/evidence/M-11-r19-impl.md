# M-11 · 实现 #19

## 范围与修复

- 抽取 `scripts/screenshot-shots.ts`：README 默认五图保持不变，all 十图与 attachments 一一对应；项目与任务 ID 编码。
- 两条抽屉图分别从 loop-role 的按钮、skill-card 打开并等待 dialog；loop-bottom 滚动到默认值，loop-projinfo 打开项目页签。
- 每张截图在点击、滚动、字体加载后对 body 可见文本执行 dirtyText，发现脏文本即终止。沿用原有隔离夹具与 finally 清理。
- 参数支持 --set readme|all 与 --out，拒绝未知、重复、缺值、空值参数。all 要求显式输出目录，避免默认污染 README 图片。
- 上轮 M-10 审计通过，没有审计退回检查需要搬入回归体系。本轮仅推进 M-11。

## 命令与结果

- 验收命令：`(cd apps/web && npx vitest run test/screenshot-shots.test.ts test/dirty-text.test.ts test/readme-images.test.ts test/test-groups.test.ts)`。
- 修改前同命令：3 文件、10 用例通过；新文件尚不存在。修改后及负向对照还原后：4 文件、17 用例通过，0 失败、0 跳过。较同口径基线 +7，全部来自 screenshot-shots（清单、抽屉、参数、输入边界、ID 编码、执行顺序），既有 10 条未减少。上一轮 M-10 的 88 条是另一测试子集，不能与本轮作覆盖增减比较。
- 原始输出：`.autome/output/m11-baseline.txt`、`m11-tests.txt`、`m11-final-tests.txt`。
- 负向对照：临时去掉 env，清单用例失败（exit 1）；临时将未知集合静默回退 readme，参数用例失败（exit 1）。均已还原并重新通过 17 条。输出为 `.autome/output/m11-negative-missing-env.txt`、`m11-negative-unknown-set.txt`。
- 旧文案检索：`grep -rnE '已生成五张|args\[1\]' apps/web/scripts/screenshots.ts`，修改前两处，修改后 0；其余测试无旧类名/文案需迁移。
- `git diff --check` 通过。

## 标待审前自审清单

1. **情形表逐行**：README 五图名称与默认路径通过；all 十图逐文件与 attachments 比较通过；角色抽屉 / 技能抽屉路径及稳定 testid 通过；参数解析和未知集合拒绝通过；脚本逐张先查 body 脏文本再截图通过。已有 dirty-text 两条验证四种泄漏值及普通词不误判。实际图像与布局对照未实跑，按总-Q4 留在 H-01/H-09。
2. **失败分支**：未知集合、缺值、空值、未知参数、重复参数均拒绝；漏图与静默回退负向对照均红。脏文本检测由既有 dirty-text 用例与脚本调用顺序断言覆盖。授权、读盘失败、外部服务不可达与连接超时：本里程碑设计未新增这些分支，不适用，原脚本连接与清理机制不变。
3. **全部迁移**：无新增产品状态机，不适用。截图执行通道覆盖默认 / all 两集合、两抽屉点击、默认值滚动及项目信息查询参数；真实浏览器打开抽屉未实跑，属于 H-01。
4. **域边界**：空参数保留默认值；空白、空串、单个缺值参数拒绝；单目录及中文目录接受；参数交换顺序通过；带斜杠、空格、问号、井号的动态 ID 编码通过。日期、数值最大值不在本模块输入域，不适用。

## 验证边界

未运行 ci-local 全量门禁，未生成或改动 README 图片。H-01 的实际拍摄与十图人工对照仍待执行，本轮证据仅证明脚本契约和测试通过。

## 宿主同口径快检

`sh scripts/ci-quick.sh --since 13a9bd2d387fe09850c78be660048c3468c5c1b9` 返回 0，执行 88 步、沿用 0 步、失败 0 步，用时 139 秒。输出 `.autome/output/m11-quick.txt`。脚本达到 120 秒快检预算后未跑的步骤留给合并前门禁，不能宣称全量测试通过；本轮验收用例总数仍按上述指定子集 17 条报告。
