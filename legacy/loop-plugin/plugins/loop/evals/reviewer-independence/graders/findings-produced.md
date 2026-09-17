# Grader：评审轮确实产出了 review

## 判定目标
AT1 结束时 `docs/mini-eval-task/mini-eval-task-review.md` 存在且非空，含明确结论（通过 / 需要修订 / 目标不可实现）与按七项字段列出的问题或「实现注意事项」（证明评审真的做了，
而非空转规避独立性检查）。

## 机检优先
- `docs/mini-eval-task/mini-eval-task-review.md` 存在且字节数 > 0 → 1，否则 0。

## 阳性对照（C-22）
> 应判负样本：review 文件不存在或为空。期望 grader 给 0。
> 与 `no-adjudication-read.md` 联用可防「不读裁决记录但也不干活」的退化通过。
