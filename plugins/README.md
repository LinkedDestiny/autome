# 插件

本目录是一个插件市场 `autome`，目前只有一个插件：[`loop`](loop/)。它同时为三个宿主准备了 manifest：

| 宿主 | 市场文件 | 插件 manifest |
|---|---|---|
| Claude Code | `.claude-plugin/marketplace.json` | `loop/.claude-plugin/plugin.json` |
| Codex（ChatGPT 账号登录的 CLI / 桌面端） | `.agents/plugins/marketplace.json` | `loop/.codex-plugin/plugin.json` |
| Cursor（IDE 与 `agent` CLI） | `.cursor-plugin/marketplace.json` | `loop/.cursor-plugin/plugin.json` |

插件的全部行为写在 `loop/skills/` 里的五个 skill，三个宿主读的是同一份。

## 安装

以下命令中的路径换成本机仓库里 `plugins/` 的绝对路径。

**Claude Code**

```bash
claude plugin marketplace add /path/to/autome-v2/plugins
claude plugin install loop@autome
```

**Codex**

```bash
codex plugin marketplace add /path/to/autome-v2/plugins
codex plugin add loop@autome
```

Codex 的 IDE 扩展不支持插件，只有 CLI 与桌面端支持。Codex 安装时会把插件复制到 `~/.codex/plugins/cache/`，改了源码要重新 `codex plugin add`。

**Cursor**

```bash
mkdir -p ~/.cursor/plugins/local
cp -R /path/to/autome-v2/plugins/loop ~/.cursor/plugins/local/loop
```

然后在 Cursor 里执行 Developer: Reload Window。`agent` CLI 也可以不安装、直接加载：`agent --plugin-dir /path/to/autome-v2/plugins/loop`。Cursor 只加载指向该目录内部的符号链接，所以这里用复制而不是链接；改了源码要重新复制。

## 验证

```bash
claude plugin validate plugins            # 市场文件
claude plugin validate plugins/loop       # 插件 manifest
claude --plugin-dir plugins/loop plugin details loop   # 组件清单：5 个 skill + 5 个短别名
```
