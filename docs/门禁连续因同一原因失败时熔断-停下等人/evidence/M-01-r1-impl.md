# M-01 · 实现 #1

结论：待审。仅实现 domain 指纹、节点范围、连续计数、T-14 夹具与本机只读验证；无上一轮审计，无需搬入审计回归检查。

## 实现说明

新增 gate_failure 模块，无 I/O，使用已有 sha2。PreviousResult 由调用方提供同任务同节点的前值。计数饱和加法避免 u32 回绕。设计 §1「160 字后加省略号」与验收「不超过 160 字符」冲突，按后者实现 159 字加省略号，并就地明确设计措辞。配置越界校验、状态机、调度接线仍归后续里程碑。

源日志退出标记实际为多行 exit_code=… 格式，集成验证按字段解析。按 started 数值排序，共 25 次 gate、21 次失败，复制失败序列 01/18/19 的最后 400 行。源日志未修改。

## 验收与基线

原始输出均在 `.autome/output/`。本轮开始 `cargo test -p autome-domain`：345 个库测试通过（m01-baseline.txt），与设计基线相同。

| 命令 | 结果及总数 | 输出文件 |
|---|---|---|
| `cargo test -p autome-domain gate_failure::` | 16/16 通过，新增 16 | m01-unit-final.txt |
| `cargo test -p autome-domain` | 361 个库用例通过，另 1 个集成用例默认忽略，文档用例 0；总计 362，比基线多 17：16 单测 + 1 本机验证 | m01-domain.txt |
| `AUTOME_T14_GATE_DIR=/Users/dannie/project/autome-v2/.autome/output/gates/T-14 cargo test -p autome-domain --test t14_gate_logs -- --ignored` | 1/1 通过：失败 01–18 相同、18≠19、20=21，前 18 次摘要含 ws | m01-t14-final.txt |
| `cargo clippy -p autome-domain --all-targets -- -D warnings` | 通过；首次 get_first 提示已改 first | m01-clippy.txt |
| `cargo fmt --all -- --check` | 通过 | m01-fmt.txt |
| `git diff --check` | 通过 | 无输出 |

未改任何旧测试。workspace 全量和全 targets clippy 留待 M-05；宿主门禁由 Autome 调度。

## 负向对照

逐个临时变异，命令均为 `cargo test -p autome-domain gate_failure::`，每次总数 16：

- 禁用数字归一化：13 通过、3 失败，volatile_details、T-14、前导零断言捕获；m01-negative-digits.txt。
- 窗口 400→60：14 通过、2 失败，T-14 摘要缺 ws 和窗口边界捕获；m01-negative-window.txt。
- 去掉 tripped 检查：15 通过、1 失败，tripped_result_resets 捕获；m01-negative-tripped.txt。

变异已全部恢复，随后 16/16 和 361/361 重跑为绿。

## 标待审之前的自审清单

1. **情形表逐行**：§2 没有前值→1（first_failure）、通过/沿用通过的 ok=true→1（passed_or_changed）、无指纹旧事件→1（legacy_result）、已熔断→1（tripped_result）、指纹不同→1（passed_or_changed）、相同→累加并比较上限（matching_failures），全部执行通过。节点表 Gate/MergeGate/FixGate=true，ReproCheck/MainCheck=false；only_three_nodes_count 遍历 ALL+BUG 所有节点，通过。沿用事件映射属于 M-03。
2. **失败分支**：超时固定指纹且区别普通失败、空串、纯空白、无关键行取最后五非空行均通过；不同错误/退出码/步骤集可区分。授权拒绝、读盘失败、外部服务不可达不适用：本轮纯函数不做授权或 I/O，日志读失败属 M-03。
3. **全部迁移**：计数六种前值迁移已逐行执行；limit=1…10 各跑首次、递增、到阈值，熔断后重置通过。任务状态机迁移不适用（M-02）；Vanished 等不调用本函数的调度行为属 M-03。
4. **输入域边界**：空串/单行/空白、五行兜底、400/401 行、ANSI/CR、中文与 Unix/~/Windows 路径、HTTP/WS 地址、时间/端口/耗时/哈希/前导零、TS2307≠TS7006、重复与重排、摘要 159/160/161/400 字符、上限 1…10、u32::MAX 均在 16 个测试中执行通过。闰日/跨年不适用：不解析日历日期。
