# When a test reads another already-claimed directory, add that directory to
# its inputs here. Unclaimed paths invalidate every step, but cannot detect
# a missing cross-directory dependency between two already-claimed inputs.
GLOBAL_INPUTS='scripts .github .githooks .gitignore Cargo.toml Cargo.lock rust-toolchain.toml
  apps/web/package.json apps/web/package-lock.json apps/desktop/package.json apps/desktop/package-lock.json'
NO_GATE='docs .autome .claude AGENTS.md README.md'

step 'Format' 'crates' cargo fmt --all -- --check
step 'Clippy' 'crates protocol' cargo clippy --all-targets -- -D warnings
step 'Test (unit, end to end, adapter flags)' 'crates protocol' --cases cargo cargo test --workspace --all-targets --no-fail-fast -- --nocapture
prep 'Desktop install' --if-missing apps/desktop/node_modules sh -c 'cd apps/desktop && { npm ci || npm install; }'
prep 'Build core' cargo build -p automed -p autome-cli
step 'Desktop' 'apps/desktop crates protocol' npm test --prefix apps/desktop
step 'Contract' 'crates/automed/src/dispatch.rs' sh -c '
  test -f crates/automed/src/dispatch.rs &&
  grep -q "pub const READ_METHODS" crates/automed/src/dispatch.rs
'
step 'Scripts' '' node --test scripts/test/*.test.mjs
prep 'Web install' --if-missing apps/web/node_modules sh -c 'cd apps/web && npm ci'
step 'Web lint' 'apps/web' npm run lint --prefix apps/web
step 'Web typecheck' 'apps/web' npm run typecheck --prefix apps/web
step 'Web build' 'apps/web' npm run build --prefix apps/web
step 'Web test' 'apps/web crates protocol plugins apps/desktop/renderer' --cases vitest npm run test --prefix apps/web
