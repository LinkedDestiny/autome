# When a test reads another already-claimed directory, add that directory to
# its inputs here. Unclaimed paths invalidate every step, but cannot detect
# a missing cross-directory dependency between two already-claimed inputs.
GLOBAL_INPUTS='scripts .github .githooks .gitignore Cargo.toml Cargo.lock rust-toolchain.toml
  apps/web/package.json apps/web/package-lock.json apps/desktop/package.json apps/desktop/package-lock.json'
NO_GATE='docs .autome .claude AGENTS.md README.md'

step 'Format' 'crates' cargo fmt --all -- --check
step 'Clippy' 'crates protocol' cargo clippy --all-targets -- -D warnings
step 'Test (unit, end to end, adapter flags)' 'crates protocol' --cases cargo cargo test --workspace --all-targets --no-fail-fast -- --nocapture
prep 'Desktop install' sh scripts/ci/install-node.sh apps/desktop
prep 'Build core' cargo build -p automed -p autome-cli
step 'Desktop' 'apps/desktop apps/web crates protocol' npm test --prefix apps/desktop
step 'Contract' 'crates/automed/src/dispatch.rs' sh -c '
  test -f crates/automed/src/dispatch.rs &&
  grep -q "pub const READ_METHODS" crates/automed/src/dispatch.rs
'
step 'Scripts' '' node --test scripts/test/*.test.mjs
prep 'Web install' sh scripts/ci/install-node.sh apps/web
step 'Web lint' 'apps/web' npm run lint --prefix apps/web
step 'Web typecheck' 'apps/web' npm run typecheck --prefix apps/web
step 'Web build' 'apps/web' npm run build --prefix apps/web
step 'Web test' 'apps/web crates protocol plugins apps/desktop' --cases vitest npm run test --prefix apps/web
step 'Web e2e' 'apps/web crates protocol plugins apps/desktop' npm run e2e --prefix apps/web
