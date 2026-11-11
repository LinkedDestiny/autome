# M-01 审计轮 #1 独立复验

结论：通过。

## 验收命令（自己跑）
- autome-domain 355 通过（基线 345 + 10）；automed lib 611 通过（基线 606 + 5）；过滤 `setup` 得 10，过滤 `setup_section` 得 5；0 失败。
- 全量：`cargo fmt --check` 干净；`cargo clippy --all-targets -D warnings` 干净；`cargo test --workspace --all-targets` 全绿（cli 19+11、domain 355、automed lib 611、end_to_end 80、其余 16、7）。e2e 仍为基线 80，未改现有用例。
- `.autome/` 相对基线无改动。

## 负向对照（逐条注入错误实现，确认失败后 git checkout 还原，工作区干净）
脚本 `.autome/output/m01a-mut.py`。
- 指纹不含命令 → D2 红
- 缺失文件与空文件同摘要 → D3 红
- SetupFailed 在 Bug 节点拒绝（守卫排除 Node::BUG）→ D9（含 task.rs 对应用例）红
- Paused 接受 SetupFailed → D7 红
- command 不 trim → C2 红
- watch 校验去掉 `..` 检查 → C3 红

验收契约能区分错误实现。未发现产品缺陷、未发现验证缺口。
