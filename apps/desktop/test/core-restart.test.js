'use strict';

// The restart policy, which exists because the previous one had no bound.
//
// A core that cannot start fails identically every time. Restarting it once a
// second forever is not resilience — it is a loop that produces no new
// information and hides the one line that would have explained it.

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  nextRestart,
  QUICK_FAILURE_MS,
  MAX_QUICK_FAILURES,
} = require('../src/core-restart');

test('a core that ran for a while and then died is restarted, and clears the count', () => {
  const decision = nextRestart({ ranForMs: QUICK_FAILURE_MS * 10, quickFailures: 2 });
  assert.equal(decision.restart, true);
  assert.equal(decision.quickFailures, 0, 'a working core is not held against the next one');
  assert.ok(decision.delayMs > 0);
});

test('a start that does not survive the threshold counts as a failed start', () => {
  const decision = nextRestart({ ranForMs: 40, quickFailures: 0 });
  assert.equal(decision.restart, true);
  assert.equal(decision.quickFailures, 1);
});

test('after the third failed start in a row, Main stops restarting', () => {
  let failures = 0;
  const outcomes = [];
  for (let i = 0; i < 5; i += 1) {
    const decision = nextRestart({ ranForMs: 40, quickFailures: failures });
    failures = decision.quickFailures;
    outcomes.push(decision.restart);
    if (!decision.restart) break;
  }
  assert.deepEqual(outcomes, [true, true, false]);
  assert.equal(failures, MAX_QUICK_FAILURES);
});

test('a long-lived core between two failed starts resets the budget', () => {
  // Three crashes over a week are three separate incidents, not a reason to
  // leave the app coreless on the third.
  let failures = nextRestart({ ranForMs: 10, quickFailures: 0 }).quickFailures;
  failures = nextRestart({ ranForMs: 10, quickFailures: failures }).quickFailures;
  failures = nextRestart({ ranForMs: QUICK_FAILURE_MS + 1, quickFailures: failures })
    .quickFailures;
  const decision = nextRestart({ ranForMs: 10, quickFailures: failures });
  assert.equal(decision.restart, true);
  assert.equal(decision.quickFailures, 1);
});

test('a missing lifetime is treated as a failed start, not as a long run', () => {
  // `onExit` computes the lifetime; if that ever arrives undefined, the safe
  // reading is the one that eventually stops.
  const decision = nextRestart({ quickFailures: MAX_QUICK_FAILURES - 1 });
  assert.equal(decision.restart, false);
});
