# M-10 审计 #16 独立复验

- 命令：`cd apps/web && /usr/bin/time -p npm run e2e`；日志 `.autome/output/a16-e2e.log`。
- 结果：20 条里 12 通过、1 失败（D-2）、7 未运行；`real 170.64`，被 `globalTimeout` 170 秒截断。预检通过。
- D-1 通过（52.3 秒，`loads` 为 1）；layout 11 条通过。
- D-2 失败原因：运行中的定位会话条目渲染了，但没有 `data-node`，选择器 `[data-node=locate]` 找不到；设计 testid 表要求该属性。
- 未运行：D-3、D-4 与 manual-items，故「命令行批准 ≤5000 ms」「重启后重放」均无证据。
- 负向对照未执行（套件不绿，前提不成立）。
- 结论：实现缺陷，退回开放，reopen 1，领域 e2e-flow。
