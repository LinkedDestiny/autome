'use strict';

// Core design primitive (plan: 核心设计问题一). Multi-axis state — Run's
// phase x hold x terminal, the environment component's five orthogonal
// facts, Project's phase x hold — must never collapse into one badge.
// buildAxisStrip renders N axes as N cells, always; any Core-computed
// derived conclusion (e.g. readiness) is kept in a visibly separate shape
// via coreComputedConclusion so app.js cannot accidentally inline it as
// "just another axis".
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeAxis = api;
  }
})(function () {
  const UNOBSERVED = 'unobserved';

  // axes: [{ id, zh, values: [{id, zh}, ...] }, ...]
  // values: { [axisId]: observedValueId } — an axis absent from `values`
  // (or explicitly null/undefined) renders as not-yet-observed rather
  // than being silently dropped from the strip.
  function buildAxisStrip(axes, values) {
    if (!Array.isArray(axes)) {
      throw new TypeError('axes must be an array');
    }
    const observed = values || {};
    return axes.map((axis) => {
      const hasValue = Object.prototype.hasOwnProperty.call(observed, axis.id);
      const observedId = hasValue ? observed[axis.id] : null;
      if (observedId === null || observedId === undefined) {
        return Object.freeze({
          axisId: axis.id,
          axisZh: axis.zh,
          valueId: UNOBSERVED,
          valueZh: '未观测',
          observed: false,
        });
      }
      const match = (axis.values || []).find((v) => v.id === observedId);
      if (!match) {
        throw new RangeError(`axis "${axis.id}" has no value "${observedId}"`);
      }
      return Object.freeze({
        axisId: axis.id,
        axisZh: axis.zh,
        valueId: match.id,
        valueZh: match.zh,
        observed: true,
      });
    });
  }

  // A derived conclusion (Run readiness, etc.) computed by Core from the
  // axes above, never read directly off any single axis. Kept structurally
  // distinct (`kind: 'core-computed'`) from an axis-strip cell so renderers
  // are forced to label it separately.
  function coreComputedConclusion(id, zh, valueId, valueZh) {
    const isObserved = valueId !== null && valueId !== undefined;
    return Object.freeze({
      kind: 'core-computed',
      id,
      zh,
      valueId: isObserved ? valueId : UNOBSERVED,
      valueZh: isObserved ? valueZh : '未观测',
      observed: isObserved,
    });
  }

  return {
    UNOBSERVED,
    buildAxisStrip,
    coreComputedConclusion,
  };
});
