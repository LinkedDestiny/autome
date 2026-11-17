# M-05 r5 审计 #5 · 通过

- 验收命令：8 个文件 89 个用例全过（契约 ≥84）；task-summary 20、task-summary-real 4、task-timeline 28、task-stage 14、task-detail-data 12，均不低于契约。
- 负向对照（脚本 .autome/output/neg5.sh，每项后 git checkout 还原，status 干净）：
  - 「你批准」不取等待区间（`if (false)`）：task-summary 2 例 + task-summary-real 1 例失败。
  - 门禁计数含 `gate.reused` 与 `repro_check`：task-summary「门禁只数指定节点 result」失败。
  - 等待节点漏 `not_a_bug`：task-summary 3 例失败。
  - loader 返回全量 sessions：task-detail-data「汇总使用全量会话但仅传输窗口」失败；注：task-detail-ssr 未因此失败（体积判据由 data 用例承担），契约措辞中「60 会话 ssr 体积失败」不准确，不影响判别力。
- 无实现缺陷、无验证缺口。
