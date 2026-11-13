# M-07 · 实现 #8（快检修复轮）

## 修复与范围

宿主在 6d7402ff10 上的快检因 ci-config.test.ts:62 的 no-regex-spaces 失败。本轮把正则里的两个连续空格改为 ` {2}`，仍精确匹配两个 ASCII 空格，不改测试断言、脚本或产品行为。全 Web lint 检查同类问题通过。M-07 保持待审，不推进新里程碑。
上一轮审计为 M-06 通过，没有审计退回检查需要迁入；本次失败检查已经在项目 ESLint 体系内，保留规则并原样运行 lint 复现，不新增重复测试。

## 命令与结果

- 红：`npm run lint --prefix apps/web`，报告 1 个 error、0 warnings，ci-config.test.ts:62:32 为 no-regex-spaces。原始输出：`.autome/output/m07-r8-red.txt`。
- 绿：`cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`，退出码 0。原始输出：`.autome/output/m07-r8-quick.txt`。这是提交前本地结果，最终快检以 Autome 提交后重跑为准。
- 验收：`npm run test --prefix apps/web -- test/ci-config.test.ts && sh -n scripts/ci-local.sh`，退出码 0；总数 8，8 通过、0 失败、0 跳过，shell 语法检查通过。与实现 #7 同一子集基线 8 相比差 0；仅改正则表达方式，未增删用例。原始输出：`.autome/output/m07-r8-accept.txt`。
- 未运行 `sh scripts/ci-local.sh` 或真实安装，全量门禁留给宿主。

## 标待审之前的自审清单

本轮保持原有待审状态，仍逐项复核：

1. **情形表逐行**：现有 7 个 CI 配置检查及 1 个 package 行为测试全部通过；行为测试逐一执行成功 0 与失败 37 两种情形，均验证隔离和清理，成功才执行版本检查。
2. **失败分支**：安装退出 37 的分支实跑，验证退出码原样传播且目录清理；lint 原报错已复现并修复。拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用，本轮与 M-07 设计均未新增这些失败分支。
3. **全部迁移**：实跑临时根创建 → 安装成功 → 版本检查 → 清理，以及创建 → 安装失败 → 清理 → 原退出码返回；无产品状态机或多通道。
4. **输入域边界**：含中文与空格的临时路径、调用者已设 PREFIX/AUTOME_HOME 均由行为测试覆盖；正则仍精确匹配两个空格，实际脚本提取和执行通过。日期、数值极值等不适用，无此输入域。

结论：本地快检及 M-07 验收通过，等待宿主快检与独立审计。
