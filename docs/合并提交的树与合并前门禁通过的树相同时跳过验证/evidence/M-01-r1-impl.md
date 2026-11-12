# M-01 实现轮 #1 证据

结论：验收通过，提交待独立审计。实现轮 1/15，仅推进 M-01；没有上一轮审计或须搬入的审计回归用例。

## 实现与范围

- ProjectConfig 增加可选开关，缺省序列化省略；ResolvedConfig 缺省 true，显式 true/false 均覆盖。
- `[gate]` 在检查 command 之前严格读取布尔值；非布尔报告 `[gate] reuse_for_main_check 必须是 true 或 false`。保存逻辑不改，测试覆盖保存后重读。
- 新增 `git::tree_id` 和 `git::second_parent`，分别使用 Git `rev-parse --verify --quiet <commit>^{tree}`、`<commit>^2`，错误沿现有 Result 返回。
- 三个源码文件只有新增行；原有测试不改。未接入调度器，运行时行为不变。

## 命令与结果

验收命令：`cargo test -p autome-domain config:: && cargo test -p automed --lib config_io:: && cargo test -p automed --lib git::`。

最终结果：32、43、38 个用例全部通过，合计 **113**，失败 0、忽略 0；相对设计轮基线 31、40、35（合计 106）增加 **7**，来自配置域 1、配置读取 3、Git 3 个新测试函数。基线也以 HEAD 中三个文件的 `#[test]` 数量核对为 31/40/35；无上一实现轮。首次以 baseline 命名的日志因后台编译与编辑重叠，实际已包含新增用例，不将其冒充修改前实跑证据。

最终日志：`.autome/output/m01-final.txt`。`autome-domain` 的额外二进制测试目标因过滤器匹配 0 个，不计入上述总数。

`cargo test --workspace -- --list` 成功，枚举 **1216** 个测试，相对设计基线 1209 增加 7（`.autome/output/m01-test-list.txt`）；这是枚举与编译证据，不是全量运行通过。全工作区测试与 `ci-local` 按契约留给 M-03。

`cargo fmt --all -- --check`、`git diff --check` 通过。

## 标待审之前的自审清单

### 1. 情形表逐行

M-01 没有独立 S 编号表，逐行核对方案 §4 与新增单元测试清单；S1–S11 属于 M-02，不在本轮接入。

| 情形 | 检查与结果 |
|---|---|
| 无 gate 或未写开关 | parse 返回 None；resolve 为 true，通过 |
| 显式 true / false | 读取与 resolve 覆盖均通过；command 非空与空字符串均覆盖 |
| 非布尔 | 字符串 false/true/yes/空串、整数 0/1/-1、浮点、数组、表、日期均精确断言中文错误，通过 |
| save_project 保留手写开关 | true、false 各保存并重读，resolve 仍取该值，通过 |
| no-ff 合并与被合并 head | 提交 SHA 不同而 tree ID 相同，通过 |
| 新增内容再提交 | 新树与原合并树不同，通过 |
| 合并第二父提交 | 等于 feature head，通过 |
| 普通提交或未知提交 | 返回错误，通过 |

### 2. 失败分支

- 非布尔配置：上述所有类型均拒绝，包括 command 为空时仍拒绝。
- tree_id：未知引用、空输入、全零不存在 SHA、非 Git 目录均返回错误。
- second_parent：根提交、普通单父提交、未知引用、空输入均返回错误。
- 拒绝授权、超时、外部服务不可达：不适用；本里程碑没有授权、网络服务或超时状态分支。文件权限拒绝未模拟；非 Git 目录覆盖 Git 查询失败向上传递。

### 3. 全部状态迁移

不适用：本里程碑只有配置和只读 Git 基础函数，不改任务状态机；配置 true/false 的保存→加载→resolve 两条路径均已实跑。

### 4. 输入域边界

- 布尔域两端 true/false、缺省 None 全覆盖；拒绝字符串布尔、空串、数值和集合类型。
- Git 输入覆盖完整提交 SHA、HEAD 引用、未知引用、空串、全零 SHA；父提交覆盖 0、1、2 个父提交；树覆盖相等与不等。
- 前导零、跨年、数值最大值不适用：开关域仅为布尔、Git 参数为内部提交引用，不是日期或整数域；日期值作为错误 TOML 类型已拒绝。

## 设计要求的负向对照

在本轮实现上逐项暂时注入错误，使用项目测试运行，均退出 101 且对应断言失败（非编译失败）；每次用 ensure 恢复原文件，最后完整重跑上述验收为绿。

| 注入 | 命令过滤器 | 红结果日志 |
|---|---|---|
| 默认值改为 false | `cargo test -p autome-domain gate_reuse_for_main_check_defaults_true` | `.autome/output/m01-negative-default.txt` |
| 字符串 parse bool 兜底 | `cargo test -p automed --lib gate_reuse_for_main_check_rejects_non_booleans` | `.autome/output/m01-negative-boolean.txt` |
| tree_id 改取 commit SHA | `cargo test -p automed --lib tree_id_of_a_no_ff_merge` | `.autome/output/m01-negative-tree.txt` |
| second_parent 改取 ^1 | `cargo test -p automed --lib second_parent_of_a_no_ff_merge` | `.autome/output/m01-negative-parent.txt` |

四项各运行 1 个测试、各失败 1 个，属于同一测试的负向重跑，不增加用例总数。原始输出仅在 `.autome/output/`，没有转存到任务文档。
