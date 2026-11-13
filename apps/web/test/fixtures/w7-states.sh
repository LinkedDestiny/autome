#!/bin/bash
# Uses only public CLI/RPC; extends the existing isolated flow fixture.
set -eu
source "$(dirname "$0")/w7-flow.sh" "$1" "$2"
N=2
new_task() {
  local slug=$1; shift
  A task new "$slug" "$@" > /dev/null
  N=$((N + 1)); echo "T-$N" >> "$ROOT/task-ids"
  printf '%s T-%s\n' "$slug" "$N" >> "$ROOT/state-ids"
}
# Wait states and failures finish deterministically before adding live sessions.
for slug in approve-probe reject-probe matrix-approval; do
  for n in 1 2 3 4; do sed "s/feat/$slug/g" "$REPO/.autome/fake/feat/$n.sh" > "$ROOT/step-copy"; fakedir "$slug"; cp "$ROOT/step-copy" "$REPO/.autome/fake/$slug/$n.sh"; done
  new_task "$slug"
done
for slug in reply-probe matrix-info; do
  bug_doc 待复核 | sed 's/^status:.*/status: 无法复现/' | bug_step "$slug" 1 ""
  new_task "$slug" --kind bug
done
for slug in convert-probe matrix-not-bug; do
  bug_doc 待复核 | sed 's/^status:.*/status: 不是 Bug/' | bug_step "$slug" 1 ""
  new_task "$slug" --kind bug
done
for slug in rerun-probe matrix-crash; do
  fakedir "$slug"; echo 'exit 3' > "$REPO/.autome/fake/$slug/1.sh"
  new_task "$slug"
done
for slug in extend-probe matrix-budget; do
  body_doc 设计中 0 0 M-01:开放 M-02:开放 | doc_step "$slug" 1 ""
  for n in 2 3 4; do body_doc 设计中 15 0 M-01:开放 M-02:开放 | doc_step "$slug" "$n" ""; done
  new_task "$slug"
done
run
# Separate draft and cancellation controls.
new_task start-probe --draft
new_task cancel-probe --draft
new_task matrix-draft --draft
new_task matrix-bug-draft --draft --kind bug
new_task matrix-cancelled --draft
A cancel matrix-cancelled > /dev/null
sed -i '' 's/parallel = 4/parallel = 5/' "$REPO/.autome/config.toml"
# All long sessions remain in named processes until teardown, never a real agent.
for slug in matrix-active matrix-paused matrix-stopped matrix-bug-active pause-probe stop-probe resume-probe; do
  fakedir "$slug"
  printf 'exec "%s/bin/w7-sleep" 300\n' "$ROOT" > "$REPO/.autome/fake/$slug/1.sh"
  if [ "$slug" = matrix-bug-active ]; then new_task "$slug" --kind bug; else new_task "$slug"; fi
  if [ "$slug" = matrix-paused ]; then A pause "$slug" > /dev/null; fi
  if [ "$slug" = matrix-stopped ]; then A stop "$slug" > /dev/null; fi
done
