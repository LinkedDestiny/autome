# M-09 · 审计 #13 独立复验

- vitest `e2e-preflight/core-sidecar/ci-config`：3 文件 33 条全过。
- Playwright 验收命令：16 条中 14 过、2 失败（W7 H-04：data-scroll-region 实际 "true" 非 ""；W9 H-02：值 `~/completion-directory` 非绝对路径）。原始输出 `.autome/output/a13-pw.txt`。
- 结论：退回开放，reopen 1，领域 e2e-infra。负向对照待验收命令转绿后由实现轮执行。
