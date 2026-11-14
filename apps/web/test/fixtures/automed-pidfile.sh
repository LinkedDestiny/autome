#!/bin/sh
if [ "$1" != '--version' ]; then
  printf '%s\n' "$$" > "$AUTOME_HOME/automed.pid"
fi
exec "$W3_REAL_AUTOMED" "$@"
