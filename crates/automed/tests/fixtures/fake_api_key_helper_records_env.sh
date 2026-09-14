#!/bin/sh
# Test-only apiKeyHelper stand-in: records the full environment it was
# invoked with to a sibling file (one per invocation, PID-suffixed so
# retries don't clobber each other), then emits a fixed dummy key on
# stdout — the only channel a real apiKeyHelper is documented to use.
# Never emit a real credential from this fixture.
dir=$(dirname "$0")
env > "$dir/observed_env.$$.txt"
echo "dummy-key-from-fixture-helper-do-not-leak"
