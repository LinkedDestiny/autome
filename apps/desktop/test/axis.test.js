'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const axis = require('../renderer/axis');

const RUN_AXES = [
  { id: 'phase', zh: '阶段', values: [{ id: 'Executing', zh: '执行中' }, { id: 'Ready', zh: '就绪' }] },
  { id: 'hold', zh: '等待', values: [{ id: 'None', zh: '无' }, { id: 'Paused', zh: '已暂停' }] },
  { id: 'terminal', zh: '终态', values: [{ id: 'None', zh: '未终止' }, { id: 'Completed', zh: '已完成' }] },
];

test('buildAxisStrip renders one cell per axis, never collapsing N axes to 1', () => {
  const strip = axis.buildAxisStrip(RUN_AXES, { phase: 'Executing', hold: 'None', terminal: 'None' });
  assert.equal(strip.length, RUN_AXES.length);
  assert.deepEqual(strip.map((cell) => cell.axisId), ['phase', 'hold', 'terminal']);
});

test('an axis missing from the values map renders as not-yet-observed, not dropped', () => {
  const strip = axis.buildAxisStrip(RUN_AXES, { phase: 'Executing' });
  assert.equal(strip.length, 3);
  const hold = strip.find((cell) => cell.axisId === 'hold');
  assert.equal(hold.observed, false);
  assert.equal(hold.valueId, axis.UNOBSERVED);
  assert.equal(hold.valueZh, '未观测');
});

test('an empty values map renders every axis as not-yet-observed', () => {
  const strip = axis.buildAxisStrip(RUN_AXES, {});
  assert.ok(strip.every((cell) => cell.observed === false));
});

test('an observed value resolves its zh label from the axis value list', () => {
  const strip = axis.buildAxisStrip(RUN_AXES, { phase: 'Ready', hold: 'None', terminal: 'None' });
  const phase = strip.find((cell) => cell.axisId === 'phase');
  assert.equal(phase.observed, true);
  assert.equal(phase.valueId, 'Ready');
  assert.equal(phase.valueZh, '就绪');
});

test('an unknown observed value id throws rather than silently rendering garbage', () => {
  assert.throws(() => axis.buildAxisStrip(RUN_AXES, { phase: 'NoSuchPhase' }), RangeError);
});

test('coreComputedConclusion is structurally distinct from an axis-strip cell', () => {
  const strip = axis.buildAxisStrip(RUN_AXES, { phase: 'Executing', hold: 'None', terminal: 'None' });
  const conclusion = axis.coreComputedConclusion('readiness', '就绪度', 'ready', '就绪');
  assert.equal(conclusion.kind, 'core-computed');
  assert.ok(!strip.some((cell) => cell.kind === 'core-computed'));
});

test('coreComputedConclusion with no value observed renders unobserved, not a guessed default', () => {
  const conclusion = axis.coreComputedConclusion('readiness', '就绪度', null, null);
  assert.equal(conclusion.observed, false);
  assert.equal(conclusion.valueId, axis.UNOBSERVED);
  assert.equal(conclusion.valueZh, '未观测');
});
