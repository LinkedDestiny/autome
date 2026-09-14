'use strict';

// Global execution bar model (plan §9: 常驻全局执行条 —— 当前完全缺失，本次
// 新建). Three lanes — 当前运行 / 等待我 / 排队 N — each item carries a
// Project label and a jumpable Task id. Queue order is a strict FIFO on
// (enqueued_event_seq, task_id): "只能查看、取消或等待，不支持隐式优先级".
//
// When Core is unreachable, cached data must still render, stamped with
// "截至某时", and every write affordance must be disabled — never silently
// hidden (plan: "无数据时不隐藏，显示明确空态").
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeExecBar = api;
  }
})(function () {
  function normalizeItem(item) {
    if (!item || !item.taskId) {
      throw new TypeError('exec-bar item requires a taskId');
    }
    return Object.freeze({
      taskId: item.taskId,
      projectLabel: item.projectLabel || '未观测',
    });
  }

  function normalizeQueuedItem(item) {
    const base = normalizeItem(item);
    if (typeof item.enqueuedEventSeq !== 'number') {
      throw new TypeError(`queued item "${item.taskId}" requires a numeric enqueuedEventSeq`);
    }
    return Object.freeze({
      taskId: base.taskId,
      projectLabel: base.projectLabel,
      enqueuedEventSeq: item.enqueuedEventSeq,
      leaseHolder: item.leaseHolder || null,
      cancelable: true,
    });
  }

  // Strict FIFO by (enqueued_event_seq, task_id) — no implicit priority.
  function sortQueue(items) {
    return [...items].sort((a, b) => {
      if (a.enqueuedEventSeq !== b.enqueuedEventSeq) {
        return a.enqueuedEventSeq - b.enqueuedEventSeq;
      }
      return String(a.taskId).localeCompare(String(b.taskId));
    });
  }

  function formatAsOf(isoTimestamp) {
    return isoTimestamp ? `截至 ${isoTimestamp}` : '截至未知时间';
  }

  // input: {
  //   connection: { status: 'connected'|'disconnected', asOf?: ISOString },
  //   running: [{taskId, projectLabel}],
  //   waitingOnMe: [{taskId, projectLabel}],
  //   queued: [{taskId, projectLabel, enqueuedEventSeq, leaseHolder}],
  // }
  function buildExecBar(input) {
    const source = input || {};
    const connection = source.connection || {};
    const connected = connection.status === 'connected';

    const running = (source.running || []).map(normalizeItem);
    const waitingOnMe = (source.waitingOnMe || []).map(normalizeItem);
    const queued = sortQueue((source.queued || []).map(normalizeQueuedItem));

    return Object.freeze({
      connectionStatus: connected ? 'connected' : 'disconnected',
      writeDisabled: !connected,
      asOfLabel: connected ? null : formatAsOf(connection.asOf),
      running,
      waitingOnMe,
      queued,
      queuedCount: queued.length,
      isEmpty: running.length === 0 && waitingOnMe.length === 0 && queued.length === 0,
    });
  }

  // §5.1: `queue.get`'s Reply now carries `entry_labels: [{task_id,
  // project_id, project_display_name}]` alongside `state`, resolved
  // server-side via one join (no per-item round trip). Builds a
  // taskId->displayName lookup; a task absent from the map (not in any
  // known project, or the label read failed) stays honestly '未观测'
  // rather than fabricated.
  function projectLabelLookup(entryLabels) {
    const lookup = new Map();
    for (const entry of entryLabels || []) {
      if (entry && entry.task_id && entry.project_display_name) {
        lookup.set(entry.task_id, entry.project_display_name);
      }
    }
    return lookup;
  }

  // Maps a `queue.get` Reply's `state` payload (ExecutionQueue's own
  // serialization: `{entries: [[taskId, enqueuedEventSeq], ...], lease:
  // {lease_id, task_id} | null}`) onto buildExecBar's input contract.
  // `entryLabels` is optional — the four pre-existing tests call this with
  // only two arguments and rely on every item staying '未观测'.
  function fromQueueSnapshot(queueState, connection, entryLabels) {
    const state = queueState || {};
    const lookup = projectLabelLookup(entryLabels);
    const running = state.lease
      ? [{ taskId: state.lease.task_id, projectLabel: lookup.get(state.lease.task_id) }]
      : [];
    const queued = (state.entries || []).map(([taskId, enqueuedEventSeq]) => ({
      taskId,
      projectLabel: lookup.get(taskId),
      enqueuedEventSeq,
    }));
    return {
      connection: connection || { status: 'disconnected' },
      running,
      waitingOnMe: [],
      queued,
    };
  }

  return {
    buildExecBar,
    fromQueueSnapshot,
  };
});
