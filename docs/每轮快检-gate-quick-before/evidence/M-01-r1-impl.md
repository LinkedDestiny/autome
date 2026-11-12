# M-01 实现 #1

结论：配置解析与需求状态机验收通过，提交审计；本轮仅推进 M-01。

## 改动与范围

- ProjectConfig / ResolvedConfig 旁挂 QuickGate；缺省超时 5，未配置时省略 JSON 字段，旧 JSON 可读。
- parse_project 按设计 §1 校验类型、超时和完整门禁依赖；全局读取与 save_project 实现不变。
- Implement 完成只将 gates_each_round 判断改为 gates_after_implement。宿主实际选命令归 M-02，Bug 执行归 M-03，本轮未实现。
- 无上一轮审计，无需搬运审计回归检查。现有测试未修改。

## 命令与结果

原始输出均在 .autome/output/，未转存到任务目录。

| 命令 | 结果 | 基线与变化 | 日志 |
|---|---|---|---|
| cargo test -p autome-domain -- gate_quick before_merge | 退出 0，16 个通过，0 失败/忽略 | 修改前 10，+6 个 gate_quick 配置/迁移用例 | m01-domain-baseline.txt / m01-domain-final.txt |
| cargo test -p automed --lib -- config_io::tests::gate_quick config_io::tests::before_merge | 退出 0，13 个通过，0 失败/忽略 | 修改前 5，+8 个 gate_quick 解析/保存用例 | m01-config-baseline.txt / m01-config-final.txt |
| cargo fmt --all -- --check | 退出 0 | 非测试计数 | m01-fmt-check.txt |
| cargo clippy --all-targets -- -D warnings | 退出 0 | 非测试计数 | m01-clippy.txt |
| git diff --check | 退出 0 | 无空白错误 | 无输出 |

本轮验收总数 29，相对修改前实跑基线 15 增加 14；这是首轮，没有上一实现轮基线。domain 附属测试目标匹配 0 个，不计入新增数。未跑全量门禁或无关测试。

## 标待审之前的自审清单

### 1. 情形表逐行

设计 §1 的十行情形均通过：
1. command + quick：default_and_explicit_timeout 验证去首尾空白、默认 5 和显式超时，并 resolve 后生效。
2. 只有 quick，有/无 concurrency：requires_full_command 均拒绝。
3. 空/空白 command + 非空 quick：同用例拒绝。
4. 空 command + 空 quick：type_and_whitespace 验证无门禁、无快检。
5. 非空 command + 空/空白 quick：同用例验证无快检。
6. quick 为整数/布尔/数组：同用例验证指定中文类型错误。
7. 只有超时：timeout_requires_quick 验证指定依赖错误。
8. 非正数/非整数超时（包括空 quick）：timeout_bounds 验证指定错误。
9. each_round + quick：each_round_has_no_violations 验证解析保留配置、快检不生效、validate 无 violations。
10. 全局两个键：global_is_ignored 经文件 load_global 成功且与默认值相等。
另 save_preserves_text_and_reload 验证角色保存后两个键及注释原样保留、重载生效。

### 2. 失败分支

- 配置类型错误、缺 quick、缺完整 command、超时非法均以上述项目测试逐条执行通过。
- Gate 失败返回实现与预算耗尽失败：gate_pass_and_fail_equal_each_round 在审计开/关、轮次 1/25 下逐字段比较既有 each_round 转换，通过。
- 拒绝授权、读盘失败、运行超时、外部服务不可达：本里程碑不适用；未新增授权/外部调用/执行器或读写文件实现，宿主运行故障属于后续里程碑。空输入属于本轮，已覆盖。

### 3. 全部迁移

- Implement + 正常结束：审计开启 Pending，以及审计关闭 Open/Done 均进 Gate，且转换逐字段与 each_round 相同。
- Gate + Passed/Failed × 审计开/关 × 轮次 1/25 全部执行，与 each_round 相同（含预算耗尽）。
- quick 在 each_round 或无完整门禁时不生效，与无 quick 的 Implement 转换相等。
- Rebase → MergeGate、Merging → MainCheck 执行并与 each_round 逐字段相等。
- 既有 before_merge 10 个测试保持原样通过，覆盖无 quick 的既有路径。

### 4. 输入域边界

- 字符串空串、纯空白、首尾空白；整数超时 1、0、-1、u32::MAX、i64::MAX（截到 u32::MAX）；字符串/浮点/布尔/数组超时均执行通过。
- 旧 ResolvedConfig JSON 无新字段，反序列化得到 None、重新序列化字节相同；有 quick JSON 往返相等；resolve 搬运项目字段。
- 前导零、闰日、跨年不适用：本轮不新增日期或数字文本解析，沿用 TOML 整数语法。

## 负向对照与恢复

1. 临时将 Implement 判断恢复为 gates_each_round，运行
   cargo test -p autome-domain -- gate_quick_implement：
   退出 101，两个实现轮进入 Gate 用例均失败（m01-negative-state.txt）。
2. 临时把完整 command 校验限制为无 concurrency 才执行，运行
   cargo test -p automed --lib -- config_io::tests::gate_quick_requires_full_command：
   退出 101，只有 concurrency + quick 被错误接受，测试失败（m01-negative-concurrency.txt）。
3. 另做缺失 command 未检查的变异，同一配置测试退出 101（m01-negative-config.txt）。
4. 所有变异均还原；重新执行两条完整 M-01 验收命令，分别 16/13 个通过。格式检查再次通过。

无环境阻塞。完整宿主执行与合并结果沿用保护尚待 M-02/M-03，不将本轮 domain 通过视为端到端功能已交付。
