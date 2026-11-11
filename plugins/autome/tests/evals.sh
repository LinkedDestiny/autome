# Sourced by run.sh; fixed case format follows plugins/loop/evals.
eval_cases='ask-before-inventing bug-missing-repro kind-ambiguous'
E1() {
  for scenario in $eval_cases; do
    case_file="$plugin/evals/$scenario/case.yaml"
    [ -f "$case_file" ] || return 1
    for key in name description prompt runs max_turns timeout_seconds; do
      grep -Eq "^${key}:.*[^[:space:]]" "$case_file" || return 1
    done
    grep -qx "graders:" "$case_file" || return 1
    grep -qx "name: $scenario" "$case_file" || return 1
    for key in runs max_turns timeout_seconds; do
      grep -Eq "^${key}: [1-9][0-9]*$" "$case_file" || return 1
    done
    refs=$(sed -n 's/^  - \(graders\/[^ ]*\.md\)$/\1/p' "$case_file")
    [ -n "$refs" ] || return 1
    for ref in $refs; do
      [ -s "$plugin/evals/$scenario/$ref" ] || return 1
    done
  done
}
E2() {
  for scenario in $eval_cases; do
    for grader in "$plugin/evals/$scenario/graders/"*.md; do
      [ -f "$grader" ] && grep -qx '## 阳性对照' "$grader" || return 1
    done
  done
}
E3() {
  for grader in "$plugin/evals/ask-before-inventing/graders/"*.md; do
    sed '/^## 阳性对照/,$d' "$grader" | grep -q '验收标准' || return 1
  done
}
E4() {
  for scenario in $eval_cases; do
    [ -f "$plugin/evals/$scenario/case.yaml" ] || return 1
    if grep -q 'scaffold_script' "$plugin/evals/$scenario/case.yaml"; then return 1; fi
  done
}
E5() {
  file="$plugin/evals/README.md"
  grep -Eq '^claude plugin eval ./plugins/autome .*--max-cost-usd [0-9]+\.[0-9]+' "$file" &&
  grep -q '不进门禁' "$file" &&
  grep -Fq '不要加 `--allow-tools Bash`' "$file"
}
