# M-01 实现轮 #1 证据

范围：R-1、A-5、方案 §1；只推进 M-01。无上一轮审计，无需搬入审计回归检查。

## 实现与修复说明

- domain 增加默认 120、最小 5、稀疏项目覆盖、旧 JSON 缺字段的默认兼容及结构化违规。
- config_io 全局／项目读写四处带上字段；项目清除覆盖后继承全局。
- config.set_loop 全局／项目均接受字段，写前拒绝小于 5；中文错误说明与 README 样例已补。
- 沿用相邻 budget_factor 的数值解析约定；没有新增运行时超时行为（由 M-02 推进）。
- 首次 automed 测试编译发现新增测试位于 needs_git 宏定义之前，改为 assert!(git_available())，避免缺 git 时静默跳过；随后编译通过。格式化初次使用 edition 2021 不适用于已有 let chains，改用仓库 edition 2024 后成功。

## 验收命令与结果

`cargo test -p autome-domain session_minutes && cargo test -p automed --lib session_minutes && grep -c 'session_minutes' README.md`

最终结果：domain 3 个、automed lib 5 个，合计 **8 个用例，全部通过，失败 0、忽略 0**；README 计数 1。相对设计明确的上一轮基线 0，增加 **8**：domain 默认／覆盖、下界拒绝、有效边界 3 个，config_io 读取／全局保存／项目保存 3 个，dispatch 校验／写入读回与拒绝 2 个。同一测试内的边界输入及重复运行不重复计数；domain 其他测试二进制过滤后为 0，不算新增或跳过用例。

原始输出：`.autome/output/m01-acceptance.txt`。只对本轮三个 Rust 文件运行 `rustfmt --edition 2024`；未运行宿主快检或全量门禁。

## 标待审之前的自审清单

### 1. 情形表逐行

本里程碑无独立情形表，逐行对照里程碑说明①～⑧：

| 设计项 | 项目测试（均通过） | 判据 |
|---|---|---|
| ① | session_minutes_defaults_and_overrides | 默认 120，全局 600 被继承，项目 30 胜出；旧 JSON 缺字段恢复 120 |
| ② | session_minutes_rejects_below_minimum | 0、1、4 精确返回 SessionMinutesOutOfRange，min=5 |
| ③ | session_minutes_accepts_minimum_default_and_maximum | 5、120、u32::MAX 均无违规 |
| ④ | session_minutes_reads_global_and_project | 两级 TOML 读出 5、30、u32::MAX；空配置分别默认／无覆盖 |
| ⑤ | session_minutes_save_global_preserves_handwritten_value | 手写 30 加载保存不丢；同一配置另存新目录仍为 30 |
| ⑥ | session_minutes_save_project_preserves_and_removes_override | 手写 30 保存后保留，清除覆盖后读成 None |
| ⑦ | session_minutes_config_validate | 默认项目 ok=true；项目值 1 为 ok=false 且违规 kind 精确匹配 |
| ⑧ | session_minutes_set_loop_round_trip_and_rejection | 全局／项目写 30 后 config.get 读回；写 4 报错且文件字节不变；项目 null 清除覆盖 |

### 2. 失败分支

- 小于 5：domain 实跑 0／1／4，dispatch 实跑项目 config.validate=1、全局与项目 config.set_loop=4；均按设计拒绝。
- 空配置：全局默认 120、项目 None，已实跑；旧序列化数据缺字段已实跑。
- 授权拒绝、读盘失败、超时、外部不可达：不适用，M-01 未新增这些失败路径；超时处理属于 M-02。

### 3. 全部迁移

无会话状态机变更。配置两通道均实跑：全局默认→30→非法 4 拒绝并保持 30；项目继承→覆盖 30→非法 4 拒绝保持→null 清除恢复全局 30。config_io 另验证 Some(30)→None 的删除写回。domain 验证不同全局值 600 与项目值 30 的优先级，避免同值掩盖覆盖错误。

### 4. 输入域边界

u32 分钟域：0、1、4 拒绝；5 接受；120 默认；u32::MAX 接受并可从两级 TOML 读出。空配置、缺字段、项目 null 均已实跑。前导零、闰日、跨年、单元素不适用：本字段为整数分钟，非日期或集合，TOML 语法由现有解析器负责。

## 设计负向对照

以下为临时缺陷注入，均用项目正式测试检查，结束后完整还原生产文件，最后重跑验收为绿：

1. 把 `< SESSION_MINUTES_MIN` 改为 `== 0`：`cargo test -p autome-domain session_minutes_rejects` 失败，1 未产生违规。日志 `.autome/output/negative-min.txt`。
2. resolve 忽略项目值：`cargo test -p autome-domain session_minutes_defaults` 失败，得到 600 而非 30。日志 `.autome/output/negative-resolve.txt`。
3. 删除 save_global 的字段写入：`cargo test -p automed --lib session_minutes_save_global` 失败，新文件读回 120 而非 30。日志 `.autome/output/negative-save.txt`。

第三项补了新文件保存断言：toml_edit 会保留原文件未知字段，仅原地保存可能放过漏写实现。负向对照执行 3 个既有测试，不增加独立用例总数。

阻塞：无。全量门禁与宿主快检留给宿主；M-01 待独立审计，未标已完成。
