# prompt 基准

`prompts/` 下这几个文件是当前协议种子渲染出来的 prompt 原文，`{slug}`、
`{request}`、`{budget_line}` 三个占位符保留在原位。

它们最早是**协议出仓之前**由编译期常量拼出来的那一份，用来证明「把协议搬出
二进制」没有顺手改掉任何一个字——七份逐字节一致。此后它们跟着协议走。

`prompt_rendering.rs` 拿当前种子渲染同样的 prompt，逐字节对比这几份。

对比失败**不一定是 bug**。协议本来就是要演进的——改了 prompt 模板，这个测试
当然会红。红了之后的正确做法是：看一遍 diff，确认改动是有意的，然后

```sh
AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering
```

把基准更新成新的，和协议改动放在同一个提交里，提交信息写清改了什么、为什么。

不要在没看 diff 的情况下直接刷新。这个测试的价值全在「看一眼」上。
