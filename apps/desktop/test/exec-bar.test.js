'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const execBar = require('../renderer/exec-bar');

test('disconnected state always produces a write-disabled flag and an as-of label', () => {
  const bar = execBar.buildExecBar({ connection: { status: 'disconnected', asOf: '2026-09-14T09:00:00Z' } });
  assert.equal(bar.writeDisabled, true);
  assert.equal(bar.asOfLabel, '截至 2026-09-14T09:00:00Z');
});

test('disconnected with no asOf timestamp still renders a label, never blank', () => {
  const bar = execBar.buildExecBar({ connection: { status: 'disconnected' } });
  assert.equal(bar.writeDisabled, true);
  assert.equal(bar.asOfLabel, '截至未知时间');
});

test('connected state disables writes never and has no as-of label', () => {
  const bar = execBar.buildExecBar({ connection: { status: 'connected' } });
  assert.equal(bar.writeDisabled, false);
  assert.equal(bar.asOfLabel, null);
});

test('empty bar is still a well-formed model, not hidden (isEmpty flag, zero counts)', () => {
  const bar = execBar.buildExecBar({ connection: { status: 'connected' } });
  assert.equal(bar.isEmpty, true);
  assert.deepEqual(bar.running, []);
  assert.deepEqual(bar.waitingOnMe, []);
  assert.equal(bar.queuedCount, 0);
});

test('queued items sort as a strict FIFO by enqueuedEventSeq, ties broken by taskId', () => {
  const bar = execBar.buildExecBar({
    connection: { status: 'connected' },
    queued: [
      { taskId: 'task-b', projectLabel: 'p', enqueuedEventSeq: 5 },
      { taskId: 'task-a', projectLabel: 'p', enqueuedEventSeq: 5 },
      { taskId: 'task-c', projectLabel: 'p', enqueuedEventSeq: 2 },
    ],
  });
  assert.deepEqual(bar.queued.map((q) => q.taskId), ['task-c', 'task-a', 'task-b']);
});

test('queued items never carry an implicit priority field, only cancelable', () => {
  const bar = execBar.buildExecBar({
    connection: { status: 'connected' },
    queued: [{ taskId: 'task-a', projectLabel: 'p', enqueuedEventSeq: 1 }],
  });
  assert.deepEqual(Object.keys(bar.queued[0]).sort(), ['cancelable', 'enqueuedEventSeq', 'leaseHolder', 'projectLabel', 'taskId'].sort());
  assert.equal(bar.queued[0].cancelable, true);
});

test('a queued item without a numeric enqueuedEventSeq is rejected rather than silently sorted last', () => {
  assert.throws(
    () => execBar.buildExecBar({ connection: { status: 'connected' }, queued: [{ taskId: 'x', projectLabel: 'p' }] }),
    TypeError
  );
});

test('an item without a taskId is rejected', () => {
  assert.throws(
    () => execBar.buildExecBar({ connection: { status: 'connected' }, running: [{ projectLabel: 'p' }] }),
    TypeError
  );
});

test('fromQueueSnapshot maps a held lease into the running lane', () => {
  const input = execBar.fromQueueSnapshot(
    { entries: [], lease: { lease_id: 'lease-1', task_id: 'task-running' } },
    { status: 'connected' }
  );
  const bar = execBar.buildExecBar(input);
  assert.deepEqual(bar.running.map((r) => r.taskId), ['task-running']);
  assert.deepEqual(bar.queued, []);
});

test('fromQueueSnapshot maps queue entries into the queued lane in wire order, buildExecBar still re-sorts them', () => {
  const input = execBar.fromQueueSnapshot(
    {
      entries: [
        ['task-b', 5],
        ['task-a', 2],
      ],
      lease: null,
    },
    { status: 'connected' }
  );
  const bar = execBar.buildExecBar(input);
  assert.deepEqual(bar.running, []);
  assert.deepEqual(bar.queued.map((q) => q.taskId), ['task-a', 'task-b']);
});

test('fromQueueSnapshot never fabricates a project label for lease or queue entries', () => {
  const input = execBar.fromQueueSnapshot(
    { entries: [['task-a', 1]], lease: { lease_id: 'lease-1', task_id: 'task-running' } },
    { status: 'connected' }
  );
  const bar = execBar.buildExecBar(input);
  assert.equal(bar.running[0].projectLabel, '未观测');
  assert.equal(bar.queued[0].projectLabel, '未观测');
});

test('fromQueueSnapshot with no queueState still yields a well-formed disconnected input', () => {
  const input = execBar.fromQueueSnapshot(null, null);
  const bar = execBar.buildExecBar(input);
  assert.equal(bar.connectionStatus, 'disconnected');
  assert.equal(bar.isEmpty, true);
});

test('fromQueueSnapshot resolves real project labels from entry_labels for both the running lease and queued entries', () => {
  const input = execBar.fromQueueSnapshot(
    { entries: [['task-queued', 1]], lease: { lease_id: 'lease-1', task_id: 'task-running' } },
    { status: 'connected' },
    [
      { task_id: 'task-running', project_id: 'p1', project_display_name: 'Alpha' },
      { task_id: 'task-queued', project_id: 'p2', project_display_name: 'Beta' },
    ]
  );
  const bar = execBar.buildExecBar(input);
  assert.equal(bar.running[0].projectLabel, 'Alpha');
  assert.equal(bar.queued[0].projectLabel, 'Beta');
});

test('fromQueueSnapshot falls back to 未观测 for a task absent from entry_labels', () => {
  const input = execBar.fromQueueSnapshot(
    { entries: [], lease: { lease_id: 'lease-1', task_id: 'task-running' } },
    { status: 'connected' },
    [{ task_id: 'some-other-task', project_id: 'p1', project_display_name: 'Alpha' }]
  );
  const bar = execBar.buildExecBar(input);
  assert.equal(bar.running[0].projectLabel, '未观测');
});

test('fromQueueSnapshot treats a missing entryLabels argument exactly like the pre-existing two-argument call', () => {
  const withoutLabels = execBar.fromQueueSnapshot(
    { entries: [], lease: { lease_id: 'lease-1', task_id: 'task-running' } },
    { status: 'connected' }
  );
  const bar = execBar.buildExecBar(withoutLabels);
  assert.equal(bar.running[0].projectLabel, '未观测');
});
