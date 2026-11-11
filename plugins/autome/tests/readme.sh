# README checks, sourced by run.sh and verify-cli.sh.
readme="$plugin/README.md"
example() {
  sed -n '/^<!-- '"$1"' -->$/,/^<!-- \/'"$1"' -->$/p' "$readme" | sed '1d;$d;/^```/d'
}
R1() {
  [ "$(grep '^## ' "$readme")" = "$(printf '## 技能\n## 安装\n## 在对话里用\n## Agent 程序化创建\n## 验证')" ]
}
R2() {
  section=$(sed -n '/^## Agent 程序化创建$/,/^## 验证$/p' "$readme")
  for text in 'task new --brief - --kind' '--json' 'autome task start <id>' 'autome project ls --json'; do
    printf '%s\n' "$section" | grep -Fq -- "$text" || return 1
  done
}
R3() {
  for key in id kind slug title state branch doc_dir started queue_position; do
    example task-new-json | grep -q '"'"$key"'":' || return 1
  done
}
R4() {
  grep -Fq 'autome：' "$readme" && grep -Fq '执行失败退出码 1' "$readme" && grep -Fq '用法错误退出码 2' "$readme"
}
R5() {
  for text in 'claude plugin install autome@autome' 'codex plugin add autome@autome' 'cp -R /path/to/autome-v2/plugins/autome ~/.cursor/plugins/local/autome' 'validate plugins/autome --strict' 'claude plugin install loop@autome' 'codex plugin add loop@autome' 'cp -R /path/to/autome-v2/plugins/loop ~/.cursor/plugins/local/loop'; do
    grep -Fq "$text" "$plugin/../README.md" || return 1
  done
}
# The CLI pretty-prints one key per line. Compare the complete structure and values;
# only non-null ids, paths and timestamps may vary between isolated ledgers.
normalize_example() {
  sed -E \
    -e 's/("(id|path|created_at|added_at|completed_at|removed_at|doc_dir)": )"[^"]*"/\1"<example>"/' \
    -e 's/^[[:space:]]*//' -e '/^$/d'
}
compare_example() {
  example "$1" | normalize_example > "$tmp/expected"
  normalize_example < "$2" > "$tmp/actual"
  diff -u "$tmp/expected" "$tmp/actual"
}
