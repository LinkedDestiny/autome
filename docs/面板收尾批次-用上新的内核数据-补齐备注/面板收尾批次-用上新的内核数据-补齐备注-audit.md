# 审计结论（审计 #8 · M-07）

结论：**实现缺陷，退回开放**（reopen 1，领域「README 截图产物未替换」，收敛模式 normal）。

- 失败命令：验收命令末段 `cd apps/web && npx vitest run test/readme-images.test.ts test/test-groups.test.ts`，尺寸用例失败：收到 [1200, 451]，期望 [1512, 944]。
- 其余各段通过：cli `web_` 10、`--bin autome` 67、构建、截图脚本两次运行（各 3 张 1512×944，残留 0）。
- 根因：脚本已实现且可用，但 `docs/images/` 的三张产物未替换。实现轮因 ps EPERM 没跑脚本；审计环境可跑，下一轮直接 `--out docs/images` 生成即可。
- 判别检查：readme-images 现有尺寸用例（无需新造）。证据：evidence/M-07-r12-audit.md。
