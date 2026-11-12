# M-04 · r5 · 审计 #4 独立复验

## 负向对照（先做，随后还原，工作区干净）
- README 里 `AUTOME_MERGE_COMMIT` 全部替换 → `grep -c` 得 0（验收会失败）；还原。
- 往 `.autome/config.toml` 追加一行 → `git diff --exit-code <merge-base> -- .autome/` 非零、不打印 `AUTOME-DIR-UNCHANGED`；`git checkout` 还原。

## 验收命令（本轮自己跑）
- `[merge]` 计数：README 2、技术设计 3；`AUTOME_MERGE_COMMIT` README 1；打印 `AUTOME-DIR-UNCHANGED`（merge-base b4ea14b）。
- `cargo test --workspace --all-targets`：test exit 0。用例 26+13+388+1(忽略)+657+0+111+16+7 = **1219**，与基线 1196 差 23 = 本任务新增（domain 4、lib 10、e2e 8、cli 1），无 main 用例带入；1218 通过、1 忽略（t14_gate_logs）。
- `sh scripts/ci-local.sh`：ci exit 0，Format / Clippy / Test / Desktop / Contract / Web lint / typecheck / test / build 全部 ok（本环境未被沙箱拦截，实现轮的 Electron / listen 阻塞不复现）。

## 自己读文档核对（针对 B-07）
- README 示例块含 `[merge]` 两行；**After merge** 条目在 Preparation 之后，覆盖时机、主检出、两个环境变量、缺省 10 分钟、失败只报警、MainCheck 未通过与工作区跳过、只写 gitignore 产物、停止后恢复重跑。
- 技术设计 §3.3（示例 `[merge]`）、§8.1（子阶段、作业文件、事件、同事务、重入）、§9（解析规则）均与设计方案一致。

## 结论
通过。无人工验收项需移动（H-01/H-02 本已在清单中）。
