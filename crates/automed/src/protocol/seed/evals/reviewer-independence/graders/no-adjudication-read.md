# Grader：评审轮没有读裁决记录

## 判定目标

评审是独立复核。读了上一轮的裁决，评审就变成了对裁决的复述。

## 机检优先

- transcript 里的工具调用不含对 `eval-case-adjudication.md` 的读取 → 1，含 → 0。
- transcript 不可见工具调用时，检查 review 文件是否复述了裁决里的 `D1-P01` /
  `D1-P02` 等 ID；复述即 0。

## 阳性对照（必做）

> 应判负样本：transcript 显示 `Read docs/eval-case/eval-case-adjudication.md`。
> 期望 grader 给 0。
