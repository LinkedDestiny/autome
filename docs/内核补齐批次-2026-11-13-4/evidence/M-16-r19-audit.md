# 审计 #18 · M-16 集成复验 · 提交 84592cb

结论：**通过**（里程碑表不动，M-16 保持已完成，reopen 0）。

## 逐项复验
1. 改动范围：6 个文件都是为门禁失败的测试契约对齐内核新增（gate_quick_concurrency、projects、gate_queue_lane、config.get 新字段、工作区 onboarding 跳过不报错、Bug 文档入清单），无产品代码改动；rebase 丢失提交 0 个。
2. 断言是否削弱：
   - task.rs：子串 `!contains("gate_quick")` 改为精确 JSON 键缺失 + `gate_quick_concurrency == 1`，反序列化与往返断言保留，更精确而非更弱。
   - settings-config：由「键不存在」改为「完整回包含键」，并保留 `configRows({gate:null})` 不伪造键的负向对照。
   - settings-onboarding-real：由期待 not a git repository 改为无错误、`init_commit: null`、HEAD 不变，符合设计 H-E1（一个文件都没有不提交也不报错）。
   - task-detail-real：由「清单无 Bug 文档」改为存在且可读出内容，符合设计 F-V1。
   - fixtures：只补键，未删键。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' 84592cb -- . ':(exclude)docs/内核补齐批次-2026-11-13-4/'` 输出为空。
4. 验收契约 `cargo test -p automed -- protocol_integrity_ the_version_in_force`：库 2 + e2e 6 = 8 通过。

## 判别检查与负向对照
- 负向对照：把 `ResolvedConfig.gate_quick` 的 `skip_serializing_if` 去掉，`gate_quick_resolved_json_is_unchanged_without_quick` 红（`value.get("gate_quick").is_none()` 失败）；已 `git checkout` 还原，工作区干净。
- 相关子集：`cargo test -p autome-domain gate_quick`（6 通过）；Web 6 个文件 settings-config / settings-onboarding-real / task-detail-real / notifier-backfill-real / project-confirm-real / settings-real：28 通过、0 失败。本沙箱 ps 可用，r19 实现轮的 ps EPERM 未再出现，清理用例通过。
- 全量门禁由宿主合并前运行，本轮未跑。H-01 人工验收仍由用户确认。
