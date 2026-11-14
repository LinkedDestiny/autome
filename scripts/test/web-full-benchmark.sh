#!/bin/sh
# Audit #8's original check is preserved verbatim in fixtures/web-full-benchmark-audit.sh.
# Run from the repository root. Keep logs separate from prior evidence.
set -eu
out=$(mktemp -d "$PWD/.autome/output/web-benchmark.XXXXXX")
printf 'logs: %s\n' "$out"
# A restricted session cannot certify the required process-leak check.
ps -axo pid=,ppid=,command= > "$out/processes-before.txt"
cd apps/web
failed=0
run() {
  name=$1; shift
  private_tmp=$(mktemp -d "$out/$name.XXXXXX")
  start=$(date +%s)
  uptime > "$out/$name.load-before"
  rc=0
  TMPDIR="$private_tmp" "$@" > "$out/$name.log" 2>&1 || rc=$?
  end=$(date +%s)
  uptime > "$out/$name.load-after"
  sleep 2
  ps -axo pid=,ppid=,command= > "$out/$name.processes"
  # Include descendants even when their command no longer contains TMPDIR.
  awk -v root="$private_tmp" '
    { parent[$1]=$2; line[$1]=$0; if (index($0,root)) found[$1]=1 }
    END { do { changed=0; for (pid in parent) if (!found[pid] && found[parent[pid]]) { found[pid]=1; changed=1 } } while(changed);
      for (pid in line) if(found[pid]) print line[pid] }
  ' "$out/$name.processes" > "$out/$name.leftovers"
  [ ! -s "$out/$name.leftovers" ] || rc=1
  grep -E 'Test Files|Tests ' "$out/$name.log" > "$out/$name.counts" || rc=1
  grep -Eq 'Tests .*skipped' "$out/$name.counts" && rc=1
  printf '%s rc=%s wall=%s\n' "$name" "$rc" "$((end-start))" >> "$out/summary.txt"
  cat "$out/$name.counts" >> "$out/summary.txt"
  [ "$rc" -eq 0 ] || failed=1
}
for i in 1 2 3; do
  run "old$i" npx vitest run --config test/fixtures/vitest-serial.config.ts
  run "new$i" npm run test
done
for i in 1 2 3 4 5; do run "seq$i" npm run test; done
printf 'benchmark exit=%s; see %s\n' "$failed" "$out/summary.txt"
exit "$failed"
