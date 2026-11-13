# M-01 · 实现 #2（快检修复轮）

结论：修复宿主在 f320b6fe92 上发现的 preserve-caught-error 错误，本地同一快检命令退出码 0。最终快检以 Autome 提交后重跑为准。本轮不推进新里程碑；M-01 原为开放，保持开放，implementation-round 更新为 2/30。不存在上一轮审计文件。

## 修复与回归

- 根因：task-detail-real.test.ts 的 beforeAll 在 setup 与 cleanup 同时失败时抛 AggregateError，errors 数组保存了两个错误，但未以 cause 关联当前捕获的 cleanupError。
- 修复：为现有 AggregateError 添加 { cause: cleanupError }，保留 errors 数组、原消息、cleanup 调用及外层原错误重抛。
- 同类检查范围：该测试的 setup catch 与 process-leak.ts 清理辅助函数；本范围只有这一处 AggregateError 包装。
- 红：原样运行 npm run lint --prefix apps/web，出现同一文件 26:53 的 preserve-caught-error，1 error、0 warnings。原始输出：.autome/output/m01-r2-lint-red.txt。
- 绿：修复后运行用户指定完整快检，退出码 0（包括同一 npm lint 命令）。原始输出：.autome/output/m01-r2-quick.txt。检查已在项目 ESLint 体系内，无审计临时检查需要搬入，不新增重复测试。

## 验证命令

```sh
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web
```

结果：退出码 0；git diff --check 通过。未运行 sh scripts/ci-local.sh、全量 Vitest 或 M-01 集成验收。本轮用户最后指令明确为快检修复轮，故仅运行指定快检。

用例总数：本轮 Vitest 执行 0 条，较上一轮 M-01 验收的 22 条少 22 条，原因是本轮仅重跑静态快检，没有删改任何用例。测试定义仍为 22 条（差 0）；不将静态检查通过计作集成用例通过。上一轮 21/22 与 ps EPERM 阻塞未在本轮重验或消除。

## 自审清单

1. 情形表逐行：本轮仅涉及 setup 成功（不变）、setup 失败且 cleanup 成功（原样重抛，不变）、两者失败（保留双错误并新增 cause）；逐分支源码检查完成，ESLint 与 typecheck 通过。完整 M-01 情形表不适用本轮快检修复范围，未重验且未标待审。
2. 失败分支：已实际复现 lint 失败并通过同一检查；setup/cleanup 双失败的运行时路径本轮未重跑，静态核对未改变清理行为。授权、读盘、外部服务等不适用此次错误元数据修复。
3. 全部迁移：不适用，本轮不改产品状态机、事件索引或清理顺序；里程碑保持开放。
4. 输入域边界：不适用，只增加 ErrorOptions.cause 的错误对象引用，不改变输入解析、大小或时间边界。

## 后续

下一实现轮仍需取得 M-01 完整验收与进程清理证据，不能以本轮快检通过代替。未切换分支、未推送、未启动下一会话。
