# M-07 · 实现 #12 · 快检修复

结论：部分修复，M-07 保持开放，不推进新里程碑。implementation-round: 12/65。截图环境阻塞不是协议失败。

## 根因与修复

宿主退回的两个检查均已在项目测试体系中，原断言未改弱：

- `build_id_cli_and_kernel_agree`：单跑 CLI 集成测试只更新 CLI 和 automed 库，不重建旁边的 automed 可执行文件。复现时 CLI 为 da151e486b89，automed 为 3eae3213065a。快检在 Rust 集成测试前没有 Build core，直到后面的 Web 测试才构建。修复 quick.mjs，为所有 `--test` / `--tests` Rust 候选声明 Build core 前置，覆盖同类 sibling binary 使用者；不修改内核或业务行为。
- `三张图片都是 1512×944 PNG`：README 三图仍是旧图，dashboard 为 1200×451。截图脚本在启动夹具/服务/浏览器之前被 `spawnSync ps EPERM` 拒绝。本轮未伪造、缩放图片，未绕过沙箱，仍未修复图片交付。
- 上一审计为 M-06 通过，没有待搬入的审计缺陷检查。宿主失败检查已原样存在，本轮直接运行它们；另加快检计划与执行顺序回归用例。

## 命令与结果

原始输出在 `.autome/output/`。

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test -p autome-cli --test cli build_id_cli_and_kernel_agree`（改前） | 红，1 例失败，两个提交标识不一致 | r12-build-id-red.txt |
| `node --test --test-name-pattern='Rust integration tests build matching' scripts/test/ci-quick.test.mjs`（改前） | 红，1 例失败，缺 Build core 前置 | r12-order-red.txt |
| `cargo build -p automed -p autome-cli` 后原样重跑 build_id 用例 | 绿，1/1 | r12-build.txt、r12-build-id-green.txt |
| 原样重跑顺序用例（改后） | 绿，1/1，计划和实际执行都先构建后测试 | r12-order-green.txt |
| `(cd apps/web && npx vitest run test/readme-images.test.ts test/test-groups.test.ts)` | 7 例，6 通过、1 失败（尺寸） | r12-images-red.txt |
| `node apps/web/scripts/screenshots.ts --out .autome/output/shots-1` | 退出 1，ps EPERM，未启动服务或浏览器 | r12-shots-1.txt |
| 用户给定的完整快检 if 命令 | 退出 0，Format、Clippy、Web install、Web lint、Web typecheck 通过；明确报告未提供本轮起点，未跑相关测试，不能称原失败集全部通过 | r12-quick.txt |

完整快检实际执行原文：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`。没有运行 ci-local。宿主提交后携带本轮起点重跑才是最终快检结论。

## 自审清单

1. **情形表逐行**：E-1 帮助本轮未改，上一轮 10+67 的结果不算本轮实跑。E-2 README 引用和文件集合通过，尺寸失败；截图生成未到运行阶段。快检新增前置同时覆盖单目标 `--test` 与批量 `--tests`；回归检查验证计划包含前置且实际 BUILD 先于 CANDIDATE。
2. **失败分支**：原构建标识不一致检查红→构建后绿；顺序检查红→修复后绿；图片检查仍红。进程枚举拒绝实跑，保持失败可见。浏览器启动/连接超时/写盘失败后的清理因前置阻塞未到达，不声称通过。
3. **全部迁移**：当前涉及快检准备→构建→集成测试顺序，新增测试覆盖该顺序；图片流程仅达到创建临时根→预检拒绝→finally 删除临时根，未启动服务、内核或浏览器。无新增业务状态迁移，不适用任务状态机检查。
4. **输入域边界**：本轮不改变截图参数、帮助参数或视口输入域，不适用新增输入边界；原 PNG 三文件断言完整保留。测试候选使用现有命令数组中独立 `--test` / `--tests` token 匹配，非字符串子串判断；lib/bin 候选不增加此前置。

## 交接

在允许进程枚举和监听端口的执行环境，运行默认截图脚本替换 docs/images 三图，再运行 shots-1、shots-2 和图片契约。当前 M-07 保持开放，未标待审或已完成。没有手工启动服务/浏览器，不启动下一会话。提交时使用仓库支持的 NO_CI=1，避免 post-commit 自启用户禁止的全量门禁。

## 相关测试收尾与用例总数

- `node --test scripts/test/ci-quick.test.mjs`：14 例，初次 13 通过、1 失败（r12-quick-tests.txt）。失败是原第 13 例要求 Desktop 沿用时完全不构建；现 Rust 集成测试仍需独立 Build core。因此保留原 5 项沿用和 partial 断言，将“不构建”更新为“恰好构建一次且先于 CLI 测试”，未弱化缓存断言。
- `node --test --test-name-pattern='13 static steps' scripts/test/ci-quick.test.mjs`：修正后 1/1 通过（r12-reuse-green.txt）。此前其他 13 例均通过，最终断言改动仅在这一个用例内，未重复跑整文件。
- `node --test --test-name-pattern='Rust integration tests build matching|03 one Web|05 budget|06 failed' scripts/test/ci-quick.test.mjs`：4/4 通过（r12-subset.txt），覆盖新前置、Web 前置、预算中断、失败报告。
- 本轮执行不同用例总数 **22 = quick 14 + build_id 1 + images/groups 7**，最终复验汇总 **21 通过、1 失败**（旧图片尺寸），没有条件跳过；重复执行与负向阶段不重复计数。同范围改前基线 **21**，净增 **1**（新构建顺序用例）。与上一轮实际执行的 **86** 相差 **-64**，因修复轮更换验证范围：不重跑帮助/bin 与源码守卫，加入快检脚本和 build_id；不是删掉 64 条测试。上一轮主验收 84 与本轮 22 差 -62，同样属于范围差异。
- 最终再次原样运行用户快检命令，日志 r12-quick-final.txt；仍无本轮起点，不能替代宿主相关测试。完整 quick 测试中的 ci-local 仅在临时夹具执行 `--plan` 比较缓存键，没有运行项目全量门禁。
- 最终快检退出 0，5 步通过、0 失败、相关测试明确未跑；`git diff --check` 通过。提交范围仅两处 CI 文件与本轮设计状态、证据、retro；通过 `git add` + `NO_CI=1 git commit` 提交当前分支，不推送。
