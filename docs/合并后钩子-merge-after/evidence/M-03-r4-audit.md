# M-03 审计证据（实现轮 #4，审计 #3）

结论：通过。

## 验收命令（自己跑）
- end_to_end 111 通过（预期 111）；automed lib 657 通过（预期 657）；0 失败。
- 过滤 `merge_after`：e2e 8、lib 10，与预期一致。

## 负向对照（改坏实现 → 确认验收变红 → `git checkout` 还原，工作区干净）
脚本：`.autome/output/mut3.sh`、`.autome/output/mut3b.sh`
- 去掉主检出互斥 → L3、L5 失败。
- 互斥不看 `setup-main` → L3 失败。
- `begun` 不认 `prepared` 标记 / 开始事件 → L5 失败。
- MainCheck 失败判成通过 → E4 失败。
- 停止时不 `discard_node("after_merge")` → E8 失败。
- 工作区不跳过 → E6 失败。
- 钩子阶段整体禁用（理智检查）→ E1/E2/E4–E8 失败。

## 独立核对
读 `scheduler.rs::run_core_step` 与 `merge_after.rs`：准备段在前、`prepared` 标记先于钩子落盘、钩子 Pending 时不再重入准备；未配 `[merge] after` 时 `merge_after=None`，`begun=false`，原路径不变。无产品缺陷、无验证缺口。
