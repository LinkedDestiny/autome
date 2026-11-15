# M-02 r4 审计复验

结论：通过。

## 验收命令
- `cargo build -p automed -p autome-cli`：通过。
- vitest 9 个文件：74 个用例全过（契约预期 74）。
- `onboarding-watch.ts` 不存在、`onboarding-disk.ts` 存在、grep 无命中：成立。

## 负向对照（逐条注入、跑对应测试、还原，`git status` 确认干净）
1. `canConfirm` 不看 `rejection`：add-project「12 四种拒绝在填表前显示且无确认表单」失败（1 failed / 14 passed）。
2. `loadBoard` 任一 `task.get` 失败即整页报错：board「9 坏配置下卡片仍渲染且不带详情」失败。
3. `configRows` 忽略 `defaults`：settings-config「13 默认段全部且只有它们有默认徽标」失败。
4. 设置页不用磁盘标记（`false && disk…`）：settings-onboarding「8 账本运行中才读磁盘标记，结束时不读盘」失败。
5. 额外：去掉 `state === 'running'` 守卫（总是读盘）：同一用例失败。

## 自造判别检查
第 5 项是契约外自造的变体（账本已结束时不得读盘），能被拦。无产品缺陷、无验证缺口。
未跑 Playwright / 监听类测试（沙箱限制，验收命令本身不含）。
