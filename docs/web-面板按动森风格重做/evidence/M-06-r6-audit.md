# M-06 · 审计 #6 · 独立复验

## 验收命令
- vitest 11 个文件 115 个用例全过（契约要求 ≥107）。
- playwright `-g "详情|W7 H-03|W7 H-04|C-2"`：6 条全过（需求详情展开日志、Bug 详情、W7 H-03、W7 H-04、C-2、C-2b）。本环境浏览器可正常运行，实现轮的 ps EPERM 阻塞未复现。

## 负向对照（均已还原，git status 干净）
1. 筛选链接加 `remove: ['from']`：task-timeline「筛选链接保留 from、doc…」失败。
2. 执行过程容器去掉 `data-scroll-region`：testids「scroll exemptions…」与 task-timeline 失败。
3. 操作区过滤掉 `rerun`（契约里「重新打开」在代码中对应「重跑」，不存在名为重新打开的按钮）：task-header「头部保留全部可用写操作…」失败。

## 结论
通过。无实现缺陷、无验证缺口。
