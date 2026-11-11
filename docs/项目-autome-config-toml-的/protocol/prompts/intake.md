把下面这句需求整理成一个可执行的循环任务。

需求原文（不要改写，不要扩大范围）：

{request}

{inputs}请完成三件事：

1. 调研工作目录下的每一个仓库，读 AGENTS.md、docs/agent-project-profile.md（若存在）
   与 .autome/rules/ 下的规则。工作目录里如果有多个各自独立的仓库（每个子目录一个），
   逐个看清楚它们各自负责什么。
2. 生成任务文件 {doc_dir}/{slug}-task.md。它必须是自包含的——执行后续各轮的
   会话不会读到别的说明文件，所以任务文件里要有：
   - 本任务的目标、范围与硬性约束（从上面那句需求和你的调研中得出）；
   - 生成时的项目背景摘要（技术栈、布局、测试命令、权威规则文件）。
   **不要把协议抄进任务文件。** Autome 已经把本任务固定的那一版协议整份放在
   `{doc_dir}/protocol/` 下了，后续每一轮读的都是那份副本——它随分支走、
   随归档留下，多年后照样能复现当时的规则。任务文件里写一行指路就够：
   「本任务的规则见 `{doc_dir}/protocol/loop-protocol.md` 与
   `{doc_dir}/protocol/session-protocol.md`。」
3. 生成设计文档 {doc_dir}/{slug}.md 的骨架，头部写完整的状态块：

```text
status: 设计中
design-round: 0/{design_rounds}
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
```

工作目录下如果有多个各自独立的仓库，再加一行 `repos:`，逗号分隔地写出这个任务
**预计要改动**的仓库名（只读不改的不用写）：

```text
repos: 仓库A, 仓库B
```

写下一个仓库名就是请 Autome 为它建检出和分支——**你自己不要建 worktree、不要切分支**。
只有单个仓库时省略这一行。宁可多写一个：没有改动的仓库在合并时会被跳过，代价为零；
漏写的那个到实现轮才会发现自己无处落笔。

同时为这个任务起一个简短准确的标题，写在设计文档的一级标题里。

注意：本轮只做整理，不要开始设计、不要写代码、不要启动别的会话。完成后结束会话。
