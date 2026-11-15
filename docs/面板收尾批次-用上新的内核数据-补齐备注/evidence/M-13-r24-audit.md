# M-13 · 审计 #16 · 集成复验（提交 6201e19，范围 03c9be1..8250a38）

结论：**通过**，里程碑表不动（全部已完成）。

## 逐项复验
1. 改动范围：6201e19 只把 `apps/web/e2e/harness/setup.ts` 普通功能实例 tick 由 1000 改为 100 ms（+3/-1），liveRoot 仍 3000 ms；`serve.ts` 的 `live = tickIntervalMs===3000` 判定与 controls/live 规格里 `tickIntervalMs===3000` 的五秒断言不受影响。globalTimeout 360 s、单例 90 s、retries 0 未动，没有放宽预算。无 rebase 丢失提交（简报 0 个）。
2. 断言与前提：e2e 规格与测试文件在该提交中零改动，没有削弱断言；规格内无依赖 1000 ms tick 的断言（grep 无命中）。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' 6201e19 -- . ':(exclude)docs/…/'` 结果为空。
4. M-13 验收命令：`npm run build --prefix apps/web && vitest run task-detail-ssr task-detail-payload test-groups` → 3 文件 11 用例全过。
- 负向对照：把 task-detail-ssr 的 150 KB 阈值临时压到 1 KB，三条均失败；已 `git checkout` 还原，工作区干净。

## 局限
本沙箱 `ps` EPERM，Playwright 在 setup 阶段即失败（`.autome/output/r16-pw.txt`），无法本机验证 C-2/C-2b 的耗时是否降到 360 s 内；这要由宿主合并前门禁实测。该修复是否足够不能由本轮证明，但改动本身未发现产品行为缺陷，也没有削弱验收。
新造检查：阈值压缩负向对照（丢弃）。
## 提示
6201e19 把设计文档 M-05 的「最新证据」指针改成了 r24-impl（待宿主复验），属流程记录，不影响产品行为。
