#!/bin/bash
# Drives the real autome/automed binaries with a fake CLI to produce a ledger
# covering the W4 notification event shapes. Re-runnable from scratch.
# Usage: w4-ledger.sh ROOT WT [extra]
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
if [ "${3:-}" = extra ]; then
  cd "$REPO"
  mkdir -p .autome/fake/extra-fail
  printf 'exit 3\n' > .autome/fake/extra-fail/1.sh
  export AUTOMED_CLAUDE_BINARY="$ROOT/bin/fake-cli" AUTOMED_CODEX_BINARY="$ROOT/bin/fake-cli"
  "$AUTOME" task new "extra fail" >/dev/null
  "$AUTOME" run --until-idle --interval 0.2 >/dev/null
  exit
fi

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
bug_doc() { # review-value, status-value
  awk '/kernel-contract: bug-status-block/{f=1;next} f&&/^```text/{g=1;next} g&&/^```/{exit} g{print}' "$WT/protocol/bug-protocol.md" \
   | grep -v '^repos:' | sed -e "s/^复核:.*/复核: $1/" -e "s/^status:.*/status: $2/" -e 's/^复现命令:.*/复现命令: sh regression.sh/'
  printf '\n## 现象\n产品标记错误。\n\n## 复现\nsh regression.sh\n\n## 根因分析\nproduct.txt 应为 fixed。\n\n## 修复说明\n修复只改变产品标记。\n\n## 复核意见\n检查回归测试与产品差异。\n'
}
bug_step() { # slug n prelude (body on stdin)
  local slug=$1 n=$2 prelude=$3 body; body=$(cat)
  fakedir "$slug"
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

A() { "$AUTOME" "$@"; }
run() { A run --until-idle --interval 0.2 "$@" > /dev/null 2>>"$OUT/run.stderr.log"; }
tid() { A rpc project.get "{\"project_id\":\"$PID\"}" | python3 -I -c '
import json,sys
d=json.load(sys.stdin); d=d.get("payload",d)
slug=sys.argv[1]
for t in d["tasks"]:
    if t.get("slug")==slug: print(t["id"])' "$1"; }

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
reuse_for_main_check = false # This ledger must exercise the post-merge failure alert.
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

# ===== 1-3: feature task: design approval -> await_merge -> merge -> done =====
feature_steps feature-alpha
A task new "feature alpha" > /dev/null; run          # -> await_design_approval
A approve feature-alpha > /dev/null; run             # -> await_merge
FA=$(tid feature-alpha)
A merge feature-alpha > /dev/null; run               # -> done

# ===== 6: main_gate_failed alert =====
feature_steps breaks-main
A task new "breaks main" > /dev/null; run
A approve breaks-main > /dev/null; run
touch "$ROOT/main-should-fail"
A merge breaks-main > /dev/null; run                 # done + project.alert
rm -f "$ROOT/main-should-fail"

# ===== 4: failed (protocol): doc without status block =====
body_doc 设计中 0 0 > /dev/null
printf '# 标题\n\n没有状态块。\n' | doc_step protocol-fail 1 ""
A task new "protocol fail" > /dev/null; run

# ===== 4b: failed (session_crashed): non-zero exit =====
fakedir crash-fail
printf 'exit 3\n' > "$REPO/.autome/fake/crash-fail/1.sh"
A task new "crash fail" > /dev/null; run

# ===== 5: bug task -> await_info =====
bug_doc 待复核 无法复现 | bug_step need-info 1 ""
bug_doc 待复核 无法复现 | bug_step need-info 2 ""
A task new "need info" --kind bug --brief - <<< $'# Bug card\nExpected fixed' > /dev/null; run
BID=$(tid need-info)

# ===== duplicate-state probe: pause/resume, reply, rerun on tasks =====
feature_steps pause-probe
A task new "pause probe" > /dev/null; run            # await_design_approval
A pause pause-probe > "$OUT/pause.out" 2>&1 || true; run
A resume pause-probe > "$OUT/resume.out" 2>&1 || true; run
A reply need-info "补充：在 Windows 上点两次" > "$OUT/reply.out" 2>&1; run   # bug: back to locate, then await_info again
A rerun crash-fail intake > "$OUT/rerun.out" 2>&1; run                        # failed task rerun (best effort)

bug_doc 待复核 "不是 Bug" | bug_step not-bug 1 ""
bug_doc 待复核 "不是 Bug" | bug_step not-bug 2 ""
A task new "not bug" --kind bug --brief - <<< "不是 Bug" > /dev/null; run

# ===== dumps =====
A rpc events.since '{"after_seq":0,"limit":1000}' > "$OUT/events.json"
A rpc task.get "{\"task_id\":\"$FA\"}" > "$OUT/task-get-feature.json"
A rpc task.get "{\"task_id\":\"$BID\"}" > "$OUT/task-get-bug.json"
A rpc project.get "{\"project_id\":\"$PID\"}" > "$OUT/project-get.json"
A rpc project.list '{}' > "$OUT/project-list.json" 2>/dev/null
A ls > "$OUT/ls.txt" 2>&1
A alerts --json > "$OUT/alerts.json" 2>&1
echo "$ROOT" > "$OUT/scratch-root.txt"
