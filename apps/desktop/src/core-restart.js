'use strict';

// When Main restarts a core that died, and when it stops trying.
//
// Restarting is right for the failure the policy was written for: a core that
// ran for a while and then crashed or was killed. The state it owned is in
// SQLite and on disk, so a new process picks up where the old one left off,
// and the user sees a blink rather than a dead app.
//
// It is exactly wrong for a core that cannot start at all — a database it
// refuses to open, a binary that is missing, a port-less machine. That core
// will fail the same way every second for as long as the app is open, and
// because Main's only report was `connected: false`, the window said "内核不
// 可达" and nothing else. An unbounded retry of a deterministic failure is not
// resilience; it is a loop that hides its own cause.
//
// So a start that does not survive `QUICK_FAILURE_MS` counts as a failed
// start rather than a crash, and after `MAX_QUICK_FAILURES` of them in a row
// Main stops and says why. The threshold is generous on purpose: the core
// opens its database and runs migrations before it is of any use, and a
// machine under load can take a second to do it.

// Longer than any legitimate startup, shorter than any session worth keeping.
const QUICK_FAILURE_MS = 5000;
const MAX_QUICK_FAILURES = 3;
const RESTART_DELAY_MS = 1000;

/**
 * Decides what to do about a core that just exited.
 *
 * `ranForMs` is how long it lived, `quickFailures` how many failed starts
 * preceded it. Returns the new count — a core that lived long enough clears
 * it, so an app left open for a week does not eventually refuse to restart a
 * core that crashed three times over six days.
 *
 * @returns {{restart: boolean, quickFailures: number, delayMs: number}}
 */
function nextRestart({ ranForMs, quickFailures = 0 } = {}) {
  const lived = Number.isFinite(ranForMs) ? ranForMs : 0;
  if (lived >= QUICK_FAILURE_MS) {
    return { restart: true, quickFailures: 0, delayMs: RESTART_DELAY_MS };
  }
  const failures = quickFailures + 1;
  return {
    restart: failures < MAX_QUICK_FAILURES,
    quickFailures: failures,
    delayMs: RESTART_DELAY_MS,
  };
}

module.exports = { nextRestart, QUICK_FAILURE_MS, MAX_QUICK_FAILURES, RESTART_DELAY_MS };
