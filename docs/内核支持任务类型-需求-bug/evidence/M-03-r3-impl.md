# M-03 · 实现轮 #3

结论：验收通过，申请待审。implementation-round：3/35。本轮仅推进 M-03；M-02 上轮审计通过，没有需逐字迁入并红绿复验的缺陷检查。

## 实现与修复

- `write_task_inputs` 从账本接收正文，按 `TaskRecord::doc_dir()` 写卡片，与附件使用同一个 inputs 提交；调用先于固定协议与会话启动。附件仍仅首次复制，卡片缺失时后续启动可补写；已有文件不覆盖。
- `PromptSpec.card_path` 只由有正文的任务整理轮传入，路径使用会话所见文档目录。卡片说明前置；有卡片时按模板 `{request}` 切段、检查后拼入原始 request；无卡片沿用原渲染。协议文件、旧 intake 金样不变。
- 新增 5 个 lib、1 个金样、3 个真实 Git/包装脚本 e2e 用例。排队用例通过同时间戳任务夹具与 release 文件同步，验证并行 1 时已有排队任务先于后开始的草稿。
- 首次编译发现临时字符串借用与 Git 输出 String 类型使用错误，已修正。首次排队 e2e 用 `exit 1` 释放槽位，但默认重试继续占槽，导致 90 秒超时（2 通过、1 失败）；夹具配置 `[retry] attempts = 0` 后三个 e2e 通过，全量再次通过。不是产品状态机改动。原始日志：`.autome/output/m03-lib.txt`（后续成功运行已覆盖编译输出）、`m03-e2e.txt`（保留首次超时）、`m03-e2e-final.txt`。
- 失败提交后已有卡片的重试提交归 M-07；本轮未提前推进该里程碑。

## 验收命令与计数

1. `cargo test -p automed --lib -- launcher::tests::task_card scheduler::tests::task_card`：5 passed，0 failed，0 ignored；较上轮该筛选基线 0 增加 5。
2. `cargo test -p automed --test prompt_rendering`：11 passed，0 failed，0 ignored；较上轮 10 增加 1，旧金样比较原样通过。
3. `cargo test -p automed --test end_to_end task_card`：3 passed，0 failed，0 ignored；较上轮筛选基线 0 增加 3。

上述三条按验收契约串联执行，退出 0；合计 19 个，较上轮相同范围 10 个增加 9 个。日志分别为 `.autome/output/m03-acceptance-lib.txt`、`m03-acceptance-prompts.txt`、`m03-acceptance-e2e.txt`。

- `cargo test --workspace`：977 passed、0 failed、0 ignored，退出 0。上一轮基线 968（944 + M-01 的 12 + M-02 的 12），本轮增加 9；分项 CLI 单元 13、CLI 集成 4、domain 306、automed lib 577、e2e 59、prompt 11、real_cli 7。原 56 个 e2e 原样通过。日志 `.autome/output/m03-workspace.txt`。
- `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check`：退出 0。日志 `.autome/output/m03-clippy.txt`、`m03-fmt-check.txt`。
- 对比上轮审计提交 `c57b71f`，程序检查旧 e2e 内容仍是当前文件前缀，prompt_rendering 去掉中性 `card_path: None` 后旧内容仍是前缀，旧 intake 金样完全不变：通过。

## 标 `待审` 之前的自审清单

### 1. 情形表逐行

设计 §4–§5 落法逐行对应：

- 无正文、无附件：scheduler 单测对不存在的 checkout 调用无输入分支，成功且无需 Git；旧 e2e 56 个通过。
- 只有正文：scheduler 单测用自定义 `plans` 根，`git show HEAD:plans/card/card-card.md` 与中文、占位符、CRLF 原文逐字节一致；提交名精确为 inputs。
- 正文及附件：同一单测验证 65536 字节卡片和附件均在 HEAD；后续调用删除源附件仍成功且已有卡片不被替换。
- 已有 checkout、缺失卡片：删除卡片后以 `first_time=false` 调用，卡片补写成功。
- 提交与会话先后：Bug e2e 首会话脚本执行 `git cat-file -e HEAD:<card>`；最终 Git 历史卡片只由 inputs 引入，inputs 早于固定协议。
- 卡片 prompt 优先级：单测、金样和 e2e 验证卡片路径在 request 前，旧金样精确为新金样后缀。
- 无卡片 prompt：还原 None 与原输出逐字节一致，旧金样不变。
- 工作区路径：launcher 单测使用 `docs-repo/.worktree/checkout-flow/plans/checkout-flow/checkout-flow-card.md` 原样显示；生产构造使用 `layout.doc_dir_seen_from`，没有硬编码 docs 根。
- K1 Bug：e2e 走需求 Loop 至 await_design_approval；task.get、project.get、dashboard.waiting 都返回 kind=bug。

### 2. 失败分支

- 卡片目录不可创建：单测用普通文件阻挡父目录，确认返回“无法创建任务卡目录”。
- 无卡片且 request 含 `{unknown}`：launcher 单测仍拒绝；有卡片的三组占位符 request 原样渲染成功。
- 未授权、外部服务不可达：不适用，本里程碑没有新增授权或网络调用。
- 空正文与超限拒绝：属 M-02 入账边界，本轮全量运行既有 `an_empty_brief_is_refused`、64KiB/多一字节等测试通过；写盘不重复校验。
- 读盘失败与超时：没有新增正文读盘/超时分支（正文读自账本）；实际卡片写盘错误传播到既有启动失败处理。失败写入后的 Git 重试专项按已定稿 M-07 验证，不宣称本轮完成。

### 3. 全部迁移

- Draft → 无事件保持 Draft：fill_slots 单测及 settle e2e，不占会话、不建 worktree、不出现在看板。
- Draft + Start → 有槽立即启动：Bug e2e started=true，首会话为任务整理，随后到设计批准。
- Draft + Start → 无槽排队 → 槽释放后启动：排队 e2e started=false、queue_position=2；同 created_at 夹具顺序为已有排队任务、草稿。
- Draft + Cancel → Cancelled/归档：e2e 检查没有检出、没有任务分支、没有会话。
- 其余 Draft 触发器及非 Draft 的 Start 拒绝沿用 M-01 状态机测试，本轮 workspace 全量通过。没有新增状态转换。

### 4. 输入域边界

- 无卡片 None、单卡片、卡片和单附件、自定义文档根、工作区相对路径均检查通过。
- 卡片中文、换行/CRLF、`{request}`/`{unknown}` 原样提交；最大 65536 UTF-8 字节写盘并读取 HEAD 比较通过。
- request 为 `修复 {request} 展示`、`改 {inputs} 与 {slug}`、`{unknown}` 三组均逐字保留。
- 空正文、空白、65537 字节由既有 dispatch 用例覆盖并通过；日期、前导零、数值最大值不适用，本里程碑没有这些输入域。

## 工作区与提交

开场 HEAD `c1af8f5 initial` 已将大量项目文件从索引删除，工作区中这些文件仍在且为未跟踪；上轮审计提交为 `c57b71f`。仅暂存本轮修改的源码、测试、新金样、设计状态及证据/retro，不暂存其它已有未跟踪文件或 `.gitignore` 改动。未修改 `.autome/` 配置或规则；仅按协议将临时脚本与输出放入 `.autome/output/`。
