# M-05 · 审计复验（实现轮 r7）· 通过

验收命令 `cd apps/web && npx vitest run test/clock.test.ts test/task-timeline.test.ts test/settings-config.test.ts test/labels.test.ts`：4 文件 41 条全过，skipped 0，与契约 8+12+11+10 一致。全部不需要监听。

负向对照（逐个注入、复跑、`git checkout` 还原，最终工作区无产品改动）：

| 注入 | 结果 |
|---|---|
| `parts()` 去掉 `timeZone: clock.timeZone`（改用进程时区） | 失败 2：时间格式表 America/New_York、两个 TZ 下 SSR 标记相同 |
| 删 `CONFIG_LABELS` 的 `gate.command` | 失败 1：settings-config 真实 config.get 行键有标签 |
| 删 `config-warnings.ts` 的 `both_default_models` | 失败 2：八种警告中文、ConfigViolation 全变体 |
| 时间线范围改回 `session.started_at` 原文 | 失败 3：clock 时间线、SSR 相同、task-timeline 无 ISO |

自造检查：`grep toLocale|getHours|new Date()|Date.now` 在 `app/`（clock.tsx 之外）无命中，渲染中无自取时间。
验证缺口：无。实现缺陷：无。ps/监听不涉及。提交由 Autome 兜底（如被拒）。
