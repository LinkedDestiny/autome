# M-11 · 实现 #17 · 截图交接与布局环境阻塞

## 范围与结论

只处理最小开放里程碑 M-11。上一审计明确允许 ps EPERM 持续时将五图重生成移入人工清单，本轮新增 H-09，要求合并前重新生成五图并目视确认。未修改产品代码、截图脚本、清理检查或其它里程碑。

M-11 保持开放，reopen 保留 3，convergence-mode 保留 milestone-review。完整验收未通过，不能标待审；没有把环境问题记为协议失败。下一步在具备权限的环境复验布局，不应重复缺值实现修复。

## 审计判别检查与回归

上一轮判别检查逐字为「人工查看 dashboard.png」，未新增脚本。本轮用图片查看工具打开 docs/images/dashboard.png，原样复现红：第二张卡片显示「undefined / undefined」，进行中卡片不在首屏。没有可逐字搬入测试体系的脚本；上一实现轮已将对应缺值回归纳入 overview.test.ts，截图脚本已有禁止 undefined、两张卡片整张入视口的断言，本轮保留并运行。实际 PNG 的绿证据未取得，移交 H-09，不以单元测试替代图片内容验证。

## 命令与结果

- `npm run screenshots --prefix apps/web`：失败，服务启动前 spawnSync ps EPERM。日志 `.autome/output/m17-screenshots.txt`；未生成新图。
- `(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts test/overview.test.ts)`：3 文件、25 例全过，日志 `.autome/output/m17-unit.txt`。同选集上一实现轮基线 25（4 + 2 + 19），差 0，无新增用例。
- 完整契约 `(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts)`：退出 1；6 条单元测试通过，14 条布局用例因 setup/cleanup 的 ps EPERM 未执行。浏览器日志 `.autome/output/m17-layout-contract.txt`；独立布局尝试同样失败，日志 `.autome/output/m17-layout.txt`。
- 契约用例总数 20 = 6 + 14，与上一实现轮基线差 0；实际执行 6，与上一实现轮差 0。未执行 14，不是通过或跳过。额外 overview 19 条与契约无重叠，唯一计划用例合计 39、实际通过 25；同选集基线差 0。
- `sh scripts/ci-quick.sh --since ffed0a9a909281c2ff1aeb15f733134e35d63f96`：5 步通过、0 失败，19 秒，日志 `.autome/output/m17-quick.txt`。仅证明快检选中的检查通过，不包含受阻布局验收。
- 未运行全量门禁。

## 标待审之前的自审清单（未达到待审条件）

1. **情形表逐行**：README 精确引用五图、目录集合、PNG 1512×944、Loop/技能非空、截图脚本文档，6 条测试通过；总览缺值回退由 19 条 overview 测试通过。实际 dashboard 图片检查失败；其它四图重生成未执行，统一移交 H-09。14 条全站布局检查前置受阻，无通过证据。
2. **失败分支**：截图进程枚举失败实际触发，启动服务前停止；overview 原有内核离线、详情读取失败和缺轮数回退检查通过。授权、外部服务超时不适用：本轮仅改交接文档，不修改相关行为。未执行缺图或去除滚动区的负向注入，不记通过。
3. **全部迁移**：不适用，本轮未修改状态机、多通道或产品行为。里程碑仍为开放，人工交接不是验收通过。
4. **域边界**：既有 overview 用例覆盖 undefined/null/0/1、零与单里程碑、无计费与零计费，全部通过；图片精确集合与尺寸检查通过。新增输入域不适用，本轮无新增产品输入。

## 提交

结束前尝试提交；若沙箱拒绝写 Git 元数据，提交由 Autome 兜底。

最终文档改动后再次执行同一指定快检：5 步通过、0 失败，16 秒；日志 `.autome/output/m17-quick-final.txt`。`git diff --check` 通过。
