# M-05 r7 审计复验

- 负向对照（每次改完跑 `cargo test -p autome-cli cost_cli_`，随后还原，工作区无残留）：
  - A 金额 `{:.2}`→`{}`：两个 render 单测失败 ✔
  - B `status` 费用行「其中 m 个」误用 `sessions`：`cost_cli_status_task_prints_the_cost_line_under_state` 失败 ✔
  - C 脚注无条件打印：`cost_cli_ls_appends_cost_and_a_footnote` 失败 ✔
- 验收命令：`cost_cli_` 5 个全过（2 单测 + 3 cli）；`sh scripts/ci-local.sh` ci exit 0，九项全绿（Format/Clippy/Test/Desktop/Contract/Web lint/typecheck/test/build，519s）。输出在 `.autome/output/m05-ci.txt`。
- 实现轮报告的 Electron SIGABRT / Web listen EPERM 是实现轮沙箱环境问题，本轮环境下全量门禁通过，非产品缺陷。
- 结论：通过。
