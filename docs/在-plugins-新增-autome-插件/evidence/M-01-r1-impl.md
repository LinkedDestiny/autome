# M-01 实现证据 · 实现 #1

范围：插件骨架、三份 manifest、三份市场登记、共享技能占位、短别名、README 占位及结构/宿主检查。没有修改 loop 或实现 M-02–M-05。没有上一轮审计，故无审计缺陷回归需迁入。

## 验收与数量

- `sh plugins/autome/tests/run.sh structure && sh plugins/autome/tests/hosts.sh`：退出 0，10 + 5 = **15 个用例全部通过**。本插件上一实现轮基线为 0（此前尚无插件测试），新增 15，分别来自 S1–S10 和 H1–H5；无删除、无跳过。原始输出：`.autome/output/impl1/final-structure.txt`、`final-hosts.txt`；宿主细节在 `.autome/output/autome-hosts.*/`。
- `/bin/bash plugins/autome/tests/run.sh structure`：10/10；`sh -n` 与 `/bin/bash -n` 检查两个脚本通过，`git diff --check` 通过。
- `sh scripts/ci-local.sh`：退出 1，144 秒。Rust 各次调用合计 1153 次用例执行全部通过（含 CI 自身重复运行的 77 项及 Adapter 单项，非去重计数）；Desktop 104 项，103 通过、1 跳过；Web 225 项，198 通过、11 失败、16 跳过。总计 1482 次执行，1454 通过、11 失败、17 跳过。上一轮未提供全量运行基线，无法声称数量差为 0，本轮建立基线；未修改这些测试。Web 失败报告包含 `listen EPERM: operation not permitted 127.0.0.1`，沙箱禁止监听；Electron 二进制缺失，下载 `TypeError: fetch failed`，DOM 用例跳过。格式、Clippy、Contract、Web lint/typecheck/build 均通过；package 未运行。原始输出 `.autome/output/impl1/gate.txt`。不将此记录为全量绿色，需在允许监听且具备 Electron 的环境复验。
- plugin-creator 通用 `validate_plugin.py` 退出 1：要求额外 `interface.longDescription`、`defaultPrompt`、`capabilities`。本设计明确仅四字段，且真实 Codex 安装/列表/缓存检查全部通过；保留项目契约，不为通用模板校验器增加字段。输出 `.autome/output/impl1/plugin-validation.txt`。

## 标待审之前的自审清单

### 1. 情形表逐行

| 契约 | 检查与结果 |
|---|---|
| S1 | 三份 manifest 存在，顶层 name 均为 autome，通过 |
| S2 | 三宿主 version 均 0.1.0，通过 |
| S3 | 三宿主 description 逐字相同，通过 |
| S4 | Claude commands、Codex/Cursor skills 路径及目录，通过 |
| S5 | Codex interface 四字段非空，通过 |
| S6 | 三份 autome 市场条目路径正确，Codex policy/category 齐全，通过 |
| S7 | loop 条目保留；另用 JSON 解析与 HEAD 对比三份原条目，内容一致 |
| S8 | 技能 frontmatter 名称、描述、隐式调用约定，通过 |
| S9 | task 短别名只转发共享技能，无创建命令，通过 |
| S10 | 相对 merge-base HEAD main 的 loop 目录无修改、无新增文件，通过 |
| H1 | Claude 市场 validate 通过 |
| H2 | Claude 插件 strict validate 通过 |
| H3 | Claude details 含 Skills (2)、autome-task、task，通过 |
| H4 | 临时 CODEX_HOME 中安装、启用并列出 autome@autome 0.1.0，通过 |
| H5 | Codex 安装缓存含共享 SKILL.md，通过 |

Cursor 本机无 CLI，按设计只做结构验证；真机加载仍属 H-06 人工验收。共享技能正文明确当前仅骨架、停止创建，未宣称已有完整行为。

### 2. 失败分支

设计 M-01 点名的三个负向对照均实际执行，并在 finally 中恢复原文件：

- 删除 Cursor 市场 autome 条目：structure 退出 1，S6 失败（`missing-cursor.txt`）。
- Codex manifest version 改成 0.1.1：S2 失败；hosts 的 H4 失败，H5 因固定版本缓存不存在也失败（`wrong-version.txt`）。
- Claude manifest 去掉 author：strict 校验退出 1（`missing-author.txt`）。

上述输出都在 `.autome/output/impl1/`；恢复后最终验收 15/15。授权拒绝、正文读盘、外部服务、创建超时等业务失败分支不适用：本里程碑无创建运行逻辑，属于后续里程碑。

### 3. 全部状态迁移

业务状态机不适用：当前只有静态插件骨架。宿主通道逐项检查：Claude 校验→details 发现；Codex 空白 CODEX_HOME→市场登记→安装并启用→缓存技能存在，H1–H5 通过；Cursor 结构声明通过，真机加载留 H-06。没有创建、开始或取消真实任务，测试 CODEX_HOME 在结束后清理，仅保留日志。

### 4. 输入域边界

业务正文大小、日期、类型等输入域不适用，校验脚本尚未进入本轮范围。检查入口的未知组 `sh plugins/autome/tests/run.sh unknown` 实跑退出 2，未冒充成功；合法单组 structure 在 sh 和系统 bash 均为 10/10。版本不一致与缺失字段边界由上面三个负向对照覆盖。

## 修复与结论

初次 structure 为 9/10：S1 的字段提取误把嵌套 author.name 也读入；将 manifest 顶层提取限定为两空格缩进后 10/10（初次输出 `structure.txt` 已被后续检查覆盖，最终结果见 `final-structure.txt`）。这是本轮测试实现错误，没有上一轮审计回归。三个市场的原 loop 条目保持原文，测试不依赖 jq、不安装插件到用户真实 CODEX_HOME。

M-01 专项验收通过，可标待审；全量 CI 的环境限制保留给审计复核，不声称已完成。
