# M-11 r14 审计证据

结论：实现缺陷，截图未替换（G-Q3 约定路径）。

1. 验收命令 vitest 段：readme-images 6 例中 3 例失败（docs/images/loop.png 不存在），docs-web 通过。这同时是「缺图」负向对照：缺两张图时验收确实失败。
2. 截图脚本在本审计环境可运行（退出 0，残留进程 0）。产物五张均 1512×944：dashboard 143635B、loop 153950B、project 168925B、skills 77856B、task 169510B 字节。查看 loop.png：全局范围、设计循环与实现循环角色卡片，渲染正常。
3. playwright layout.spec.ts：14 条全过，含「Loop 配置」「技能」。「去掉 data-scroll-region」负向对照未单独重跑：该机制由前几轮已通过里程碑的同一用例函数守住，本里程碑未改 layout 用例。
4. 审计轮不得改 docs/images（越界），故退回；下一轮复制产物后验收应全绿。
5. 提交由 Autome 兜底（如被拒）。
