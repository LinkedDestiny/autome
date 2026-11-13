# M-01 审计复验（实现轮 r1，审计 #1）

结论：通过。

## 验收命令
`cd apps/web && npx vitest run <9 个文件>`：9 个文件、120 个用例全过，与契约预期的总数一致（skipped 0）。本沙箱可监听，w9-http 3 条也过（实现轮受 EPERM 阻塞的 14 条已补验）。

## 负向对照（临时注入，已全部还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| paths.ts 去掉 realpath | 7 条失败：paths（最深祖先、符号链接逃逸、HOME 符号链接、边界）与 project-confirm（符号链接零调用、probe 守卫、probe→add 同路径） |
| admin.ts 不真正调 project.probe | 3 条失败，含 project-confirm 第 1 条（调用序列） |
| gateway.ts 不认 LocalOpError | 5 条失败，含 project-confirm 第 2 条、real 外部目录拒绝、w9-http confirm |
| write-gate 去掉 project.remove | 3 条失败：write-gate remove、w9-http remove、real 重复 confirm |

## 自造判别检查（通过即丢弃）
同前缀兄弟目录（HOME=/tmp/ah/home，/tmp/ah/home2/x 应拒）、`~/a/./b` 放行、相对路径拒、`\` 拒：全部符合，守卫不是字符串前缀误判。脚本 `.autome/output/sib.test.ts`。
