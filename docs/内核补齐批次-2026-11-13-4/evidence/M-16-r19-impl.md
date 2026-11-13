# 实现 #19：宿主合并门禁失败修复

## 范围与结论

本轮为用户指定的修复轮，不推进新里程碑。上轮审计 #17 通过，没有退回的实现缺陷；M-01—M-16 的状态和已关闭里程碑证据指针均保持原样。状态块更新为 `implementation-round: 19/80`，下一步独立复验本轮测试契约修复。没有运行全量门禁或宿主快检，没有修改产品代码、页面、门禁脚本或测试清理保护。

宿主基线 `2e0696bfda` 的原始日志为主检出 `.autome/output/gates/T-95/merge_gate-2e0696bfdacd.log`（只读）。Rust 在 domain 套件的一个断言失败后停止；Web 共 833 个，827 通过、6 失败。本轮先单独复现这些现有项目测试，再修复，未把门禁总览当作具体失败证据。

## 根因、修复及同类检查

1. **E / M-11**：`gate_quick_resolved_json_is_unchanged_without_quick` 以字符串子串寻找 `gate_quick`，误匹配合法新增字段 `gate_quick_concurrency`。改为精确 JSON 键缺失断言，并验证并发缺省值 1；原反序列化、往返一致及启用 quick 的断言全部保留。共享 ResolvedConfig 的稀疏序列化未改。
2. **E、G / M-11、M-14**：dashboard double 缺少顶层 `projects` 和任务行 `gate_queue_lane`。补齐两个键，真实回包与 double 的精确键集合断言保留。首个 backfill 用例以前提前退出，导致通知未初始化，第二个重启用例随之失败；修复夹具后两者都通过，没有修改通知逻辑。
3. **G / M-14，设计 §6 的 config.get 全字段约定**：用项目已有 `captureReplies()` 和独立临时账本捕获当前回包，只更新 w9 JSON 的新增配置字段、defaults 与 probe.rejection；不改历史随机 ID、时间和路径。补齐 `gate_quick`、`gate_quick_concurrency`、`gate_run`、`setup`、全局 `quick_concurrency`。`project-confirm-real` 的逐层精确 shape 对照和 `settings-real` 的值相等对照均保留。共享夹具使用者检查又发现 `settings-config` 仍断言这些键不存在；改为验证完整回包含键，同时用独立稀疏输入保留“不伪造省略字段”的负向对照。
4. **H / M-15，H-E1/H-E3**：旧 Web 测试期待工作区返回 `not a git repository`，与已修复契约冲突。改为无错误、状态 skipped；此夹具没有成员内产物，故明确验证 `init_commit: null` 且成员 HEAD 不变。存在产物的提交以及索引锁失败保持状态/重试由本轮内核子集覆盖。
5. **F / M-12，F-V1**：旧 Web 测试期待 Bug 文档不在清单。改为文档必须存在，再按清单名称读出真实内容并检查 `复现命令: sh regression.sh`，原 Bug 事件与任务隔离断言不变。

已检索同类 `gate_quick` 子串判断、工作区旧错误断言、Bug 清单否定断言，以及两份共享夹具的引用。`add-project.test.ts` 注入通用内核错误的展示测试并非断言真实工作区必然失败，予以保留。上轮没有审计发现缺陷的新检查需要搬迁；这次失败检查本身均已在项目测试体系中，原样运行记录如下。

## 命令与红绿证据

原始输出均存 `.autome/output/`，不复制到任务目录。所有真实内核测试使用既有夹具的临时 AUTOME_HOME，没有触碰真实账本。

