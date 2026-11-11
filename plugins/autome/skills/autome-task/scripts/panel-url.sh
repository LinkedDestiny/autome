#!/bin/sh
command -v curl >/dev/null 2>&1 || exit 0
server=${AUTOME_HOME:-$HOME/.autome}/web/server.json
port=''
if [ -r "$server" ]; then
  port=$(sed -n 's/.*"port"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$server" | head -1)
fi
default=${AUTOME_PANEL_DEFAULT_PORT:-7420}
ports=$port
[ "$port" = "$default" ] || ports="$ports $default"
for port in $ports; do
  base="http://127.0.0.1:${port}"
  health=$(curl -fsS --max-time 2 "$base/healthz" 2>/dev/null) || continue
  printf '%s' "$health" | grep -Eq '"ok"[[:space:]]*:[[:space:]]*true([[:space:]]*[,}]|[[:space:]]*$)' || continue
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$base/tasks/${1:-}" 2>/dev/null)
  if [ "$code" = 200 ]; then printf '%s/tasks/%s\n' "$base" "${1:-}"
  else printf '%s/\n' "$base"; fi
  exit 0
done
exit 0
