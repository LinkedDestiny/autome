# M-04 · 实现 #4

## 范围与修复说明

仅推进 M-04：完成插件 README 五个固定小节和市场 README 双插件介绍、三宿主安装与验证命令；新增 tests/readme.sh，由 run.sh readme/all 复用；verify-cli.sh 增加 readme 组 V7–V9。loop 文件未修改。上一轮审计 M-03 通过，没有退回缺陷，因此本轮不适用审计缺陷检查逐字搬入及红绿复现。

JSON 示例来自本轮真实 autome 0.1.0 隔离账本，完整保留键及固定值，仅替换 id、路径、时间。V7/V8 比较完整结构和值，规范化非 null 的 id/path/doc_dir/时间；null 值、状态、标题、类型、分支等仍比较。临时仓库显式初始化为 master，避免机器 Git 默认分支配置影响示例比较。M-06 的独立负向回归仍留待该轮交付，没有提前推进。

## 验收命令与数量

- `sh plugins/autome/tests/run.sh readme`：5/5，R1–R5。
- `sh plugins/autome/tests/verify-cli.sh readme`：3/3，V7–V9。真实 autome 0.1.0，隔离 AUTOME_HOME 并 unset AUTOMED_DB_PATH，临时项目退出后清理。
- `sh plugins/autome/tests/run.sh all`：54/54，比上一轮 49 增加 5（R1–R5），没有删除或跳过。
- `sh plugins/autome/tests/verify-cli.sh skill`：6/6，与上一轮差 0。
- `sh plugins/autome/tests/hosts.sh`：5/5，与上一轮差 0。Claude 校验与 Codex 隔离安装实跑；Cursor 仅结构检查，真机仍属人工验收。
- 插件项目检查合计 68/68，比上一轮同口径 60 增加 8（README 静态检查 5、真实 CLI 对照 3），无失败或跳过。
- `node .autome/output/impl4/review.cjs`：3/3 临时负向对照（副本 task_id、pending、删除 Codex 安装行分别使 V7、V7、R5 变红）。正常源文件验收全绿；不混入项目用例总数。
- `sh .autome/output/impl4/errors.sh`：5/5 临时真实 CLI 错误检查，不混入项目用例总数。
- `sh -n plugins/autome/tests/run.sh plugins/autome/tests/readme.sh plugins/autome/tests/verify-cli.sh` 与 `git diff --check` 通过。
- 原始输出在 `.autome/output/impl4/`：readme.txt、cli-readme.txt、all.txt、cli-skill.txt、hosts.txt、review.txt、errors.txt；采样 task.json、projects.json、plain.txt；负向副本及日志同目录。没有转存到任务文档目录。

## 标待审之前的自审清单

### 1. 情形表逐行

- README 五个固定小节：R1 检查完整标题顺序。
- 程序化创建、开始命令与列项目：R2；人工逐行核对正文第一行、UTF-8、65536 字节、feature/bug、默认 draft、明确直接开始才去掉 draft、显式项目 id/名称/根路径以及省略 -p 的语义。
- task new JSON：R3 与 V7 对真实输出，完整结构及固定值一致；字段说明覆盖 id、kind、title、branch、doc_dir、state、started、queue_position。
- project ls JSON：V8 对创建前真实列表，包含 removed_at 与项目路径，README 明确过滤已移除项目。
- 非 JSON 两行：V9 完整逐字比较，同一账本第二张同名卡的 T-2 与 slug 测试-2 均一致。
- 错误说明：R4 和 errors.sh 五项真实检查，四条典型错误原文匹配，执行失败 1、用法错误 2。
- 状态隔离：逐行核对 AUTOME_HOME、AUTOMED_DB_PATH 优先及 unset 说明；实际验证脚本均隔离账本，无真实账本创建或开始任务。
- 三宿主安装、双插件与验证：R5 检查新增命令及 loop 原行，hosts 5 项通过；市场文档说明两插件共用技能模式，Cursor 复制目录与原 loop 内容保留。

### 2. 失败分支

- 文档点名空正文、65546 字节超长、当前目录无注册项目、开始非 draft 任务：errors.sh 四项均得到文档原文和退出码 1；未知参数得到前缀 autome：及退出码 2。
- 文档示例字段误写 task_id、状态误写 pending：副本 V7 均失败；Codex 安装行缺失：副本 R5 失败。原文件不注入缺陷。
- 拒绝授权、读盘失败、超时、外部服务不可达：本轮只增加文档与验收，不新增这类执行分支，因此本里程碑不适用；既有脚本 run.sh all 的 panel 不可达/超时路径已复验通过。真实宿主授权交互仍未实跑。

### 3. 全部迁移

本轮无新增状态机；逐行核对文档引用的四种已有回显情形：draft→用户可手动开始；queued→自动调度、不调用 task start；started true→说明 Loop 推进方式；其它→原样转达。V7 实跑 draft，既有 V3 实跑 started true，T4 复验排队与其它分支文字。创建前确认、修改后重确认、失败转达、成功无 id 先核对不重试均已对照技能。未将文档检查表述为真实 Agent 多轮对话验收。

### 4. 输入域边界

README 标明非空 UTF-8、65536 字节上限；本轮复跑 K9/K10、V5/V6：65536 接受、65537 拒绝；空正文 errors.sh 与 K15 拒绝。K11/V2 确认正文含 shell 定界符及命令文字仅作为数据；README 使用文件重定向。单项目列表由 V8 实测、同标题第二张卡由 V9 实测。日期/闰日/跨年/前导零不适用，本轮不新增对应输入域。

## 全仓门禁与交付

`sh scripts/ci-local.sh` 退出 1，用时 215 秒。Rust 1153 次执行通过；Desktop 104 项（103 通过、1 跳过）；Web 225 项（198 通过、11 失败、16 跳过）。合计 1482 次执行，与上一轮基线 1482 差 0；1454 通过、11 失败、17 跳过均与基线一致。失败为 Web 测试监听 127.0.0.1 被沙箱拒绝（listen EPERM），需允许监听的环境复验，未改测试以绕过限制。其它门禁步骤通过。日志 `.autome/output/impl4/gate.txt`。未跑付费 eval 或 Cursor 真机、多轮 Agent 人工验收。

四项自审在标待审前完成。implementation-round 写 4/40，M-04 仅标待审，其它里程碑保持原状态；repos 原本不存在，不增删。下一实现轮推进 M-05 eval 用例；本轮结束后由 Autome 调度审计，不启动下一会话。
