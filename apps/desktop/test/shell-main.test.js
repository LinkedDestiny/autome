const { test } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const { createServer } = require('../src/shell/server');
const { createWindow } = require('../src/shell/window');
const { registerIpc } = require('../src/shell/ipc');
const instance = { url: 'http://127.0.0.1:7420', pid: 123, instance: 'owned', reused: false };
function server(replies = [{ code: 0, stdout: JSON.stringify(instance) }]) {
  const calls = [];
  const api = createServer({ executable: '/electron', webRoot: '/web', core: '/core/autome', env: { PATH: '/usr/bin:/bin' }, spawnProcess(...args) {
    calls.push(args); const child = new EventEmitter(); child.kill = () => {}; child.stdout = new EventEmitter(); child.stderr = new EventEmitter();
    queueMicrotask(() => { const r = replies.shift(); if (r.pending) return; if (r.error) child.emit('error', r.error); else { child.stdout.emit('data', r.stdout ?? ''); child.stderr.emit('data', r.stderr ?? ''); child.emit('close', r.code); } });
    return child;
  } });
  return { api, calls };
}
function desktop() {
  const handlers = {}, notices = [], opened = [];
  class Window {
    constructor(options) { this.options = options; this.webContents = new EventEmitter(); this.webContents.setWindowOpenHandler = fn => { this.open = fn; }; }
    loadURL(url) { this.loaded = url; }
    isDestroyed() { return false; } isMinimized() { return true; }
    restore() { this.restored = true; } show() {} focus() { this.focused = true; }
  }
  class Notice extends EventEmitter { constructor(options) { super(); this.options = options; notices.push(this); } show() {} }
  const window = createWindow({ BrowserWindow: Window, shell: { openExternal: url => opened.push(url) }, preload: '/preload.js', url: instance.url });
  const dialog = { result: { canceled: false, filePaths: ['/目录'] }, async showOpenDialog() { return this.result; } };
  registerIpc({ ipcMain: { handle: (name, fn) => { handlers[name] = fn; } }, dialog, Notification: Notice, window, url: instance.url });
  return { handlers, notices, window, opened, dialog, Window, Notice };
}
const sender = { senderFrame: { url: instance.url + '/projects/new' } };
async function application(value, stopResult = 'stopped') {
  const d = desktop(), app = new EventEmitter(), stopped = [];
  let window, done = false;
  app.whenReady = async () => {}; app.getVersion = () => '2.0.0';
  app.quit = () => { let prevented = false; app.emit('before-quit', { preventDefault() { prevented = true; } }); if (!prevented) done = true; };
  runInNewContext(readFileSync(require.resolve('../main'), 'utf8'), {
    __dirname: '/desktop', process: { env: {}, resourcesPath: '/resources' }, console,
    URL, AbortSignal, fetch: async () => ({ json: async () => ({ web_version: '2.0.0' }) }),
    require: name => {
      if (name === 'electron') return { app, BrowserWindow: d.Window, ipcMain: { handle() {} }, dialog: d.dialog, Notification: d.Notice, shell: { openExternal() {} } };
      if (name === './src/shell/server') return { createServer: () => ({ start: async () => value, stop: async v => { stopped.push(v); return v.reused ? 'reused' : stopResult; } }) };
      if (name === './src/shell/window') return { createWindow: options => { window = createWindow(options); return window; } };
      if (name === './src/shell/ipc') return { registerIpc };
      if (name === './package.json') return { version: '2.0.0' };
      return require(name);
    },
  });
  for (let i=0; i<20 && !window; i++) await new Promise(resolve => setImmediate(resolve));
  assert.ok(window); assert.equal(window.loaded, value.url);
  app.quit();
  for (let i=0; i<20 && !done; i++) await new Promise(resolve => setImmediate(resolve));
  assert.equal(done, true);
  return { stopped, notices: d.notices };
}
test('复用服务：仅一次 start --json，窗口加载返回地址', async () => {
  const s = server([{ code: 0, stdout: JSON.stringify({ ...instance, reused: true }) }]);
  const value = await s.api.start(); const d = desktop(); d.window.loadURL(value.url);
  assert.equal(value.reused, true); assert.equal(d.window.loaded, instance.url); assert.equal(s.calls.length, 1);
  assert.deepEqual(s.calls[0][1], ['/web/bin/autome-web.js', 'start', '--json']);
  await application(value);
});
test('Electron 自带 Node 与同包内核；启动失败与无效回包拒绝', async t => {
  const s = server(); await s.api.start(); assert.equal(s.calls[0][0], '/electron');
  assert.deepEqual(s.calls[0][2].env, { PATH: '/usr/bin:/bin', ELECTRON_RUN_AS_NODE: '1', AUTOME_NODE: '/electron', AUTOME_BUNDLED_CORE: '/core/autome' });
  for (const reply of [{ code: 1, stderr: '读盘失败' }, { error: Error('不可达') }, { code: 0, stdout: '' }, { code: 0, stdout: JSON.stringify({ ...instance, url: 'https://evil.test' }) }]) await assert.rejects(server([reply]).api.start());
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const waiting = server([{ pending: true }]).api.start(); const rejected = assert.rejects(waiting, /超时/);
  t.mock.timers.tick(35000); await rejected; t.mock.timers.reset();
});
test('自有服务退出绑定 instance，无门禁正常停止', async () => {
  const s = server([{ code: 0, stdout: '面板已停止' }]); assert.equal(await s.api.stop(instance), 'stopped');
  assert.deepEqual(s.calls[0][1], ['/web/bin/autome-web.js', 'stop', '--instance', 'owned', '--wait', '0']);
  const app = await application(instance); assert.deepEqual(app.stopped, [instance]); assert.equal(app.notices.length, 0);
});
test('有门禁保留；停止失败不会误报门禁', async () => {
  assert.equal(await server([{ code: 1, stdout: '门禁还在跑，没有停止' }]).api.stop(instance), 'gates');
  await assert.rejects(server([{ code: 1, stderr: '没有响应' }]).api.stop(instance));
  const app = await application(instance, 'gates'); assert.equal(app.notices.length, 1); assert.match(app.notices[0].options.body, /面板服务留在后台/);
});
test('复用的服务退出不停止', async () => { const s = server([]); assert.equal(await s.api.stop({ ...instance, reused: true }), 'reused'); assert.equal(s.calls.length, 0); });
test('两条 IPC 均拒绝异源；目录选择与取消', async () => {
  const d = desktop();
  for (const url of ['https://evil.test', 'http://127.0.0.1:7421', 'file:///tmp/a', '']) {
    await assert.rejects(d.handlers['autome:pick-directory']({ senderFrame: { url } }));
    assert.throws(() => d.handlers['autome:notify']({ senderFrame: { url } }, {}));
  }
  assert.equal(await d.handlers['autome:pick-directory'](sender), '/目录');
  d.dialog.result.canceled = true; assert.equal(await d.handlers['autome:pick-directory'](sender), null);
});
test('通知边界与解析后的同源链接；点击聚焦导航', () => {
  const d = desktop(), notify = input => d.handlers['autome:notify'](sender, input);
  for (const input of [null, {}, { title: 1, body: '', href: null }, { title: 'a'.repeat(201), body: '', href: null }, { title: '', body: 'a'.repeat(1001), href: null }, { title: '', body: '', href: 'a'.repeat(501) }, ...['//evil.test/x', '/\\evil.test/x', 'javascript:alert(1)', 'https://evil.test', 1].map(href => ({ title: '', body: '', href }))]) assert.throws(() => notify(input));
  notify({ title: '', body: '', href: null });
  for (const href of [null, '', '/tasks/T-1', '/'+ 'a'.repeat(499)]) {
    notify({ title: 'a'.repeat(200), body: 'a'.repeat(1000), href }); d.notices.at(-1).emit('click');
    assert.equal(d.window.focused, true); assert.equal(d.window.restored, true);
    if (href !== null) assert.equal(d.window.loaded, new URL(href, instance.url).href);
  }
});
test('导航同源放行，新窗口 deny，只有 http(s) 外链交系统', () => {
  const d = desktop(); let blocked = 0;
  for (const url of [instance.url + '/tasks/T-1', 'https://evil.test', 'file:///tmp/a']) d.window.webContents.emit('will-navigate', { preventDefault: () => blocked++ }, url);
  assert.equal(blocked, 2);
  for (const url of ['https://example.com', 'file:///tmp/a', 'javascript:alert(1)']) assert.deepEqual(d.window.open({ url }), { action: 'deny' });
  assert.deepEqual(d.opened, ['https://example.com']); assert.deepEqual(d.window.options.webPreferences, { preload: '/preload.js', sandbox: true, contextIsolation: true, nodeIntegration: false });
});
test('继任服务已保留，不发门禁通知且仍携带原 instance', async () => {
  const s = server([{ code: 0, stdout: '当前运行的不是指定的那个面板服务，已保留' }]);
  assert.equal(await s.api.stop(instance), 'stopped'); assert.equal(s.calls[0][1][3], 'owned');
  const app = await application(instance); assert.equal(app.notices.length, 0);
});
