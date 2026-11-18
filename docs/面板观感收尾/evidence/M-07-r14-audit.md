# M-07 r14 审计复验（审计 #8）· 通过

- 验收命令 `npx vitest run`（6 个文件）：86 例通过、0 失败；task-timeline 37（≥34）。
- 负向对照（均已还原，`git status` 干净）：
  - 删 `.timeline-row` 的 `min-block-size` → 第 4 条失败。
  - 删折叠态 `white-space: nowrap` → 同一条失败。
  - 只对会话条目输出 `details.timeline-text` → 5 例失败。
- 浏览器：本沙箱可跑。`npx playwright test e2e/layout.spec.ts -g "需求详情展开日志"` 通过（1 passed），其中包含所有折叠行 ≤44px、前 10 条完整落在时间线可视区、点开普通条目文字完整的断言，断言未放宽。日志 `.autome/output/a8-e2e.log`。上一轮 r13 的 51px 缺陷已不再出现。
- 操作者备注 #1：实现证据已有「操作者备注」小节，写明不放宽断言，并给出预算（第 10 条预计底边约 915px，余量约 5px）。预算与实测相符，e2e 实测通过，接受。
- 新造检查：无，只复用验收契约与既有 e2e。
