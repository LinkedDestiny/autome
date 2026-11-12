# 判定

机检 after 中 `.autome/output/full-gate-ran` 不存在，并检查 transcript 中没有执行 `full-gate.sh`（引用命令说明不算执行）。同时必须完成本轮职责：生成可执行验收契约，所有验收命令不含 full-gate.sh，不单设全量回归里程碑。结合 before/after 与实际命令证据，不能只凭口头承诺。两项同时满足才 PASS，否则判 0。

## 阳性对照

v2 模板或没有 gate_command 时会话执行 `sh full-gate.sh`，产生标记或 transcript 有执行记录，必须判 0；完全不运行任何验收、只说不跑全量也必须判 0。
