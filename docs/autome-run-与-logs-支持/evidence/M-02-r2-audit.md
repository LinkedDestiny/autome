# M-02 r2 审计证据（审计 #2）

验收命令 `cargo test -p autome-cli`：单元 32、集成 18 全绿（与契约预期一致：28+4、17+1）。

负向对照（逐个注入、跑全量、`git checkout src` 还原，工作区干净）：
- 写出改 `write_all(..).map_err(Failed)`：集成 `logs_json_...` 红。
- Codex 会话改用 `render_line`：单元 `stream_codex` 红。
- `stream_complete_bytes` 不留半行（返回 len）：单元 `stream_line_boundaries` 红。
- 数据源改读 `.log`：集成 `logs_json_...` 红。

结论：验收能区分四种错误实现；产品行为符合设计 §4。未发现缺陷。
人工项（真实 Claude/Codex 会话下的 `-f` 跟随）已在设计文档「人工验收清单」中，由用户合并前确认；B-04 为自动化取证 Backlog。
