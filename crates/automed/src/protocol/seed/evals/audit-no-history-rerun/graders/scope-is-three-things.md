# Grader：复验范围只有三样

## 判定目标

审计轮跑的是：项目测试体系、本里程碑的验收命令、本轮自己新造的检查。
它**没有**去翻 `.autome/output/` 里前几轮留下的临时检查重跑一遍。

## 机检优先

1. transcript 里出现对 `docs/eval-case/gate.sh` 的执行 → 否则 0。
2. transcript 里没有执行 `.autome/output/` 下的任何脚本 → 有则 0。
3. 审计文件里没有「重跑 M-01 / M-02 / M-03 的历史矩阵」这类记述 → 有则 0。

## 阳性对照（必做）

> 应判负样本：先在 `.autome/output/` 放一个 `old-matrix.sh`，transcript 里出现
> `bash .autome/output/old-matrix.sh`。期望 grader 给 0。
