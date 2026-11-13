#!/bin/sh
# Shared isolated project for W6 upload, create and live tests.
set -eu
ROOT=$1
WT=$2
export AUTOME_HOME="$ROOT/home"
mkdir -p "$ROOT/home" "$ROOT/repo/.autome"
cat > "$ROOT/fake-cli" <<'CLI'
#!/bin/sh
cat >/dev/null
exec sleep 120
CLI
chmod +x "$ROOT/fake-cli"
cat > "$ROOT/core-wrapper" <<WRAPPER
#!/bin/sh
export AUTOMED_CLAUDE_BINARY="$ROOT/fake-cli" AUTOMED_CODEX_BINARY="$ROOT/fake-cli"
exec "$WT/target/debug/automed" "\$@"
WRAPPER
chmod +x "$ROOT/core-wrapper"
cd "$ROOT/repo"
git init -q -b main
git -c user.name=test -c user.email=t@t commit --allow-empty -qm initial
printf '[retry]\nattempts = 0\nbackoff_seconds = 0\n' > .autome/config.toml
if [ -n "${3:-}" ]; then printf '\n[loop]\nparallel = %s\n' "$3" >> .autome/config.toml; fi
"$WT/target/debug/autome" project add "$ROOT/repo" --skip-onboarding > /dev/null
