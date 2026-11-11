# Autome

把对话里讨论好的需求或 Bug 整理成任务卡，确认后通过 autome 命令行创建。

## 技能

`autome-task` 是 Claude Code、Codex、Cursor 共用的技能；Claude Code 另有短别名 `/autome:task`。
技能与正文模板见 [SKILL.md](skills/autome-task/SKILL.md)。

## 安装

按[插件市场安装说明](../README.md)安装 `autome@autome`（Cursor 复制插件目录）。
命令行也必须在 PATH 上：仓库检出内运行 `sh scripts/install-cli.sh`；其它机器运行：

```sh
curl -fsSL https://raw.githubusercontent.com/LinkedDestiny/autome/main/scripts/install-cli.sh | sh
```

默认安装到 `~/.local/bin`。技能找不到命令行时只给安装说明并停止。

## 在对话里用

讨论完说「建成 autome 任务」或「把这个 Bug 提到 autome」。信息不全会先追问，不编造验收标准。
技能按当前物理目录推断项目，无法推断时列出项目供选择。
创建前完整展示正文、类型、项目、大小与创建模式，等待明确确认；修改后重新展示并确认。
默认存为待开始；明确说「直接开始」才去掉 `--draft`。创建后回显任务 id；只有探测到运行中的面板才附链接。

## Agent 程序化创建

先准备 UTF-8 Markdown 文件 `card.md`：第一行 `# <标题>`，全文非空且不超过 65536 字节。
按技能中的需求或 Bug 模板填写，先向用户展示完整正文、类型、项目和创建模式并取得明确确认。
正文用文件写入能力保存，不把正文拼进 shell 命令或 heredoc；下面仅通过标准输入传输文件内容。

```sh
autome project ls --json
autome task new --brief - --kind feature --draft -p prj-example --json < card.md
```

把 `feature` 换为 `bug` 可创建 Bug；`-p` 换成已注册的项目 id、名称或项目根路径，子目录不是项目根路径。
不写 `-p` 时取包含当前目录的最深未移除项目；建议确认项目后显式传 id，避免执行目录变化。
只有明确选择直接开始才去掉 `--draft`。创建后不自动调用开始命令；待开始任务可由用户运行 `autome task start <id>`。

以下是隔离账本中 `autome 0.1.0` 的真实输出，示例仅替换 id、路径和时间，未增删键。
正文标题为「测试」，新账本第一次创建：

<!-- task-new-json -->
```json
{
  "queue_position": null,
  "started": false,
  "task": {
    "archived": false,
    "branch": "autome/测试",
    "budget_n": null,
    "completed_at": null,
    "created_at": "2026-11-11T00:00:00Z",
    "doc_dir": "docs/测试",
    "id": "T-1",
    "kind": "feature",
    "merge_commit": null,
    "metrics": null,
    "protocol_ref": null,
    "request": "测试",
    "slug": "测试",
    "state": {
      "state": "draft"
    },
    "title": "测试"
  }
}
```
<!-- /task-new-json -->

`task.id` 是后续命令使用的 id，`task.title` 是标题，`task.kind` 是类型；`task.branch` 为任务分支，`task.doc_dir` 为文档目录。
`task.state.state` 为 `draft` 表示待开始；`started` 仅在实际启动后为 `true`。
不带 `--draft` 但槽位占满时，状态为 `queued`、`started` 为 `false`，`queue_position` 给出从 1 开始的排队位置；调度器会自动开始，不应对排队任务执行 `task start`。
`started: true` 表示已开始，后续 Loop 由 `autome run` 或打开的桌面端推进；其它状态原样转达，不猜测。

创建之前的 `autome project ls --json` 示例（临时仓库默认分支为 `master`）：

<!-- project-ls-json -->
```json
{
  "projects": [
    {
      "counts": {},
      "project": {
        "added_at": "2026-11-11T00:00:00Z",
        "default_branch": "master",
        "display_name": "repo",
        "disposition": "adopted_existing",
        "docs_repo": null,
        "id": "prj-example",
        "kind": "repo",
        "members": [],
        "onboarding": {
          "onboarding": "skipped"
        },
        "parallel_limit": 3,
        "path": "/path/to/repo",
        "removed_at": null
      },
      "slots_in_use": 0
    }
  ]
}
```
<!-- /project-ls-json -->

项目在 `projects[].project` 中；`removed_at` 非 null 的项目已移除，不应选择。
不带 `--json` 时输出两行；这里接着在同一账本以相同标题创建第二张待开始卡：

<!-- task-new-text -->
```text
T-2  测试-2  待开始
已存为待开始。用 `autome task start T-2` 开始。
```
<!-- /task-new-text -->

执行失败退出码 1，用法错误退出码 2；标准错误是一行 `autome：<原因>`。典型原文：

```text
autome：任务卡正文不能为空
autome：任务卡正文超过 64KB（65546 字节，上限 65536 字节）
autome：当前目录不在任何已注册的项目里：用 -p 指定，或先 `autome project add <路径>`
autome：只有待开始的任务可以开始
```

出错时转达原文及退出码，修改后重新确认再创建；成功但读不出 `task.id` 时先用 `autome ls` 核对，不盲目重试。
状态目录为 `$AUTOME_HOME`（默认 `~/.autome`），`AUTOMED_DB_PATH` 优先于它。
测试时务必使用临时 `AUTOME_HOME` 并 `unset AUTOMED_DB_PATH`，只注册临时仓库，不触碰真实账本。

## 验证

```sh
sh plugins/autome/tests/run.sh all
sh plugins/autome/tests/verify-cli.sh readme
sh plugins/autome/tests/verify-cli.sh skill
sh plugins/autome/tests/hosts.sh
```

确定性测试不启动 Agent；真实宿主对话与付费 eval 不包含在上述测试中。
