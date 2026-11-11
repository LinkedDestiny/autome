# M-03 实现 #3：内核接线

结论：验收通过，报待审。本轮仅推进 M-03。上一轮 M-02 审计通过，没有审计发现缺陷的检查需要逐字迁入。

## 实现与修复说明

- config_io 读取并校验 same_failure_limit，包含 command 为空时的越界检查；保存现有配置保留该键。
- gate::log_window 从文件末尾限读 256 KiB，丢弃首行残片，保留末尾 400 行；读失败返回空串。原 tail 与注入内容不变。
- Store 增加按任务、节点、事件种类读取最新结论和检查后续尝试边界的查询；schema 未改。
- run_gate 在 Done 且 head 未变后调用 record_gate_result：失败的三个计数节点追加四个字段；先识别重放，再计算新结果。达到阈值发 GateSameFailure，其它路径仍用原 finish_gate。
- 新增 12 个测试，现有测试无修改。scheduler 的删除行均属于 run_gate 原有事件写入的改写；gate 的两行说明移动仍在测试模块外，config_io/store 无删除。
- 本轮首次验证为 10/12：两条重放夹具误以为初始化必有项目配置，改用现有测试的缺省空文本写法。第二次为 11/12：实现会话启动会提交设计文档、改变 head，下一次假作业必须绑定当前 head。修正夹具后 12/12；生产代码没有为迁就测试改变 head 校验。早期全量使用修正前二进制为 617/618，最终全量重跑通过。

## 命令与结果

- cargo test -p automed --lib same_failure：12 通过、0 失败、0 忽略，606 个被过滤。最终输出：.autome/output/m03-target-final.txt。
- cargo test -p automed --lib：总数 618，全部通过、0 忽略。相对上一轮审计基线 606 增加 12，来自 config_io 4、gate 1、scheduler 7；原 606 个仍全部运行。最终输出：.autome/output/m03-lib-final.txt。
- cargo clippy --all-targets -- -D warnings：退出 0。输出：.autome/output/m03-clippy-final.txt。
- cargo fmt --all -- --check：退出 0，输出文件为空：.autome/output/m03-fmt.txt。
- git diff --check：退出 0。
- 本轮未跑 workspace 全量与端到端；M-04 负责新增 e2e，M-05 负责全量回归。宿主 sh scripts/ci-local.sh 由 Autome 调度，本轮未启动。

## 标待审之前的自审清单

### 1. 情形表逐行

计数表逐行执行：same_failure_other_nodes_and_boundaries 覆盖无历史→1、同指纹→2→3 熔断、已熔断→1、通过→1、旧事件无指纹→1、不同指纹→1；same_failure_reused_resets 覆盖 merge_gate F/F/reused/F→1。全部通过。

节点表逐行执行：same_failure_result_fields 对 gate、merge_gate、fix_gate 各验证原七字段不变且追加四字段；same_failure_excluded_results 对 repro_check、main_check 的失败与 gate 的通过验证不加字段。全部通过。

持久性与隔离：same_failure_database_reopen 关闭再打开同一磁盘数据库得到 2；其它节点与其它任务插入结果不影响当前节点得到 2。全部通过。

不算结果的分支：核对 run_gate 的 Running、Vanished、head 已变分支仍先于 record_gate_result 返回；无门禁配置仍直接 finish_gate，未插入事件。原 gate 消失、停止丢弃、运行超时等测试随 618 个全量运行通过；未声称新造了 head 变化并发实验。

### 2. 失败分支

- 读盘失败：不存在的文件、目录作为日志路径均返回空串；实际结果写入仍可得到兜底指纹，通过。
- 空输入：空日志返回空串，单行中文保留；不存在的日志经指纹兜底参与计数，通过。
- 超时：退出码 124 的结果摘要为“门禁运行超时，被内核终止”，通过；原门禁超时测试亦通过。
- 非法配置：0、11、-1、2.5、字符串、布尔值、数组、超过 u32 的整数，command 非空/为空均报规定中文错误，通过。
- 拒绝授权、外部服务不可达：不适用，本里程碑不新增授权或外部服务调用；门禁命令的非零退出按失败结果处理。

### 3. 全部迁移

- 已落账 count=3/tripped=true、状态仍 Active Gate：实际 run_gate 消费完成作业后 Failed GateSameFailure(count=3)，gate.result 仍为一条，通过。
- 已落账 count=2/tripped=false、状态仍 Active Gate：实际 run_gate 退回 Implement，事件仍一条；其后 session.started、下一次同指纹完成作业→count=3 并失败，事件两条，通过。
- gate.started 后的新运行不当作重放：recorded 辅助函数每次写入该边界，验证 1→2→3，通过。
- gate.reused 后同指纹失败重新从 1 起，通过。
- 熔断后新的尝试从 1 起；旧事件、指纹变化、通过都重新从 1 起，通过。
- 未变更的通过/复现/主分支报警迁移由原库测试回归；rerun/extend 状态机属于已关闭的 M-02，本轮没有修改。

### 4. 输入域边界

- 上限缺省 3，合法最小 1、最大 10，保存 7 后仍为 7，通过；非法域见上。
- 日志为空、单行、500 行截为末 400 行、超过 256 KiB 的中文长行加末行、超过窗口且无换行、不可读路径，均通过。
- 日期、闰日、跨年：不适用，该接口没有日期输入。前导零不是合法 TOML 整数字面量，不扩展 TOML 语法。

## 负向对照

故障注入均在当前文件临时执行并完整还原；最终两条验收命令在还原后重跑：
- 范围改为 0..=10：same_failure_config_invalid 红（0 被错误接受）。
- 查询取消节点条件：same_failure_other_nodes_and_boundaries 红。
- 关闭重放判断：两个 same_failure_replay 用例均红。
- 最新结论查询忽略 gate.reused：same_failure_reused_resets 红。
对应原始输出：.autome/output/m03-negative-{zero,node,replay,reuse}.txt。以上用于证明新测试能抓住指定错误，不是上一轮审计退回缺陷。

## 交付边界

只修改当前 worktree；未改协议、门禁脚本、其它里程碑实现或既有测试，未推送，未启动后续会话。配置与规则未修改；.autome/output 仅存本轮命令输出和临时验证脚本。
