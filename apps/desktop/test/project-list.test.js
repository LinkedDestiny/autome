'use strict';

// §5.1: the project list panel resolves real project.list data through
// axis.js:buildAxisStrip, which throws RangeError for any value id it does
// not recognize. Rust and the JS vocabulary can drift (a variant renamed or
// added on one side first) — this must degrade to an unobserved cell, never
// crash the whole screen. Also covers the NULL display_name case: rows
// created before Project identity existed have no name to show, and must
// render an honest historical-fallback label rather than an empty string.

const test = require('node:test');
const assert = require('node:assert/strict');
const nav = require('../renderer/nav');

test('panelFor returns a project-list panel with a 2-cell axis strip per project when nothing is selected', () => {
  const state = nav.setProjects(nav.initialState(), [
    { id: 'p1', display_name: 'Alpha', phase: 'Ready', hold: 'None' },
  ]);
  const panel = nav.panelFor(state);
  assert.equal(panel.kind, 'project-list');
  assert.equal(panel.items.length, 1);
  assert.equal(panel.items[0].id, 'p1');
  assert.equal(panel.items[0].displayName, 'Alpha');
  assert.equal(panel.items[0].axes.length, 2);
  assert.ok(panel.items[0].axes.every((cell) => cell.observed === true));
});

test('an unrecognized phase/hold value degrades to unobserved instead of throwing RangeError', () => {
  const state = nav.setProjects(nav.initialState(), [
    { id: 'p1', display_name: 'Alpha', phase: 'SomeFutureRustVariant', hold: 'AlsoUnknown' },
  ]);
  assert.doesNotThrow(() => nav.panelFor(state));
  const panel = nav.panelFor(state);
  assert.ok(panel.items[0].axes.every((cell) => cell.observed === false));
});

test('a missing phase/hold value (undefined, not just unrecognized) also degrades to unobserved', () => {
  const state = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  const panel = nav.panelFor(state);
  assert.ok(panel.items[0].axes.every((cell) => cell.observed === false));
});

test('a NULL display_name (historical row predating identity) renders as 未登记身份（历史数据）, never empty or undefined', () => {
  const state = nav.setProjects(nav.initialState(), [
    { id: 'p1', display_name: null, phase: 'Registered', hold: 'None' },
  ]);
  const panel = nav.panelFor(state);
  assert.equal(panel.items[0].displayName, '未登记身份（历史数据）');
});

test('selecting a project carries its display name (or the historical fallback) into project-detail', () => {
  const withNullName = nav.selectProject(
    nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: null, phase: 'Registered', hold: 'None' }]),
    'p1'
  );
  assert.equal(nav.panelFor(withNullName).project.displayName, '未登记身份（历史数据）');

  const withRealName = nav.selectProject(
    nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha', phase: 'Registered', hold: 'None' }]),
    'p1'
  );
  assert.equal(nav.panelFor(withRealName).project.displayName, 'Alpha');
});
