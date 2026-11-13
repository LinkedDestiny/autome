# M-05 审计 #5 独立复验

结论：通过。

## 验收命令
`cargo build -p automed -p autome-cli && npm run test --prefix apps/web -- <8 个文件>`：退出码 0，8 文件 51 用例通过 0 失败（与预期 51 一致；30 新/改 + 21 既有）。本环境允许本地监听，实现轮遇到的 EPERM 未出现，HTTP/WebSocket 两例（older-kernel-real 第 1、3 号）真实通过。

## 负向对照（每次注入后还原，git status 干净）
| 注入 | 结果 |
|---|---|
| 横幅改回「内核离线：…」（app/lib/live.tsx） | older-kernel-live「横幅逐字原文」失败 |
| isSlow 恒为 false | older-kernel-core 第 3 号及「恢复与再次拒绝只在变化时记录」失败 |
| 有 PATH 时直接返回 cargo 档 | core-binary「PATH 第一个 autome 优先于缺省安装」失败 |
| 缺省安装改回查 ~/.local/bin/automed | core-binary「本地安装先于 cargo」「不再查旧 local-bin automed」失败 |
| activeSidecar 不放行 outdated | older-kernel-real 第 1、3 号失败 |

首次的 cargo 前置变体（仅当 target 文件存在才返回）未被拦住，因测试用假 exists；它不是契约描述的错误实现，换成无条件前置后即被拦。属自造变体偏差，非验收缺口。

未发现产品缺陷。人工验收项：无。
