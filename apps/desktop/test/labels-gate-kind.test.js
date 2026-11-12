'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const source = fs.readFileSync(path.join(__dirname, '../renderer/lib/labels.js'), 'utf8');
const labels = import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
for (const [name, node, kind, label] of [
  ['需求快检', 'gate', 'quick', '快检'],
  ['FixGate 快检', 'fix_gate', 'quick', '修复门禁（复现 + 快检）'],
]) {
  test(name, async () => {
    const { gateTagLabel } = await labels;
    for (const ok of [true, false]) {
      for (const sha of ['', 'a', '123456789abcdef']) {
        assert.equal(gateTagLabel({ node, kind, ok, sha }), `${label}${ok ? '通过' : '未通过'} · ${sha.slice(0, 7)}`);
      }
    }
  });
}
test('完整、复现、未知与无 kind 保持旧标签', async () => {
  const { gateTagLabel, nodeLabel } = await labels;
  for (const kind of [undefined, 'full', 'repro', 'unknown']) {
    for (const node of ['gate', 'fix_gate', 'merge_gate', 'main_check']) {
      for (const ok of [true, false]) {
        assert.equal(gateTagLabel({ node, kind, ok, sha: '123456789' }), `${nodeLabel(node)}${ok ? '通过' : '未通过'} · 1234567`);
      }
    }
  }
});
