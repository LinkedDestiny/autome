# M-08 实现轮 #8

## 改动与审计输入

README 的 Host gate 示例新增默认 run，英文段落说明 before_merge 的次数、延迟发现回归的代价、会话测试范围、Bug 复现与 MergeGate 分工；技术设计 §8.1 用中文补全同样的取舍，以及五份模板、gate_line、审计简报/复核 prompt、切换时 gate.skipped。两处保留失败重跑与 MainCheck，默认模式明确为 each_round。

上一轮 M-07 审计通过，没有退回缺陷或需要逐字迁入项目测试的检查。本轮不修改产品代码，不新增测试。

## 验收命令与结果

原样执行设计 M-08 契约命令：

```sh
for p in 'run = "each_round"' before_merge 'once per task' 'not until the merge' 'sessions do not run the full gate themselves' 'only the repro command'; do grep -qF "$p" README.md || { echo "README 缺: $p"; exit 1; }; done; for p in before_merge 每个任务一次 合并前才暴露 多退回一次实现轮 会话不再各自兜底 只跑复现命令; do grep -qF "$p" docs/development/technical-design.md || { echo "技术设计缺: $p"; exit 1; }; done; cargo fmt --all -- --check
```

退出 0，无输出；命令副本 `.autome/output/m08-acceptance.sh`，原始输出 `.autome/output/m08-acceptance.txt`。`git diff --check` 退出 0。

本轮验收共 12 个文案断言及 1 个格式检查，全部通过；另有 3 项负向对照，全部按预期拒绝。Rust 测试用例运行总数 0，较上一轮运行的 87（protocol 67 + prompt_rendering 20）减少 87：本轮契约仅验证文档及格式，不运行那两组测试，并非删除或跳过测试。项目测试用例增删为 0。全量测试、clippy 与桌面/Web 测试由宿主在本轮提交后运行，本轮不声称已取得该结果；真实模型和人工验收 H-01/H-02 未运行。

## 标待审之前的自审清单

1. **情形表逐行**：按设计 §9 逐项核对。README 默认配置行、once per task、not until the merge、再退回实现轮、sessions do not run the full gate themselves、验收/复现与相关子集、Bug only the repro command 均齐全。技术设计含对应中文取舍、五份模板与 gate_line、审计/复核文案及 gate.skipped；MergeGate 重试与 MainCheck 保留。12 项机器断言通过，其余语义逐项人工阅读核对通过。
2. **失败分支**：设计 M-08 点名的三项负向对照全部实际执行，详见下文。拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用：本轮仅静态文档，无新增运行分支。
3. **全部迁移**：不适用，本轮不修改状态机。文档覆盖默认逐轮、before_merge 跳过 Gate、Bug 保留 FixGate、MergeGate 失败回修再验及合并后 MainCheck；未将本轮文案验收冒充运行迁移测试。
4. **输入域边界**：不适用，无新增输入解析域。示例保留 each_round 默认并给出 before_merge 可选值，不宣称支持其它配置值。

## 负向对照与恢复

- 临时移除 README 的 `sessions do not run the full gate themselves`：原契约退出 1，报告 `README 缺: sessions do not run the full gate themselves`。输出 `.autome/output/m08-negative-readme.txt`。
- 临时移除技术设计的 `会话不再各自兜底`：原契约退出 1，报告 `技术设计缺: 会话不再各自兜底`。输出 `.autome/output/m08-negative-design.txt`。
- 临时在 domain lib.rs 添加未格式化的 `const _:u8=1;`：原契约的 cargo fmt 检查退出 1。输出 `.autome/output/m08-negative-fmt.txt`。
- 三项均在 ensure 中还原原文件后再运行完整契约，退出 0；产品源码无残留修改。首次辅助脚本处理中文时遇到 Ruby 编码不匹配，在写入中文文件前终止；改用 UTF-8 读取后完成检查，不影响产品与验收结果。

结论：M-08 验收通过，标待审，交独立审计；本轮不关闭里程碑。
