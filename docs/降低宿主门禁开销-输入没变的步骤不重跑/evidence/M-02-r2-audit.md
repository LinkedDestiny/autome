# M-02 · 审计复验（实现轮 #2，审计 #2）

结论：**通过**（含一项验证缺口，已就地补强契约并复验，不计 reopen）。

## 验收命令
- `node --test scripts/test/ci-reuse.test.mjs scripts/test/ci-claims.test.mjs`：19 个全过（ci-reuse 18 + ci-claims 1），与契约一致。
- `vitest run test/ci-config.test.ts`：9 个全过（数量不变）。
- 宿主快检（fmt/clippy/lint/typecheck）在 0155b6a6 通过，仅作引用。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 期望失败的用例 | 结果 |
|---|---|---|
| Web test 输入去掉 `crates` | 02 | 红 ✔ |
| 键里去掉 `unclaimed` 哈希 | 05 | 红 ✔ |
| 接受 worktree 内快照（跳过位置判断） | 07 | 红 ✔ |
| 不清除 `AUTOMED_BIN` 等变量 | 12 | 红 ✔ |
| `Build core` 当成可沿用步骤（prep 永不执行） | 11 | **仍绿 ✘** |

## 验证缺口
用例 11 的 `indexOf('==> Build core') < indexOf('==> Web test')` 在 Build core 缺席时为 `-1 < n`，恒真；后半「全部沿用时不出现」也不区分。所以契约里「Build core 写成普通可沿用步骤时用例 ⑪ 失败」的对照实际放得过。
产品行为没错：自造判别检查 `.autome/output/m02-buildcore.test.mjs`（要求 `indexOf >= 0` 且输出 `BUILD`）在当前实现上绿、在 prep 永不执行的注入下红（已还原）。
处置：契约 ⑪ 加上「必须断言 Build core 日志与 BUILD 输出确实出现」；测试文件的修改写入 Backlog BL-02（审计轮不改项目测试）。

## 同类检查
Rust 测试的输入 `crates protocol`：grep 未见 crates 测试读 plugins/apps/docs 的路径（仅临时目录内的 docs 字面量），与 B.1 一致。
