# M-07 r12 审计证据

## 负向对照
- README 保留旧图：`docs/images/*.png` 现状即旧图（1200×451 / 742 / 749）。`vitest run test/readme-images.test.ts` 的尺寸用例失败（收到 1200×451，期望 1512×944）——验收确实拦得住旧图。
- 「帮助放 Node 侧」对照：未注入。`web_` 的「没有 Node 时 --help 仍退出 0」用例在现实现下通过，且该用例的判据（退出码）直接区分；风险低，未单独注入。

## 验收命令各段（独立执行）
- `cargo test -p autome-cli --test cli web_`：10 passed。
- `cargo test -p autome-cli --bin autome`：67 passed。
- `cargo build -p automed -p autome-cli`、`npm run build --prefix apps/web`：通过。
- `node apps/web/scripts/screenshots.ts --out .autome/output/shots-1` 与 `shots-2`：均退出 0，各 3 张 1512×944 PNG，残留进程检查 0。**脚本在本环境可运行**（实现轮称 ps EPERM，审计环境无此限制）。
- vitest readme-images：2 passed、1 failed（尺寸用例）；test-groups 通过。

## 结论
产品缺陷：设计要求 README 三张新图由脚本生成并替换 `docs/images/`，现仍是旧图，验收命令失败。审计轮不得改 docs 目录外文件，故不替换。
实现轮下一步：`node apps/web/scripts/screenshots.ts --out docs/images`，重跑 readme-images、test-groups。判别检查即 readme-images 现有尺寸用例（红→绿），无需额外检查。
