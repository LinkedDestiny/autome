const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
function load() {
  let exposed; const calls = [];
  runInNewContext(readFileSync(require.resolve('../preload'), 'utf8'), { require: () => ({ contextBridge: { exposeInMainWorld: (name, api) => { exposed = { name, api }; } }, ipcRenderer: { invoke: (...args) => { calls.push(args); return Promise.resolve('/目录'); } } }) });
  return { ...exposed, calls };
}
test('只暴露 pickDirectory 与 notify', () => { const p = load(); assert.equal(p.name, 'autome'); assert.deepEqual(Object.keys(p.api).sort(), ['notify', 'pickDirectory']); });
test('参数与 Promise 原样透传', async () => { const p = load(), input = { title: '标题', body: '正文', href: '/tasks/T-1' }; assert.equal(await p.api.pickDirectory(), '/目录'); await p.api.notify(input); assert.deepEqual(p.calls, [['autome:pick-directory'], ['autome:notify', input]]); });
