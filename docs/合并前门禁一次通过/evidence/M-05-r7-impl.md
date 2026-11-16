# M-05 实现轮 #7（快检修复）

本轮仅修复宿主快检问题，M-05 保持待审，不推进 M-06。起始 HEAD 为 `f10b795d`；上一轮实现提交 `286f7d6c8c`，重现选择计划用上一轮起点 `734d9df5`。上一轮审计未退回 M-05，没有需逐字迁入的审计检查。

## 根因与修复

用户给出的尾部均为通过用例，不能据此认定某个用例失败或卡死；最后的 Terminated 是宿主 10 分钟超时。当前 worktree 没有 `.autome/output/gates/` 完整日志，不推测具体未完成用例。

1. 同一起点执行 `AUTOME_ROUND_START=734d9df5 sh scripts/ci-quick.sh --plan`：虽明确将函数边界外改动的 end_to_end 留给合并前门禁，golden 文件却又选择 `cargo test -p automed --tests --no-fail-fast`，重新带入整套端到端。这与宿主尾部运行大量端到端用例吻合。修复共享输入分支：按 Cargo metadata 选择 unit 与各独立 integration target，排除整套 end_to_end 并明确记未跑；仍保留既有函数体内改动的精确选择，其它 crate 行为不变。未缩减合并前门禁覆盖，也未延长或关闭预算。
2. 用户指定的原样快检在本会话暴露三处 `clippy::needless_borrow`（launcher 新测试给 `quick_spec` 传 `&tpl`）；移除这三处多余借用。上一轮只跑 lib clippy 未覆盖这些测试代码。修复不改变产品逻辑。

README 和设计 C-2 同步选择规则。只更新 M-05 最新证据指针；M-01 至 M-04 状态与证据未动。

## 红绿与命令

- 新增项目回归 `17 shared automed test inputs never admit the whole end_to_end target`，真实临时 Git 仓库改动提示词 golden，运行真实快检 plan，断言显式 prompt_rendering 候选、没有 `--tests` / 整套 end_to_end、未跑说明存在。
- 修复前 `node --test --test-name-pattern='17 shared' scripts/test/ci-quick.test.mjs`：1 个失败，首先命中缺少独立 prompt_rendering 候选；计划另直接显示错误 `--tests` 候选（`r7-red.txt`、`r7-plan-before.txt`）。修复后同命令：1 个通过（`r7-green.txt`）；同起点计划不再含 `--tests`（`r7-plan-after.txt`）。以上输出均在 `.autome/output/`。
- 首次用户原样快检：失败 1 步 Clippy，其余静态步骤通过，68 秒。会话没有继承 AUTOME_ROUND_START，相关测试明确未跑；此结果不冒充宿主环境验证（`r7-quick.txt`）。
- M-05 Rust 验收：`cargo test -p automed --lib config_io::tests::gate_quick_session && cargo test -p automed --lib launcher::tests::round_gate_line && cargo test -p automed --test prompt_rendering && cargo test -p automed --test end_to_end gate_quick_round_start && cargo test -p automed --test end_to_end gate_deadline`，分别 3、3、49、2、1，共 58 个通过（`r7-acceptance.txt`）。其中 launcher 筛选测试首次运行在去掉多余借用之前，最终代码由带起点快检内的整库测试覆盖。

- 带上一轮起点重跑用户原样快检：命令前导出 `AUTOME_ROUND_START=734d9df5`，其余命令与用户一致。退出 0，208 秒，9 步、0 失败；prompt_rendering 49、autome-domain lib 487、automed lib 1044 全部通过（`r7-quick-final.txt`）。相关阶段 156 秒，最后准入 automed lib 从第 6 秒开始、用时 150 秒，后续 3 个目标因准入预算未跑；这遵守既有预算语义，未把未跑记通过。
- `node --test scripts/test/*.test.mjs`：71 个，70 通过、1 失败（`r7-scripts.txt`），失败为既有第 14 例依赖真实一秒时钟，前置发现步骤已耗完预算（设计 BL-07）。测试改成夹具私有 date 替身，初值 100，第一步运行后写 102，第二步应被 1 秒预算拒绝；保留全部原断言。第 14 例单跑转绿（`r7-budget-green-final.txt`）。实现过程中首次假时钟版本有生成脚本转义错误，已修正，不将该次算通过（`r7-budget-green.txt`）。这是当前运行抓到的缺陷，不冒充上一轮审计退回检查。

- 最终工作区以本轮起点 `AUTOME_ROUND_START=f10b795d` 运行用户原样快检：退出 0，147 秒，6 步、0 失败；automed lib 1044 个通过，相关阶段 125 秒（`r7-current-quick.txt`）。未因超出准入预算 5 秒而中断已准入的测试，与设计一致。

- 最终 `node --test scripts/test/ci-quick.test.mjs`：18 / 18 通过，0 失败、取消、跳过，约 203 秒（`r7-ci-quick-final.txt`）。修改后的整个文件复验通过；其它脚本文件的 53 个用例沿用本轮全套那次的通过结果，未宣称那次全套命令退出成功。

**用例总数及基线差：** M-05 主验收 76 / 76（上一轮 75，+1 为新增共享输入回归）；脚本体系本轮覆盖 71（上一轮 70，+1，同一条回归），首跑 70 绿 1 红，修复后重跑受影响文件 18 绿，其余 53 未变且首跑绿。补充 automed lib 1044、domain lib 487 均与上一轮数量相同。所有本轮命令去重后共 1654 个（上一轮同范围 1653，+1）；筛选重跑、红绿过程及两次快检不重复计数。首次脚本全套失败没有隐藏；最终修改涉及的测试已全部复验通过。

## 四项自审

1. **情形表逐行**：直接 integration 文件仍只选目标；automed src 仍选 lib；共享 golden/fixture 改动选 unit 与独立 integration，明确不选整套 end_to_end；end_to_end 函数体内、helper/删除/边界变动、Rust 字符串伪函数沿用第 08 例；其它 crate 共享输入沿用第 11 例。快检脚本测试逐项检查候选、优先级与去重。
2. **失败分支**：无起点显式未跑、非法起点退出非零、非法预算拒绝、报告不可写拒绝、相关测试失败返回 1 均由现有脚本测试执行；新回归已取得修复前红、修复后绿。外部服务不可达及授权拒绝不适用：本次修复只改变本地测试候选。宿主超时原样再跑不是断言失败证据，不声称已复现全部十分钟过程。
3. **全部迁移**：不适用产品状态机，本次未修改调度或状态迁移。脚本候选准入→运行、预算不足→未跑、部分成功不发布整套通过记录由原测试第 04/05/06/12/13 例覆盖；新路径明确将整套 end_to_end 记未跑，合并前仍完整执行。
4. **输入域边界**：现有测试覆盖零预算、前导零、非法/溢出预算、无起点、非法 revision、带空格/引号/中文路径、删除文件及函数边界；新增共享输入路径使用真实 golden。日期、闰日、跨年不适用：没有日期输入变化。

未运行 `sh scripts/ci-local.sh`。本地通过不能代替提交后 Autome 快检结论。

状态块与里程碑表检查通过：7/30，全部里程碑状态与轮初一致，M-05 单行证据指针；`git diff --check` 通过。
