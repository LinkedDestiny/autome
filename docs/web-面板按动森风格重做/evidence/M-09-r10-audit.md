# M-09 审计（实现轮 r10）· 通过

## 负向对照（均已还原，git status 干净）
- 去掉 set_role 的 `project_id`：skills-model 2 + skills-real 3 失败（5 红）。
- 打开器接受请求参数里的路径（同时放宽键白名单）：open-path「不接受浏览器路径」失败。仅改取路径一行而不放宽白名单不红——白名单本身已 400 拦截，产品安全。
- 用 `project_scoped` 禁用全局一档：skills-model 2 失败（同名混合用例在内）。
## 验收命令
- vitest 9 个文件 145 用例全过（契约下限 120）。
- playwright `layout.spec.ts controls.spec.ts -g "技能|F-1"`：2 条通过（本环境浏览器可运行，实现轮的 ps EPERM 阻断不再存在）。
## 人工项
H-07（真实 Finder 打开目录）已在设计文档人工验收清单中，用户合并前确认。
