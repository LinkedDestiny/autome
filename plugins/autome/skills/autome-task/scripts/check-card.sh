#!/bin/sh
LC_ALL=C
export LC_ALL

case "${1:-}" in feature|bug) [ "$#" -eq 1 ] || exit 2 ;; *) exit 2 ;; esac
card=$(mktemp) || exit 1
trap 'rm -f "$card"' EXIT HUP INT TERM
cat > "$card" || exit 1
bytes=$(wc -c < "$card" | tr -d ' ')
awk -v kind="$1" -v bytes="$bytes" '
function fail(message) { print "不合格：" message; bad=1 }
function section( clean) {
  if (!count) return
  clean=content; gsub(/[[:space:]]/, "", clean)
  if (clean == "") fail("小节不能为空：" heading)
  else if (required[heading] && clean ~ /^(无|待补充|TBD|TODO)$/) fail("必填小节不能是占位：" heading)
  if (heading == "验收标准" && !bullet) fail("验收标准至少一个列表项")
  if (heading == "复现步骤" && !numbered) fail("复现步骤至少一个编号项")
}
BEGIN {
  if (kind == "feature") {
    n=split("背景 要做什么 不做什么 验收标准 参考", expected, " ")
    required["背景"]=required["要做什么"]=required["验收标准"]=1
  } else {
    n=split("现象 复现步骤 期望行为 实际行为 环境 线索", expected, " ")
    for (i=1;i<n;i++) required[expected[i]]=1
  }
}
NR == 1 {
  title=substr($0,3); clean=title; gsub(/[[:space:]]/, "", clean)
  if ($0 !~ /^# / || clean == "") fail("第一行必须是非空的 # 标题")
  next
}
/^[[:space:]]*```/ { fence=!fence; content=content $0 "\n"; next }
!fence && /^## / {
  section(); count++; heading=substr($0,4)
  if (count > n || heading != expected[count]) fail("小节名称或顺序不符：" heading)
  content=""; bullet=0; numbered=0; next
}
{
  content=content $0 "\n"
  if (!fence && /^[[:space:]]*(- |[0-9]+\. )/) bullet=1
  if (!fence && /^[[:space:]]*[0-9]+\. /) numbered=1
}
END {
  if (NR == 0) fail("正文为空")
  section()
  if (count != n) fail("小节数量不符")
  if (bytes > 65536) fail("正文 " bytes " 字节，超过 65536 字节")
  if (bad) exit 1
  print "OK " bytes " 字节 · 标题：" title
}' "$card"
