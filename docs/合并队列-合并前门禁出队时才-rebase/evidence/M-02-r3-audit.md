# M-02 审计复验（实现轮 r3）· 通过

## 验收命令（自己跑）
`cargo test -p autome-domain merge_auth_ && cargo test -p automed --lib merge_auth_ && cargo test -p autome-cli merge_auth_ && cargo test -p automed --test end_to_end merge_auth_` → 退出码 0；domain 6、lib 6、cli 1、e2e 4，共 17，与契约一致。

## 负向对照（均已还原，`git status` 干净）
- A：`merge_press` 接受 `Failed` → `merge_auth_merge_press_by_state` 失败（`merge_press(&failed).is_err()`）。
- B：`cancel_press` 放行 `Merging` → domain `merge_auth_merge_press_by_state` 与 lib `merge_auth_cancel_is_rejected_once_the_merge_has_started` 失败。
- C：`Authorize` 分支对任务应用 `Approve` → e2e `merge_auth_preauthorizing_before_design_approval_does_not_skip_it` 失败。
- 契约里“保留 advance 自动清除”一项未单独注入：该行为已由 `merge_auth_leaving_the_merge_tail_writes_no_revocation` 与 e2e 保留授权用例直接断言，且实现里已无清除代码。
日志：`.autome/output/acc.log`（不入库）。

## 结论
验收命令能区分三种典型错误实现，当前实现通过。未发现产品缺陷；无验证缺口。快检由宿主报告通过。提交由 Autome 兜底（若提交失败）。
