# M-01 · r2 · 审计独立复验：通过

- 验收命令：vitest 4 文件 **4 passed / 40 passed（14+12+4+core-supervisor 基线 10），skipped 0**；`run.sh card panel` 29/29。含 web-instance 第 14 条与 web-launcher 第 10、11 条（真实服务），上轮 web-stop-gate 4 条失败已消失。
- 负向对照（改源码、跑对应文件、`git checkout` 还原，工作区差异为空）：
  1. 陈旧判定不核对 `instance` → web-instance 2 条失败（换主人、三态判定表）；
  2. `stop` 不等门禁 → web-stop-gate 2 条失败（拒绝停止、--force 门禁存活）；
  3. `stop` 不核对 `--instance` → web-launcher 绑定旧 uuid 一条失败。
  其余对照项由对应用例断言覆盖，未逐一注入。
- 结论：通过；无产品缺陷。
