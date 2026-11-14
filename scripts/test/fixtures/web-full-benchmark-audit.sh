cd apps/web
O=../../.autome/output/m06c
run(){ n=$1; shift; s=$(date +%s); l=$(uptime|sed 's/.*averages: //'); "$@" > $O/$n.log 2>&1; rc=$?; e=$(date +%s); echo "$n rc=$rc wall=$((e-s)) load=[$l] $(grep -E 'Test Files|Tests ' $O/$n.log | tr -s ' ' | tr '\n' ';')" >> $O/summary.txt; }
for i in 1 2 3; do run old$i npx vitest run --config vitest.old.config.ts; run new$i npm run test; done
for i in 1 2 3 4 5; do run seq$i npm run test; done
echo DONE >> $O/summary.txt
