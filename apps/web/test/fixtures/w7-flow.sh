#!/bin/bash
# Real kernel fixture. Usage: bash w7-flow.sh ROOT WT; never invokes a real agent.
set -eu
ROOT=$1; WT=$2; OUT=$ROOT/output; mkdir -p "$OUT"
AUTOME="${AUTOME_BIN:-$WT/target/debug/autome}"
AUTOMED="${AUTOMED_BIN:-$WT/target/debug/automed}"
export AUTOME_HOME="$ROOT/autome-home"
REPO="$ROOT/repo"; mkdir -p "$AUTOME_HOME" "$REPO" "$ROOT/bin"

cat > "$ROOT/bin/fake-cli" <<'FAKE'
#!/bin/sh
set -u
dir=$PWD
fake=""
while [ "$dir" != "/" ]; do
  if [ -d "$dir/.autome/fake" ]; then fake="$dir/.autome/fake"; break; fi
  dir=$(dirname "$dir")
done
[ -n "$fake" ] || exit 90
slug=$(basename "$PWD")
[ -d "$fake/$slug" ] && fake="$fake/$slug"
n=$(cat "$fake/next" 2>/dev/null || echo 1)
step="$fake/$n.sh"
echo "$n" >> "$fake/history"
expr "$n" + 1 > "$fake/next"
if [ ! -f "$step" ]; then echo "fake-cli: no step $n" >&2; exit 92; fi
cat > "$fake/$n.prompt"
sh "$step" "$fake/$n.prompt"
FAKE
chmod +x "$ROOT/bin/fake-cli"

export AUTOMED_CLAUDE_BINARY="$ROOT/bin/fake-cli" AUTOMED_CODEX_BINARY="$ROOT/bin/fake-cli"
cat > "$ROOT/evidence.sh" <<'EV'
k=$(grep -m1 '^implementation-round:' "docs/$SLUG/$SLUG.md" 2>/dev/null | sed 's#[^0-9]*\([0-9]*\)/.*#\1#')
if [ -n "${k:-}" ]; then
  mkdir -p "docs/$SLUG/evidence"
  printf '命令：fake\n结果：通过\n' > "docs/$SLUG/evidence/M-01-r$k-impl.md"
  printf '复验：fake\n结论：通过\n' > "docs/$SLUG/evidence/M-01-r$k-audit.md"
  printf '轮次 | M-01 | 通过 | e | 无\n' >> "docs/$SLUG/retro.md"
fi
EV


fakedir() { mkdir -p "$REPO/.autome/fake/$1"; echo 1 > "$REPO/.autome/fake/$1/next"; }
body_doc() {
  local status=$1 dr=$2 ir=$3; shift 3
  printf '# t\n\nstatus: %s\ndesign-round: %s/15\nimplementation-round: %s/25\ncurrent-milestone: M-02\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n' "$status" "$dr" "$ir"
  printf '\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n'
  for m in "$@"; do printf '| %s | %s | %s 标题 | 0 | |\n' "${m%%:*}" "${m##*:}" "${m%%:*}"; done
  printf '\n## Backlog\n\n- B-01 以后再做的事\n\n## 争议项\n\n- D-01 有争议的事\n'
}
doc_step() {
  local slug=$1 n=$2 prelude=$3 body; body=$(cat); fakedir "$slug"
  cat > "$REPO/.autome/fake/$slug/$n.sh" <<STEP
set -e
$prelude
mkdir -p "docs/$slug/evidence"
cat > "docs/$slug/$slug.md" <<'AUTOME_EOF'
$body
AUTOME_EOF
SLUG=$slug
. "$ROOT/evidence.sh"
git add -A >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session $n" >/dev/null 2>&1 || true
STEP
}
bug_doc() {
  awk '/kernel-contract: bug-status-block/{f=1;next} f&&/^```text/{g=1;next} g&&/^```/{exit} g{print}' "$WT/protocol/bug-protocol.md" \
   | grep -v '^repos:' | sed -e "s/^复核:.*/复核: $1/" -e 's/^复现命令:.*/复现命令: sh regression.sh/'
  printf '\n## 现象\nx\n\n## 复现\nsh regression.sh\n\n## 根因分析\ny\n\n## 修复说明\nz\n\n## 复核意见\nw\n'
}
bug_step() { local slug=$1 n=$2 prelude=$3 body; body=$(cat); fakedir "$slug"
  cat > "$REPO/.autome/fake/$slug/$n.sh" <<STEP
set -e
$prelude
mkdir -p docs/$slug
cat > docs/$slug/$slug-bug.md <<'BUG_EOF'
$body
BUG_EOF
git add -A
git -c user.name=fake -c user.email=f@f commit -qm 'bug round $n' || true
STEP
}
ln -s /bin/sleep "$ROOT/bin/w7-sleep"
cat > "$ROOT/gate.sh" <<GATE
if [ "\${AUTOME_GATE_NODE:-}" = main_check ]; then
  while [ ! -f "$ROOT/release-main-check" ]; do "$ROOT/bin/w7-sleep" 0.1; done
