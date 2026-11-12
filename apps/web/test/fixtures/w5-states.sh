#!/bin/sh
# Isolated real-core fixture: failed, draft, then queued for the live supervisor.
set -eu
ROOT=$1
WT=$2
export AUTOME_HOME="$ROOT/home"
AUTOME="$WT/target/debug/autome"
mkdir -p "$ROOT/home" "$ROOT/repo/.autome"
cat > "$ROOT/fake-cli" <<'CLI'
#!/bin/sh
cat >/dev/null
case "$PWD" in
  *active-probe*) exec sleep 120 ;;
  *) exit 3 ;;
esac
CLI
chmod +x "$ROOT/fake-cli"
export AUTOMED_CLAUDE_BINARY="$ROOT/fake-cli" AUTOMED_CODEX_BINARY="$ROOT/fake-cli"
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
"$AUTOME" project add "$ROOT/repo" --skip-onboarding > /dev/null
"$AUTOME" task new 'failure probe' > /dev/null
"$AUTOME" run --until-idle --interval 0.1 > /dev/null
"$AUTOME" task new 'draft probe' --draft > /dev/null
"$AUTOME" task new 'active probe' > /dev/null
