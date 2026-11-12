'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

test('gate_same_failure shows the count, reason and log', async () => {
  const source = fs.readFileSync(path.join(__dirname, '../renderer/lib/labels.js'), 'utf8');
  const { failureReason } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  for (const count of [1, 3, 10]) {
    assert.equal(
      failureReason({ kind: 'gate_same_failure', count, summary: 'S', log: '/l' }),
      `门禁连续 ${count} 次因同一原因失败：S · 日志 /l`,
    );
  }
});