| 命令 | 红 / 修复前 | 最终结果 | 日志 |
|---|---|---|---|
| `cargo test -p autome-domain gate_quick_` | 6 个：5 通过、1 失败；就是宿主的子串断言 | 6 通过 | `r19-rust-red.txt`、`r19-rust-green.txt` |
| `npm run test --prefix apps/web -- test/notifier-backfill-real.test.ts test/project-confirm-real.test.ts test/settings-onboarding-real.test.ts test/settings-real.test.ts test/task-detail-real.test.ts` | 21 个：14 通过、7 失败；包含宿主 6 个失败和本地新增的 ps 权限失败 | 21 个：20 通过、1 环境失败；afterAll 亦因同一权限错误失败，退出 1 | `r19-web-red.txt`、`r19-web-final.txt` |
| `npm run test --prefix apps/web -- test/notifier.test.ts test/notifier-real.test.ts test/project-confirm.test.ts test/settings-config.test.ts test/alerts.test.ts test/alerts-real.test.ts` | 更新共享夹具后 42 个：41 通过、1 失败，发现 settings-config 的旧省略字段断言 | 42 通过 | `r19-web-related.txt`、`r19-web-related-final.txt` |
| `cargo test -p automed --lib -- config_get_ workspace_onboarding_commit_ task_documents_ dashboard_tolerates_` | 相关契约复验，非新增缺陷复现 | 13 通过 | `r19-kernel-related.txt` |

中间一次复跑 `r19-web-green.txt` 仍失败，暴露嵌套 `gate_queue_lane` 遗漏和测试误以为无产物也应提交；已依据源码及设计修正，不把该日志计作通过证据。捕获回包在 `r19-replies.json`，捕获日志 `r19-capture.txt`。

**最终唯一用例总数 82（6 + 21 + 42 + 13），81 通过、1 环境失败、0 跳过；另有同原因 afterAll 错误，不算独立用例。** 相比实现 #18 的 96 个主验收用例为 **−14**，原因是本轮改跑门禁失败文件及直接引用夹具的子集，不再跑上一轮 kernel/note/write-gate/rebase 集成范围；不是删掉 14 个测试。与本轮修改前相同选择范围相比总数变化 **0**，没有新增、删除或跳过用例，红绿复跑不重复计数。

## 标待审之前的四项自审

本轮不改变里程碑状态，仍逐项执行清单：

1. **情形表逐行**：E 的无 quick/有 quick/运行模式组合由 6 个 gate_quick 用例覆盖；G 的完整配置/显式覆盖/defaults、坏配置容错由 config_get/dashboard_tolerates 与 Web 精确回包对照覆盖；H 单仓库与工作区、完成与跳过、有产物与无产物由 workspace_onboarding_commit 与 Web onboarding 覆盖；F 需求/Bug/默认分支/清理后的文档由 task_documents 与真实详情页用例覆盖。全部对应行为通过。
2. **失败分支**：索引锁提交失败不推进状态及重试成功、坏 TOML 不阻止页面/移除、重复注册拒绝、外部目录拒绝、超长保存不写盘均由本轮子集执行；settings-config 也逐项检查拒绝授权、读盘失败、请求超时、内核未连接的错误显示。进程清理检查因沙箱拒绝 ps 未通过，单列为阻塞，未吞掉错误。
3. **全部迁移**：本轮没有新增状态机；相关既有迁移仍检查 Onboarding in_progress→completed/skipped、失败保持原态→重试，通知首次回填→重启同状态不重复→驳回再进入时新增、旧面板库播种；全部通过。需求/Bug 的真实事件及合并前阶段断言通过，进程清理收尾待宿主复验。
4. **输入域边界**：精确 JSON 键与相同前缀键、null 与键缺失、空字符串/0/false、未知键与 toString、无产物 workspace HEAD 不变、超长内容 262145 拒绝均通过。闰日/跨年不适用：本轮没有日期输入改动。

## 环境限制与交接

本沙箱执行 `ps -axo` 返回 `Operation not permitted`，Node 对应 `spawnSync ps EPERM`。`task-detail-real` 的清理用例与 afterAll 保留原样，未以跳过、mock ps 或削弱断言制造全绿。真实业务断言已通过，无法在本环境证明无残留进程；夹具调用 core.stop 后仍保留其临时目录供宿主核查。

`git diff --check` 通过。全量 `sh scripts/ci-local.sh` 和宿主快检均未运行，结果以 Autome 后续执行为准。H-01 人工验收仍沿用原审计待办。本轮结束后不启动下一会话。
