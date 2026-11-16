#!/bin/sh
# Audit #7 regression command; run from the repository root.
for i in 1 2 3 4 5; do (cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01') || exit 1; done
