# project add 的 init commit 把 docs 下所有未跟踪文件一起提交

## 现象
`autome project add <仓库> --skip-onboarding` 生成的 `chore(autome): initialise project` 提交，把仓库 `docs/` 下用户自己未跟踪、不想提交的文件（如 `docs/research/`、草稿计划）也提交进了默认分支。

## 复现步骤
1. 建一个 git 仓库，提交一个 README，再新建未跟踪文件 `docs/draft.md`
2. 运行 `autome project add <仓库> --skip-onboarding`
3. `git show --stat HEAD`

## 期望行为
init commit 只包含 Autome 自己生成的文件（`.autome/` 脚手架、`.gitignore`、`AGENTS.md`、Autome 写的 docs 文件），用户原有的未跟踪文件保持未跟踪。

## 实际行为
`docs/draft.md` 出现在 init commit 里。2026-11-10 在 autome-v2 上实际发生过：`docs/plans/2026-10-11-…md` 与 `docs/research/` 被卷进去，只能手工重做提交。

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/automed/src/dispatch.rs` 的 `init_commit` 把整个 `docs` 目录交给 `git::commit_paths`；应只列出 Autome 生成的具体路径。
