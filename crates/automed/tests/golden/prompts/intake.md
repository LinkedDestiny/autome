把下面这句需求整理成一个可执行的循环任务。

需求原文（不要改写，不要扩大范围）：

{request}

请完成三件事：

1. 调研本仓库，读 AGENTS.md、docs/agent-project-profile.md（若存在）
   与 .autome/rules/ 下的规则。
2. 生成任务文件 docs/{slug}/{slug}-task.md。它必须是自包含的——执行后续各轮的
   会话不会读到别的说明文件，所以任务文件里要有：
   - 本任务的目标、范围与硬性约束（从上面那句需求和你的调研中得出）；
   - 生成时的项目背景摘要（技术栈、布局、测试命令、权威规则文件）；
   - **`.autome/skill/loop-protocol.md` 的全文，逐字复制**，不要只写路径、
     不要概括、不要改写。任务目录归档多年后仍要能凭它复现当时的规则。
   另见 .autome/skill/session-protocol.md 的会话边界，同样逐条遵守。
3. 生成设计文档 docs/{slug}/{slug}.md 的骨架，头部写完整的状态块：

```text
status: 设计中
design-round: 0/15
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
```

同时为这个任务起一个简短准确的标题，写在设计文档的一级标题里。

注意：本轮只做整理，不要开始设计、不要写代码、不要启动别的会话。完成后结束会话。
