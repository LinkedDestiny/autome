'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const status = require('../renderer/status');
const vocab = require('../renderer/vocabulary');

test('statusChip supports exactly the four documented tones', () => {
  assert.deepEqual(status.TONES, ['ok', 'warning', 'critical', 'unobserved']);
});

test('statusChip rejects an unknown tone rather than defaulting to ok', () => {
  assert.throws(() => status.statusChip('fine'), RangeError);
});

test('the unobserved tier is dashed and never mistakable for a pass', () => {
  const chip = status.statusChip('unobserved');
  assert.equal(chip.dashed, true);
  assert.equal(chip.label, '未观测');
  assert.notEqual(chip.tone, 'ok');
});

test('ok/warning/critical are solid, not dashed', () => {
  for (const tone of ['ok', 'warning', 'critical']) {
    assert.equal(status.statusChip(tone).dashed, false);
  }
});

test('each tone has a distinct icon so color is never the only signal', () => {
  const icons = status.TONES.map((tone) => status.statusChip(tone).icon);
  assert.equal(new Set(icons).size, icons.length);
});

test('gateChip returns unobserved for any unevaluated check, regardless of the passed flag', () => {
  assert.equal(status.gateChip(false, true).tone, 'unobserved');
  assert.equal(status.gateChip(false, false).tone, 'unobserved');
});

test('gateChip only returns ok when observed and passed', () => {
  assert.equal(status.gateChip(true, true).tone, 'ok');
  assert.equal(status.gateChip(true, false).tone, 'critical');
});

test('missingComponentTone: core_task_required missing is critical, everything else is warning', () => {
  for (const cls of vocab.REQUIREMENT_CLASSES) {
    const tone = status.missingComponentTone(cls.id, vocab.REQUIREMENT_CLASSES);
    if (cls.id === 'core_task_required') {
      assert.equal(tone, 'critical');
    } else {
      assert.equal(tone, 'warning');
    }
  }
});

test('missingComponentTone rejects an unknown requirement class', () => {
  assert.throws(() => status.missingComponentTone('nonexistent', vocab.REQUIREMENT_CLASSES), RangeError);
});
