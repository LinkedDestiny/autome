'use strict';

// Guards against renderer/vocabulary.js silently drifting from the Rust
// source it mirrors. vocabulary.js's own header notes a research pass once
// miscounted CompletionGate at 41 fields (actual: 40, verified by hand);
// the count-only check (`COMPLETION_GATES.length === 40`) that caught that
// still can't catch a rename — a field dropped and a differently-spelled
// one added would keep the count stable. This does a real set/sequence
// comparison against the Rust source text via regex extraction. Full
// schema codegen (T4-M0) is a separate, larger increment — see the plan's
// "明确不做".

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vocab = require('../renderer/vocabulary');

const REPO_ROOT = path.join(__dirname, '..', '..', '..');
const COMPLETION_RS = path.join(REPO_ROOT, 'crates', 'autome-domain', 'src', 'completion.rs');
const RUN_RS = path.join(REPO_ROOT, 'crates', 'autome-domain', 'src', 'run.rs');
const PROJECT_RS = path.join(REPO_ROOT, 'crates', 'autome-domain', 'src', 'project.rs');

function extractEnumVariants(source, enumName) {
  const marker = `pub enum ${enumName} {`;
  const start = source.indexOf(marker);
  assert.ok(start !== -1, `enum ${enumName} not found in project.rs`);
  const bodyStart = start + marker.length;
  const bodyEnd = source.indexOf('\n}', bodyStart);
  assert.ok(bodyEnd !== -1, `could not find the closing brace of enum ${enumName}`);
  const body = source.slice(bodyStart, bodyEnd);
  const variants = [...body.matchAll(/^\s*([A-Za-z][A-Za-z0-9]*)\s*,?\s*$/gm)].map((m) => m[1]);
  assert.ok(variants.length > 0, `regex extracted zero variants from enum ${enumName}`);
  return variants;
}

function extractCompletionGateFieldNames(source) {
  const invocation = source.indexOf('completion_gate! {');
  assert.ok(invocation !== -1, 'completion_gate! macro invocation not found in completion.rs');
  const bodyStart = source.indexOf('\n', invocation) + 1;
  const bodyEnd = source.indexOf('\n}', bodyStart);
  assert.ok(bodyEnd !== -1, 'could not find the closing brace of the completion_gate! invocation');
  const body = source.slice(bodyStart, bodyEnd);
  const names = [...body.matchAll(/^\s*([a-z_][a-z0-9_]*)\s*:/gm)].map((m) => m[1]);
  assert.ok(names.length > 0, 'regex extracted zero field names from completion_gate! — check the marker still matches');
  return names;
}

function extractRunLinearNominalPath(source) {
  const marker = 'const RUN_LINEAR_NOMINAL_PATH: [RunPhase; 17] = [';
  const start = source.indexOf(marker);
  assert.ok(start !== -1, 'RUN_LINEAR_NOMINAL_PATH constant not found in run.rs — check its type/len annotation still matches');
  const bodyStart = start + marker.length;
  const bodyEnd = source.indexOf('];', bodyStart);
  assert.ok(bodyEnd !== -1, 'could not find the closing bracket of RUN_LINEAR_NOMINAL_PATH');
  const body = source.slice(bodyStart, bodyEnd);
  const variants = [...body.matchAll(/RunPhase::([A-Za-z][A-Za-z0-9]*)/g)].map((m) => m[1]);
  assert.ok(variants.length > 0, 'regex extracted zero variants from RUN_LINEAR_NOMINAL_PATH');
  return variants;
}

test('vocabulary.js COMPLETION_GATES ids are exactly completion.rs field names — set equality, catches renames the length-only check misses', () => {
  const rustFields = extractCompletionGateFieldNames(fs.readFileSync(COMPLETION_RS, 'utf8'));
  const jsIds = vocab.COMPLETION_GATES.map((g) => g.id);
  assert.equal(jsIds.length, rustFields.length);
  assert.deepEqual([...jsIds].sort(), [...rustFields].sort());
});

test('vocabulary.js RUN_LINEAR_NOMINAL_PATH matches run.rs RUN_LINEAR_NOMINAL_PATH exactly, same order', () => {
  const rustPath = extractRunLinearNominalPath(fs.readFileSync(RUN_RS, 'utf8'));
  assert.deepEqual(vocab.RUN_LINEAR_NOMINAL_PATH, rustPath);
});

test('vocabulary.js PROJECT_PHASES ids are exactly project.rs ProjectPhase variants — set equality', () => {
  const rustVariants = extractEnumVariants(fs.readFileSync(PROJECT_RS, 'utf8'), 'ProjectPhase');
  const jsIds = vocab.PROJECT_PHASES.map((p) => p.id);
  assert.equal(jsIds.length, rustVariants.length);
  assert.deepEqual([...jsIds].sort(), [...rustVariants].sort());
});

test('vocabulary.js PROJECT_HOLDS ids are exactly project.rs ProjectHold variants — set equality', () => {
  const rustVariants = extractEnumVariants(fs.readFileSync(PROJECT_RS, 'utf8'), 'ProjectHold');
  const jsIds = vocab.PROJECT_HOLDS.map((h) => h.id);
  assert.equal(jsIds.length, rustVariants.length);
  assert.deepEqual([...jsIds].sort(), [...rustVariants].sort());
});
