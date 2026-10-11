# Loop 协议

这里是 Loop 的规则本身：六个角色共同遵守的协议、每个角色的 prompt 模板、每轮简报
摘哪些小节的映射，以及说明「某一版协议应该让会话做出什么行为」的 eval 用例。

协议只有一个来源：autome-v2 仓库的 `crates/automed/src/protocol/seed/`，编译进
`automed`，随 Autome 发布。本机没有别的副本，也没有可以单独发版、固定或回滚的协议
仓库（ADR-0003）。

## 文件

| 文件 | 是什么 |
|---|---|
| `loop-protocol.md` | 六个角色共同遵守的规则：原则、角色、文件、设计循环、里程碑、实现与审计循环、Backlog、内核检查。 |
| `session-protocol.md` | 一次会话的边界：怎么开始、怎么结束、硬性边界、状态块长什么样、什么才算协议失败。 |
| `prompts/*.md` | 每个角色一份 prompt 模板。Autome 填 `{doc_dir}`、`{budget_line}` 这些占位符，填不完的会话不启动。 |
| `brief-map.toml` | 角色 → 协议小节的映射，决定每轮简报里摘哪几段。 |
| `evals/` | 行为用例。`behavioral` 类改动必须带一个，且要「改前红、改后绿」。 |
| `CHANGELOG.md` | 每个版本改了什么、依据是哪些任务、预测哪个指标会怎么动。 |
| `contract.toml` | 内核契约区清单，由 Autome 生成。**只是给人看的**，期望哈希存在二进制里。 |

## 版本

当前版本是 `CHANGELOG.md` 里编号最大的 `## protocol/vN` 一节。任务在第一个会话之前
把整份协议复制到 `docs/<slug>/protocol/`，并记下 `protocol_ref`（版本号 + 内容哈希）。
会话读的是任务目录里那份副本——所以装了新版本的 Autome，正在跑的任务的规则也不会变；
而两个构建把不同的内容叫成同一个版本号，会由哈希暴露出来，不会悄悄地发生。

协议页按 `protocol_ref` 分组列出各版本下任务的指标，以及每个版本的改动条目。

## 怎么改

协议的改动就是 Autome 的代码改动，走同一套评审：

1. 改这个目录下的文件，在 `CHANGELOG.md` 末尾加一节新的 `## protocol/vN`，每条改动
   一个块，写清 `kind`、依据、预测。
2. `cargo run -p automed -- protocol eval crates/automed/src/protocol/seed`——静态门，
   不通过不能发。`--behaviour` 跑行为用例，要花钱。
3. 改了 prompt 模板的，跑 `cargo test -p automed --test prompt_rendering`，确认 golden
   的差异就是你想要的，再用 `AUTOME_UPDATE_GOLDEN=1` 接受。
4. `sh scripts/ci-local.sh`，打包，部署。

有几样东西改这里也改不了，eval 第一层会直接拒绝：

- 被 `<!-- kernel-contract: … -->` 包住的区域。内核解析的就是那些文本，要改得连解析器
  一起改。
- 状态机、转换表、预算、守卫代码。那不是协议，那是 Autome 本身。
- 项目的 `.autome/rules/`。项目规则走另一条路（面板上的「建议入规」卡片）。
