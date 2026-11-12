# M-02 r2 审计复验

结论：通过。

## 负向对照（均实跑，每次还原，`git status --porcelain` 为空）
| 注入 | 结果 |
|---|---|
| bell.tsx 按钮加 `style={{background:"#fff"}}` | styles「不散写颜色」「无内联样式」两条红（28/30） |
| `.shell` overflow:hidden→auto | styles 一屏布局红 |
| 删 `.project-list` 的 min-height:0 | styles 一屏布局红 |
| `writeGate` 在 core 未知时 allowed:true | live-status「全域」「open 尚无 core」两条红 |
| `needsKernelReload` 恒 false | live-status 恢复 hook 用例红 |
| 删 `--tile-peach` | styles token 逐条一致红 |
均能拦住错误实现，验收命令有鉴别力。

## 验收
- 定点 5 文件：30 通过（10+6+4+8+2）。
- lint 0、typecheck 0、Web 全量 37 文件 330 用例全过，skipped 0（基线 310+20，与预期一致）、build 0，`Nunito*.ttf` 计数 1。
- desktop：104 用例，103 通过、0 失败、1 skipped（W5 不改 desktop，数量不变）。
- 本环境可监听端口，全量中含 server/shell 相关用例均通过。
- 宿主门禁 cc7b3bd 通过（简报）。

## 未取证
真实浏览器 1512×944 一屏肉眼验收属 Playwright/W12 范围，设计已列入人工验收，非 M-02 会话可跑项。
