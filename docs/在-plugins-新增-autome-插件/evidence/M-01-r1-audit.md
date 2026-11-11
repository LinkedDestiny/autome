# M-01 审计 #1 独立复验

- 验收命令 `run.sh structure`：10/10；`hosts.sh`：5/5（共 15，与契约一致）。
- 负向对照（均已还原，git status 干净）：
  - 分别删去三份市场文件（`plugins/.agents|.claude-plugin|.cursor-plugin/marketplace.json`）里的 autome 条目 → S6 失败（S7 亦失败，因本审计用 json.dump 重排了格式）。
  - Codex plugin.json version 改 0.1.1 → S2 失败，H4、H5 失败。
  - Claude plugin.json 去掉 author → H2 失败。
- 结论：验收能区分错误实现；产品行为符合 M-01。通过。
- 人工项 H-01…H-06 属后续里程碑/人工清单，不影响 M-01。
- 提交由 Autome 兜底（如被拒）。
