# Sourced by run.sh; all fixtures live in its temporary directory.
scripts="$plugin/skills/autome-task/scripts"
feature() {
  printf '# 测试\n## 背景\n已有讨论\n## 要做什么\n实现讨论内容\n## 不做什么\n无\n## 验收标准\n- 可检查\n## 参考\n无\n'
}
bug() {
  printf '# 缺陷\n## 现象\n失败\n## 复现步骤\n1. 执行操作\n## 期望行为\n成功\n## 实际行为\n报错\n## 环境\n测试版本\n## 线索\n无\n'
}
card_result() {
  sh "$scripts/check-card.sh" "$1" < "$tmp/card" > "$tmp/result"
  result=$?
  [ "$result" -eq "$2" ]
}
K1() { feature > "$tmp/card"; card_result feature 0 && grep -Fq "OK $(wc -c < "$tmp/card" | tr -d ' ') 字节" "$tmp/result"; }
K2() { bug > "$tmp/card"; card_result bug 0; }
K3() { feature | sed '1s/# /标题 /' > "$tmp/card"; card_result feature 1; }
K4() { feature | sed '/## 不做什么/,+1d' > "$tmp/card"; card_result feature 1; }
K5() { feature | sed 's/## 背景/## 要做什么/;s/## 参考/## 背景/' > "$tmp/card"; card_result feature 1; }
K6() { feature > "$tmp/card"; printf '## 多余\n内容\n' >> "$tmp/card"; card_result feature 1; }
K7() { feature | sed 's/- 可检查/无/' > "$tmp/card"; card_result feature 1; }
K8() { bug | sed 's/1\. 执行操作/执行操作/' > "$tmp/card"; card_result bug 1; }
sized_card() {
  feature > "$tmp/card"
  size=$(wc -c < "$tmp/card")
  awk -v n="$(($1 - size))" 'BEGIN { for(i=0;i<n;i++) printf "x" }' >> "$tmp/card"
}
K9() { sized_card 65536; card_result feature 0; }
K10() { sized_card 65537; card_result feature 1 && grep -q 65537 "$tmp/result"; }
K11() {
  feature > "$tmp/card"
  printf 'AUTOME_CARD_EOF\ntouch %s/probe\n' "$tmp" >> "$tmp/card"
  card_result feature 0 && [ ! -e "$tmp/probe" ] && grep -Fq "OK $(wc -c < "$tmp/card" | tr -d ' ') 字节" "$tmp/result"
}
K12() { feature > "$tmp/card"; card_result chore 2; }
K13() { feature > "$tmp/card"; printf '\n```md\n## 不是小节\n```\n' >> "$tmp/card"; card_result feature 0; }
K14() { bug > "$tmp/card"; card_result bug 0; }
# Boundary and failure cases supplement the contractual K1-K14.
K15() { : > "$tmp/card"; card_result feature 1; }
K16() { feature | sed '1s/.*/#   /' > "$tmp/card"; card_result feature 1; }
K17() {
  for placeholder in 无 待补充 TBD TODO; do
    feature | sed "s/已有讨论/$placeholder/" > "$tmp/card"
    card_result feature 1 || return 1
  done
}
K18() { feature | sed '/## 参考/,$d' > "$tmp/card"; printf '## 参考\n  \n' >> "$tmp/card"; card_result feature 1; }
K19() { feature | sed 's/- 可检查/1. 可检查/' > "$tmp/card"; card_result feature 0; }
setup_panel() {
  mkdir -p "$tmp/bin" "$tmp/home/web" "$tmp/no-curl"
  rm -f "$tmp/home/web/server.json"
  cat > "$tmp/curl-stub" <<'STUB'
#!/bin/sh
for url do :; done
case "$url" in
  */healthz)
    case "$PANEL_MODE:$url" in
      default:*:7420/*|custom:*:7555/*) printf '{"ok":true}'; exit 0 ;;
      false:*) printf '{"ok":false}'; exit 0 ;;
      http-error:*) exit 22 ;;
      timeout:*) exit 28 ;;
      *) exit 7 ;;
    esac ;;
  *)
    case "$PANEL_MODE" in custom) printf 200 ;; task-down) printf 000; exit 7 ;; *) printf 404 ;; esac ;;
esac
STUB
  chmod +x "$tmp/curl-stub"
  ln -sf "$tmp/curl-stub" "$tmp/bin/curl"
}
panel_result() {
  output=$(AUTOME_HOME="$tmp/home" AUTOME_PANEL_DEFAULT_PORT=7420 PANEL_MODE="$1" PATH="$tmp/bin:$PATH" sh "$scripts/panel-url.sh" T-9)
  [ "$?" -eq 0 ] && [ "$output" = "$2" ]
}
P1() { setup_panel; panel_result down ''; }
P2() { setup_panel; panel_result default 'http://127.0.0.1:7420/'; }
P3() { setup_panel; printf '{"port":7555}' > "$tmp/home/web/server.json"; panel_result custom 'http://127.0.0.1:7555/tasks/T-9'; }
P4() { setup_panel; printf '{"port":7555}' > "$tmp/home/web/server.json"; panel_result default 'http://127.0.0.1:7420/'; }
P5() { setup_panel; panel_result false ''; }
P6() { setup_panel; output=$(PATH="$tmp/no-curl" /bin/sh "$scripts/panel-url.sh" T-9); [ "$?" -eq 0 ] && [ -z "$output" ]; }
P7() {
  setup_panel
  command -v curl >/dev/null || return 1
  output=$(AUTOME_HOME="$tmp/home" AUTOME_PANEL_DEFAULT_PORT=1 sh "$scripts/panel-url.sh" T-9)
  [ "$?" -eq 0 ] && [ -z "$output" ]
}
P8() { setup_panel; panel_result timeout ''; }
P9() { setup_panel; panel_result http-error ''; }
P10() { setup_panel; printf '{"unknown":7555}' > "$tmp/home/web/server.json"; panel_result default 'http://127.0.0.1:7420/'; }
