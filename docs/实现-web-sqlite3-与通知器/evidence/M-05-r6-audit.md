# M-05 审计 #5 独立复验（r6-audit）

结论：通过。

- 验收命令：`bell-system.test.ts` + `bell-view.test.ts` 共 16 个全过。全量：web 33 文件 300 用例 0 跳过；desktop 104 个、103 过 1 跳过（同基线）；lint、typecheck、build 无错误。
- 负向对照（逐个注入、跑两文件、`git checkout` 还原，工作区干净）：
  - `hidden` 判断写反 → 5 个用例失败；`tag` 固定为 `'x'` → 用例 (5)(8) 失败；
  - 挂载时调 `request()` → 构造参数/无自动授权用例失败；列表找不到就放弃 → 用例 (5)(6) 失败；
  - 去掉 `levelOf` 级别判断 → 「知会不弹」「四种权限」2 个用例失败。
- 自审：`shouldShow` 与设计一致（need_you ∧ granted ∧ hidden）；`document.hidden` 与 R-07 的 `visibilityState !== 'visible'` 在目标浏览器等价。
- 真实浏览器的授权框、横幅、多标签 tag 合并无法取证，已在人工验收清单 H-02、H-03、H-04。
