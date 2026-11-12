#!/bin/bash
# Drives the real autome/automed binaries with a fake CLI to produce a ledger
# covering the W4 notification event shapes. Re-runnable from scratch.
# Usage: backfill-ledger.sh ROOT WT setup|redo|reject
set -eu
ROOT=$1
WT=$2
OUT="$ROOT/output"
mkdir -p "$OUT"
AUTOME="$WT/target/debug/autome"
[ -x "$AUTOME" ] || { echo "build first: cargo build -p automed -p autome-cli"; exit 1; }

export AUTOME_HOME="$ROOT/autome-home" HOME="$ROOT/home"
REPO="$ROOT/repo"
mkdir -p "$AUTOME_HOME" "$HOME" "$REPO" "$ROOT/bin"
# --- fake claude/codex (same as crates/automed/tests/end_to_end.rs FAKE_CLI) ---
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

# --- evidence snippet each loop round must leave ---
cat > "$ROOT/evidence.sh" <<'EV'
k=$(grep -m1 '^implementation-round:' "docs/$SLUG/$SLUG.md" 2>/dev/null | sed 's#[^0-9]*\([0-9]*\)/.*#\1#')
if [ -n "${k:-}" ]; then
  mkdir -p "docs/$SLUG/evidence"
  printf '命令：fake\n结果：通过\n' > "docs/$SLUG/evidence/M-01-r$k-impl.md"
  printf '复验：fake\n结论：通过\n' > "docs/$SLUG/evidence/M-01-r$k-audit.md"
  printf '轮次 | M-01 | 通过 | e | 无\n' >> "docs/$SLUG/retro.md"
fi
EV

# --- gate: passes when gate-ok exists; fails on the post-merge checkout of main when flag file exists ---
cat > "$ROOT/gate.sh" <<EOF2
case "\$PWD" in
  *_verify-*) if [ -f "$ROOT/main-should-fail" ]; then echo "main is broken after the merge"; exit 1; fi ;;
esac
if [ -f gate-ok ]; then echo "gate fine on \$AUTOME_HEAD"; exit 0; fi
echo "gate says no: gate-ok is missing"
exit 1
EOF2

# --- helpers: step scripts live in repo/.autome/fake/<slug>/N.sh ---
fakedir() { mkdir -p "$REPO/.autome/fake/$1"; echo 1 > "$REPO/.autome/fake/$1/next"; }
# body_doc status design_round impl_round [ID:state]...
body_doc() {
  local status=$1 dr=$2 ir=$3; shift 3
  printf '# 购物车结算\n\nstatus: %s\ndesign-round: %s/15\nimplementation-round: %s/25\ncurrent-milestone: 无\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n' "$status" "$dr" "$ir"
  if [ $# -gt 0 ]; then
    printf '\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n'
    for m in "$@"; do printf '| %s | %s | %s 标题 | 0 | |\n' "${m%%:*}" "${m##*:}" "${m%%:*}"; done
  fi
}
# doc_step slug n prelude body-from-stdin
doc_step() {
  local slug=$1 n=$2 prelude=$3 body; body=$(cat)
  fakedir "$slug"
  cat > "$REPO/.autome/fake/$slug/$n.sh" <<STEP
set -e
SLUG=$slug
$prelude
mkdir -p "docs/$slug"
cat > "docs/$slug/$slug.md" <<'AUTOME_EOF'
$body
AUTOME_EOF
. "$ROOT/evidence.sh"
git add -A docs gate-ok >/dev/null 2>&1 || git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session $n" >/dev/null 2>&1 || true
STEP
}
retro_step() {
  local slug=$1 n=$2
  fakedir "$slug"
  cat > "$REPO/.autome/fake/$slug/$n.sh" <<STEP
set -e
mkdir -p "docs/$slug"
cat > "docs/$slug/lessons.md" <<'AUTOME_EOF'
# 教训

\`\`\`yaml
- id: L-01
  domain: verification
  symptom: 审计要自己跑一遍验收命令
  root_cause: 实现轮的证据只写了结论，没有写命令
  evidence: docs/$slug/lessons.md
  level: rule
  proposal: 证据文件必须逐字写出跑过的命令 ($slug)
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
\`\`\`
AUTOME_EOF
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session $n retro" >/dev/null 2>&1 || true
STEP
}
# feature_steps slug : full happy path steps 1..7 (+retro 8); gate-ok created in step 5
feature_steps() {
  local s=$1
  body_doc 设计中 0 0 | doc_step "$s" 1 ""
  body_doc 设计中 1 0 | doc_step "$s" 2 ""
  body_doc 设计中 1 0 | doc_step "$s" 3 ""
  body_doc 实现中 2 0 M-01:开放 | doc_step "$s" 4 ""
  body_doc 实现中 2 1 M-01:待审 | doc_step "$s" 5 "echo ok > gate-ok"
  body_doc 实现中 2 1 M-01:已完成 | doc_step "$s" 6 ""
  retro_step "$s" 7
}
A() { "$AUTOME" "$@"; }
run() { A run --until-idle --interval 0.2 "$@" > /dev/null 2>>"$OUT/run.stderr.log"; }
tid() { A rpc project.get "{\"project_id\":\"$PID\"}" | python3 -I -c '
import json,sys
d=json.load(sys.stdin); d=d.get("payload",d)
slug=sys.argv[1]
for t in d["tasks"]:
    if t.get("slug")==slug: print(t["id"])' "$1"; }

assert_state() {
  local id; id=$(tid "$1")
  A rpc task.get "{\"task_id\":\"$id\"}" > "$OUT/task-$1.json"
  node -e 'const d=require(process.argv[1]); const t=(d.payload??d).task; if(t.state.node!==process.argv[2]) {console.error(t.state);process.exit(1)}' "$OUT/task-$1.json" "$2"
}
if [ "$3" = setup ]; then
# --- scratch repo ---
cd "$REPO"
git init -q -b main
git config user.name scratch; git config user.email s@s
echo hello > README.md
printf '.autome/fake/\n' > .gitignore
mkdir -p .autome
cat > .autome/config.toml <<CFG
[gate]
command = "sh $ROOT/gate.sh"
timeout_minutes = 5

[retry]
attempts = 0
backoff_seconds = 0
CFG
git add -A && git commit -q -m initial
: > "$OUT/run.stderr.log"

A project add "$REPO" --skip-onboarding --json > "$OUT/project-add.json" || { echo project add failed; exit 1; }
PID=$(python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); d=d.get("payload",d); print(d["project"]["id"])' "$OUT/project-add.json")
echo "project: $PID"

feature_steps merge-me
A task new "merge me" >/dev/null; run
assert_state merge-me await_design_approval
A approve merge-me >/dev/null; run
assert_state merge-me await_merge
elif [ "$3" = redo ]; then
cd "$REPO"
PID=$(node -e 'const d=require(process.argv[1]); console.log((d.payload??d).project.id)' "$OUT/project-add.json")
feature_steps redo-me
A task new "redo me" >/dev/null; run
assert_state redo-me await_design_approval
elif [ "$3" = reject ]; then
cd "$REPO"
PID=$(node -e 'const d=require(process.argv[1]); console.log((d.payload??d).project.id)' "$OUT/project-add.json")
body_doc 设计中 2 0 | doc_step redo-me 5 ""
body_doc 设计中 3 0 | doc_step redo-me 6 ""
body_doc 实现中 4 0 M-01:开放 | doc_step redo-me 7 ""
echo 5 > "$REPO/.autome/fake/redo-me/next"
A reject redo-me "补充验收要求" >/dev/null; run
assert_state redo-me await_design_approval
else
exit 2
fi
