# M-01 实现轮 #1

结论：验收通过，提交独立审计；implementation-round 为 1/20。本轮仅推进 M-01。此前无审计轮，无需搬迁审计回归检查。

## 实现

新增项目级 MergeAfterConfig、默认超时 10、ResolvedConfig 传递与纯函数 merge_after::plan；config_io 在 setup 与 gate 之间解析 merge 表。save_project 保持原有实现，由测试确认未知表、注释及未知键保留。现有测试未修改，新增 D1–D4、C1–C4 共 8 个用例。运行时接入留给 M-02。

## 命令与结果

原始日志仅在 .autome/output/，没有复制到任务文档。

- `cargo test -p autome-domain > .autome/output/m01-domain.txt 2>&1`：退出 0；388 个单元用例通过，相对设计轮基线 384 增加 4（D1–D4）；另有既有 doctest 1 个 ignored，0 个执行，未新增忽略。
- `cargo test -p automed --lib > .autome/output/m01-lib.txt 2>&1`：退出 0；651 个用例通过，相对基线 647 增加 4（C1–C4），0 失败、0 忽略。
- `cargo test -p autome-domain merge_after > .autome/output/m01-domain-filter.txt 2>&1`：退出 0，4 个通过，384 个单元用例被过滤。
- `cargo test -p automed --lib merge_after > .autome/output/m01-lib-filter.txt 2>&1`：退出 0，4 个通过，647 个被过滤。
- `cargo fmt --all -- --check > .autome/output/m01-fmt-check.txt 2>&1`：通过。
- `cargo clippy --all-targets -- -D warnings > .autome/output/m01-clippy.txt 2>&1`：退出 0，通过。
- `git diff --check`：通过。

受影响目标合计执行 1039 个单元用例，较设计轮基线 1031 增加 8，差额完全来自本轮新增测试；本轮为首个实现轮，无上一实现轮基线。workspace 全量、e2e、desktop/web 留至后续里程碑，本轮不声明通过。

## 标待审之前的自审清单

1. **情形表逐行**：全部已执行。
   - 工作区：D4 对有/无提交 × 四种 verdict 共 8 组均返回「工作区项目不支持合并后命令」。
   - 单仓无提交：D4 对四种 verdict 均返回「没有单一的合并提交」。
   - 单仓有提交、Failed：D3 返回「验证 main 未通过」。
   - 单仓有提交、Skipped：D3 返回「验证 main 被跳过」。
   - 单仓有提交、NotRun / Passed：D2 两组均为 Run。总计完整覆盖 16 个输入组合。
   - 配置：C1 检查去空白、显式超时及未知键；C2 检查缺省 10、缺节/空表/缺 after/空串/空白忽略超时；C3 检查拒绝非法类型；C4 改 loop 后保存，重新加载相同配置且原 merge 段原文保留。D1 检查缺省反序列化、None 不序列化和 resolve 有/无配置。
2. **失败分支**：C3 分别执行非表（整数、布尔、数组、字符串）、非字符串 after（整数、布尔、数组、表）、非法超时（0、-1、字符串、浮点、布尔、数组、表），全部得到设计指定错误文字。空输入在 C2 验证为 None。拒绝授权、读盘失败、命令超时、服务不可达不适用：本里程碑不新增授权、读盘或进程/网络执行路径。
3. **全部迁移**：不适用，本轮不改变任务状态机；纯判定的 16 个组合已全部验证，配置 None/Some 到 resolved 的传递均验证。
4. **输入域边界**：C1 验证 1、3、u32::MAX、u32::MAX+1、i64::MAX，后两者截到 u32::MAX；C2 验证空串、空格与 tab、无命令时非法超时被忽略；C3 验证零、负数与非整数。前导零为 TOML 本身的语法规则，本轮未改 TOML 解析器；闰日、跨年、集合单元素不适用（本接口不接收日期或集合）。

## 修复与限制

没有上一轮退回缺陷，也没有验收失败后修复。新增类型与计划不依赖 I/O、async 或 SQLite；本轮不执行用户命令。未修改 .autome 配置或规则，只有按协议写入被忽略的 output 日志。
