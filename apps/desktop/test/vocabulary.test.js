'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const vocab = require('../renderer/vocabulary');

function assertNoDuplicateIds(list, name) {
  const ids = list.map((entry) => entry.id);
  assert.equal(new Set(ids).size, ids.length, `${name} has duplicate ids`);
}

function assertAllHaveNonEmptyZh(list, name) {
  for (const entry of list) {
    assert.ok(entry.zh && entry.zh.length > 0, `${name} entry "${entry.id}" has no zh label`);
  }
}

test('RunPhase: 19 variants, matching run.rs RunPhase exactly', () => {
  assert.equal(vocab.RUN_PHASES.length, 19);
  assertNoDuplicateIds(vocab.RUN_PHASES, 'RUN_PHASES');
  assertAllHaveNonEmptyZh(vocab.RUN_PHASES, 'RUN_PHASES');
});

test('RUN_LINEAR_NOMINAL_PATH: 17 steps, excludes Repairing and Replanning', () => {
  assert.equal(vocab.RUN_LINEAR_NOMINAL_PATH.length, 17);
  assert.ok(!vocab.RUN_LINEAR_NOMINAL_PATH.includes('Repairing'));
  assert.ok(!vocab.RUN_LINEAR_NOMINAL_PATH.includes('Replanning'));
  assert.equal(new Set(vocab.RUN_LINEAR_NOMINAL_PATH).size, 17);
});

test('RunHold: 14 variants, including Blocked with 5 BlockedReason sub-variants', () => {
  assert.equal(vocab.RUN_HOLDS.length, 14);
  assertNoDuplicateIds(vocab.RUN_HOLDS, 'RUN_HOLDS');
  assertAllHaveNonEmptyZh(vocab.RUN_HOLDS, 'RUN_HOLDS');
  assert.ok(vocab.RUN_HOLDS.some((h) => h.id === 'Blocked'));
  assert.equal(vocab.BLOCKED_REASONS.length, 5);
  assertNoDuplicateIds(vocab.BLOCKED_REASONS, 'BLOCKED_REASONS');
});

test('RunTerminal: 6 variants', () => {
  assert.equal(vocab.RUN_TERMINALS.length, 6);
  assertNoDuplicateIds(vocab.RUN_TERMINALS, 'RUN_TERMINALS');
  assertAllHaveNonEmptyZh(vocab.RUN_TERMINALS, 'RUN_TERMINALS');
});

test('ProjectPhase: 8 variants', () => {
  assert.equal(vocab.PROJECT_PHASES.length, 8);
  assertNoDuplicateIds(vocab.PROJECT_PHASES, 'PROJECT_PHASES');
  assertAllHaveNonEmptyZh(vocab.PROJECT_PHASES, 'PROJECT_PHASES');
});

test('ProjectHold: 7 variants', () => {
  assert.equal(vocab.PROJECT_HOLDS.length, 7);
  assertNoDuplicateIds(vocab.PROJECT_HOLDS, 'PROJECT_HOLDS');
  assertAllHaveNonEmptyZh(vocab.PROJECT_HOLDS, 'PROJECT_HOLDS');
});

test('CompletionGate: exactly 40 named boolean fields, each with a zh gloss', () => {
  assert.equal(vocab.COMPLETION_GATES.length, 40);
  assertNoDuplicateIds(vocab.COMPLETION_GATES, 'COMPLETION_GATES');
  assertAllHaveNonEmptyZh(vocab.COMPLETION_GATES, 'COMPLETION_GATES');
});

test('SkillEvidenceLadder: 6 rungs, Installed through Effective in order', () => {
  assert.equal(vocab.SKILL_EVIDENCE_LEVELS.length, 6);
  assert.deepEqual(
    vocab.SKILL_EVIDENCE_LEVELS.map((l) => l.id),
    ['installed', 'bound', 'discoverable', 'available_to_attempt', 'invoked', 'effective']
  );
});

test('environment axes: 5 orthogonal axes with plan §5.9 value counts', () => {
  assert.equal(vocab.ENVIRONMENT_AXES.length, 5);
  const counts = Object.fromEntries(vocab.ENVIRONMENT_AXES.map((a) => [a.id, a.values.length]));
  assert.deepEqual(counts, {
    presence: 3,
    integrity: 3,
    auth_mode: 4,
    auth_state: 5,
    qualification: 4,
  });
  for (const axis of vocab.ENVIRONMENT_AXES) {
    assertNoDuplicateIds(axis.values, `ENVIRONMENT_AXES.${axis.id}`);
  }
});

test('readiness is Core-computed with 3 values, not a 6th orthogonal axis', () => {
  assert.equal(vocab.READINESS_VALUES.length, 3);
  assert.ok(!vocab.ENVIRONMENT_AXES.some((a) => a.id === 'readiness'));
});

test('requirement classes: 5 classes, exactly one is core_task_required (critical)', () => {
  assert.equal(vocab.REQUIREMENT_CLASSES.length, 5);
  const critical = vocab.REQUIREMENT_CLASSES.filter((c) => c.missingSeverity === 'critical');
  assert.deepEqual(critical.map((c) => c.id), ['core_task_required']);
});