fi
exit 0
GATE
cat > "$ROOT/core-wrapper" <<WRAPPER
#!/bin/sh
export AUTOMED_CLAUDE_BINARY="$ROOT/bin/fake-cli" AUTOMED_CODEX_BINARY="$ROOT/bin/fake-cli"
exec "$AUTOMED" "\$@"
WRAPPER
chmod +x "$ROOT/core-wrapper"
A() { "$AUTOME" "$@"; }
run() { A run --until-idle --interval 0.2 > /dev/null 2>>"$OUT/run.stderr.log"; }
cd "$REPO"; git init -q -b main; git config user.name s; git config user.email s@s
echo hello > README.md; printf '.autome/fake/\n' > .gitignore; mkdir -p .autome
cat > .autome/config.toml <<CFG
[gate]
command = "sh $ROOT/gate.sh"
quick = "sh $ROOT/gate.sh"
run = "before_merge"
timeout_minutes = 5
reuse_for_main_check = false

[loop]
parallel = 4

[retry]
attempts = 0
backoff_seconds = 0
CFG
git add -A && git commit -q -m initial
A project add "$REPO" --skip-onboarding > /dev/null
S=feat
body_doc 设计中 0 0 M-01:开放 M-02:开放 | doc_step $S 1 ""
body_doc 设计中 1 0 M-01:开放 M-02:开放 | doc_step $S 2 ""
body_doc 设计中 1 0 M-01:开放 M-02:开放 | doc_step $S 3 ""
body_doc 实现中 2 0 M-01:开放 M-02:开放 | doc_step $S 4 ""
body_doc 实现中 2 1 M-01:待审 M-02:开放 | doc_step $S 5 ""
body_doc 实现中 2 1 M-01:已完成 M-02:开放 | doc_step $S 6 ""
body_doc 实现中 2 2 M-01:已完成 M-02:待审 | doc_step $S 7 ""
body_doc 实现中 2 2 M-01:已完成 M-02:已完成 | doc_step $S 8 ""
body_doc 已完成 2 2 M-01:已完成 M-02:已完成 | doc_step $S 9 ""
B=bugx
bug_doc 待复核 | bug_step $B 1 "printf 'broken\\n' > product.txt; printf 'echo out; test \"\$(cat product.txt)\" = fixed\\n' > regression.sh"
bug_doc 待复核 | bug_step $B 2 "printf 'fixed\\n' > product.txt"
bug_doc 通过 | bug_step $B 3 ""
# Register each id immediately, including partial setup failures.
A task new "feat" --brief - <<< $'# 卡\n<b>raw</b> [x](javascript:alert(1))' > /dev/null
echo T-1 >> "$ROOT/task-ids"
A task new "bugx" --kind bug --brief - <<< '# Bug card' > /dev/null
echo T-2 >> "$ROOT/task-ids"
run
# The real design approval is a user barrier: a second drain is required.
A approve feat > /dev/null
run
echo more > other.txt
git add other.txt
git commit -qm "main moves"
A rpc task.get '{"task_id":"T-1"}' > "$OUT/feature.json"
A rpc task.get '{"task_id":"T-2"}' > "$OUT/bug.json"
A rpc task.changes '{"task_id":"T-1"}' > "$OUT/changes.json"
A rpc events.since '{"after_seq":0,"limit":1000}' > "$OUT/events.json"

# Optional regeneration of checked-in, unmodified kernel response fixtures.
if [ -n "${W7_CAPTURE_DIR:-}" ]; then
  mkdir -p "$W7_CAPTURE_DIR"
  cp "$OUT/feature.json" "$OUT/bug.json" "$OUT/changes.json" "$OUT/events.json" "$W7_CAPTURE_DIR/"
fi
