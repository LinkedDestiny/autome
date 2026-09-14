'use strict';

// Four-tier alert vocabulary (plan: 告警强化), encoded as four-fold —
// color, shape, icon, and text all agree, so the "unobserved" tier reads
// as visually distinct even at a glance, not just by color. This is the
// user's explicit ask to keep alarm semantics legible against the
// otherwise-soft island visual language.
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeStatus = api;
  }
})(function () {
  const TONES = Object.freeze(['ok', 'warning', 'critical', 'unobserved']);

  const TONE_META = Object.freeze({
    ok: { icon: '✓', dashed: false, defaultLabel: '通过' },
    warning: { icon: '!', dashed: false, defaultLabel: '注意' },
    critical: { icon: '✕', dashed: false, defaultLabel: '异常' },
    unobserved: { icon: '—', dashed: true, defaultLabel: '未观测' },
  });

  // The one hard invariant this module exists to enforce: anything not
  // explicitly observed-and-passing renders as the fourth, dashed tier —
  // never silently defaults to 'ok'. Callers must pass a tone explicitly;
  // there is no "no tone given" fallback to 'ok'.
  function statusChip(tone, label) {
    if (!TONES.includes(tone)) {
      throw new RangeError(`unknown tone "${tone}"`);
    }
    const meta = TONE_META[tone];
    return Object.freeze({
      tone,
      label: label || meta.defaultLabel,
      icon: meta.icon,
      dashed: meta.dashed,
    });
  }

  // plan.md §5.9 requirement-class severity: a missing component is never
  // worse than 'warning' unless its class is core_task_required — guards
  // against "monitored item missing" collapsing into "everything blocked".
  function missingComponentTone(requirementClassId, requirementClasses) {
    const entry = (requirementClasses || []).find((c) => c.id === requirementClassId);
    if (!entry) {
      throw new RangeError(`unknown requirement class "${requirementClassId}"`);
    }
    return entry.missingSeverity;
  }

  // A binary check (e.g. one CompletionGate field) that has not actually
  // been evaluated must chip 'unobserved', never be inferred as pass/fail
  // from the mere absence of a receipt.
  function gateChip(observed, passed, label) {
    if (!observed) {
      return statusChip('unobserved', label);
    }
    return statusChip(passed ? 'ok' : 'critical', label);
  }

  return {
    TONES,
    statusChip,
    missingComponentTone,
    gateChip,
  };
});
