# M-12 审计复验（实现 #20）· 通过

- 验收命令：3 文件 56 用例通过、0 失败；loop-page 新增 7 个用例。
- 负向对照（均已还原，git status 干净）：
  1. 删除 Link 的 onClick 确认 → 7 个新用例失败。
  2. dirty 恒真 → 无改动、改回原值、保存成功失败。
  3. dirty 恒假 → 取消、确认、保存失败及字段变更用例失败。
  4. dirty 改为 `!!drafts[editing]` → 仅「改回原值」失败（区分真实差异与草稿存在）。
  5. confirm 判断取反 → 4 个用例失败。
- 源码核对：保存成功清草稿、失败保留；project 查询参数 encodeURIComponent 保留；取消 preventDefault 不改草稿。
- 无人工项需移动。
