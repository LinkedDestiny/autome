# M-06 · 实现 #6 的审计 #6 复验证据

| 项 | 结果 |
|---|---|
| `vitest run test-groups core-sidecar eventbus-live logtail-live-cleanup` | 30 通过（4/18/7/1），需先 `cargo build` 刷新 target/debug |
| 负向 eventbus-live 入 unit | test-groups 用例 2 红，已还原 |
| 负向 去 globalSetup + 无 build/ | test-groups 用例 3 红；gateway-http 11 个红（build/server/index.js 缺失），已还原 |
| `npm run test`（全量） | 失败：vitest 5.1.0 报 kernel 与 unit 的 maxWorkers 不同而 groupOrder 相同，no tests |
| 交替基准 / 5 连跑 | 无法取数（宿主 load 约 7，也非空闲）；H-01 保留 |
| ENOTEMPTY 结论 | 未取得 |

结论：实现缺陷，reopen 1，领域「Web 测试分组配置」。判别检查 `.autome/output/m06a/full-run-check.sh`。工作区已还原，范围守卫外无改动。
