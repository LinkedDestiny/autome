# M-04 实现轮 #4

结论：验收通过，申请待审。本轮只推进 M-04；implementation-round 为 4/35。

## 实现与修复

- VALUED 新增 --brief / --kind；task new 支持文件与真实 stdin、UTF-8 错误、可选需求及草稿；未传新选项时原有参数保持不变。
- 只有 brief 时省略 request，让内核派生标题；初次集成验证发现发送空 request 会报「任务卡正文缺少标题」，修复后 stdin 与文件用例通过。
- task start 支持 id / slug；reply 原样传 feedback 到 task.reject；增加待开始状态、下一步与 task.started 事件显示、帮助文字。
- 新增 4 个单元测试和 4 个真实二进制集成测试。既有 cli.rs 是当前文件的逐字节前缀（cmp 验证通过），没有改写原测试。
- 上轮审计 M-03 通过，没有退回实现缺陷，因此无必须逐字移植的审计失败检查；上轮补强的调度器测试保留。

## 命令与结果

所有测试通过现有 Sandbox / 内存数据库隔离，不打开真实 ~/.autome。原始输出仅在 .autome/output/。

| 命令 | 结果与基线差 | 原始输出 |
|---|---|---|
| cargo test -p autome-cli task_card | 8 个通过（单元 4、集成 4）；上一轮同过滤范围 0，增加 8，全部来自本轮新增测试 | m04-test.txt |
| cargo test -p autome-cli | 25 个通过（单元 17、集成 8），0 失败、0 忽略；上一轮 17（13+4），增加 8 | m04-cli-all.txt |
| cargo test -p autome-domain task_card | 7 个通过，较上一轮 7 增加 0 | m04-domain.txt |
| cargo test -p automed --lib dispatch::tests::task_card | 12 个通过，较上一轮 12 增加 0 | m04-rpc.txt |
| cargo clippy -p autome-cli --all-targets -- -D warnings | 退出 0 | m04-clippy.txt |
| cargo fmt --all -- --check | 退出 0 | m04-fmt-check.txt |

本轮执行的不同用例共 44（CLI 25 + domain 7 + RPC 12），同范围上一轮 36，增加 8；过滤命令重复执行的 8 个不再计数。上一轮 workspace 977，本轮未跑 workspace，不能把推算的 985 报成实跑；全量门禁属于 M-05。

## 标待审之前的自审清单

1. **情形表逐行**
   - §6 参数：--brief - 的 dash 被当作值、--kind bug 与 --draft 被正确解析，参数单测通过。
   - stdin / 文件：真实子进程 stdin 创建 Bug 草稿，JSON 类型和状态正确；start 使用 slug，run 到批准设计，卡片字节与 stdin 原文一致。文件无需求时取标题，默认 feature。
   - 新 flag 仅在提供时加入参数；旧 happy-path 集成测试通过；未给需求和 brief 的用法错误退出 2。
   - 人读：草稿创建提示、status 待开始及下一步命令测试通过；start JSON 保留内核响应。
   - reply：feature / bug 都在批准设计后进入下一轮设计，prompt 含双空格、换行及中文反馈原文；缺文本退出 2。
   - state_line / next_step / task.started / USAGE：4 个新增单测通过。
2. **失败分支**
   - 文件不存在：读取任务卡正文中文错误、退出 1。
   - 非 UTF-8：0xff 文件拒绝，中文错误、账本数量不变。
   - 空白正文、65537 字节：拒绝、账本数量不变；无需求无 brief、reply 缺文本：用法错误。
   - 非草稿 start：取消态拒绝且状态不变；Draft reply：拒绝且保持草稿。
   - 授权拒绝、外部服务不可达、超时：不适用，CLI 在本范围内调用本地派发器，没有新增授权或远端请求。标准输入读错误与读盘错误各自映射 Failed；读盘失败已实际检查。
3. **全部迁移**
   - CLI 实跑：创建→Draft；Draft→Start→Active→批准设计；Draft→Cancel→Cancelled；Cancelled→Start 拒绝；Draft→Reply 拒绝；feature / bug 的批准设计→Reply→Design→批准设计。
   - 领域 7 用例覆盖 Draft Start→Queued、Cancel→Cancelled、全部其他 Trigger 拒绝，以及全部非 Draft 状态 Start 拒绝。
   - RPC 12 用例复验开始后排队及并行槽位、取消归档、草稿不入看板等路径。没有新增状态机分支。
4. **输入域边界**
   - 文件 65536 字节接受、65537 字节拒绝；空白文本拒绝；非法 UTF-8 拒绝；中文和占位符正文逐字节保留；反馈多行和双空格保留。
   - 缺需求 / 缺反馈、默认 feature / 显式 bug、id / slug 两种标识均有实际检查。
   - 前导零、闰日、跨年：不适用，本里程碑没有数字或日期输入域。

## 负向对照

- 临时从 VALUED 移除 --brief，运行 cargo test -p autome-cli brief_and_kind_take_values_and_a_dash_is_a_value：1 个失败，确认参数测试能捕获漏解析。输出 m04-negative-brief.txt。
- 临时仅将 reply 映射改成 task.approve，运行 cargo test -p autome-cli task_card_reply_rejects_the_design_with_the_text_verbatim：1 个失败，确认真实反馈路径测试能区别错误映射。输出 m04-negative-reply.txt。
- 两处注入均已恢复；随后重新执行验收及 CLI 全套，8 / 25 个全部通过，clippy / fmt 通过。

## 提交范围与环境

会话开始时 HEAD 1465fce（initial）已把大量项目文件从跟踪树删除，磁盘内容仍在，git status 因而显示 crates/docs 等为未跟踪；这不是本轮删除或重建。仅暂存本轮涉及的四个 CLI 文件、本证据、设计文档和 retro，其他既有未跟踪文件及 .gitignore 改动不纳入提交。按用户要求尝试在当前分支提交，不切分支、不推送。
