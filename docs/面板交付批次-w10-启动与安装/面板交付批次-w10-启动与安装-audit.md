# 审计 · 第 13 轮 · M-09

结论：**实现缺陷，退回开放**（reopen 0→1，领域 e2e-infra）。

## 失败命令
`cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts` → 14 通过、2 失败（契约要求 16 条全过）。vitest 三文件 33 条全过。

1. `manual-items.spec.ts:33` W7 H-04：`toHaveAttribute('data-scroll-region', '')` 实际 `"true"`。React 把布尔 JSX 属性 `data-scroll-region` 渲染成 `"true"`，断言写错（验收 §设计 H-04 只要求它是 scroll region）。
2. `manual-items.spec.ts:49` W9 H-02：点选后 `path-input` 值为 `~/completion-directory`，用例期望展开后的绝对路径。设计 H-02 只写「值是那个路径」，产品给出 `~` 形是补全的既有行为；期望值与产品对不上。

## 说明
- 失败原因均在验收用例（项目测试体系不通过），按协议计实现缺陷退回；产品行为本身未被证明违背设计。
- 负向对照未做：验收命令本身未绿，先修用例再做对照。下一轮实现轮须跑完契约四条负向对照。
- 判别检查：即验收命令本身，输出 `.autome/output/a13-pw.txt`；实现轮修后原样重跑应绿。
- 实现 #14 证据称浏览器 16 条未执行，本轮首次实跑，与此一致。
- 提交由 Autome 兜底（如被拒）。
