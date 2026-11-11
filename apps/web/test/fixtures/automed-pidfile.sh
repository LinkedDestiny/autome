#!/bin/sh
printf '%s\n' "$$" > "$AUTOME_HOME/automed.pid"
exec "$W3_REAL_AUTOMED"
