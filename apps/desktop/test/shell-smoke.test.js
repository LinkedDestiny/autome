const { test } = require('node:test');
const assert = require('node:assert/strict');
const { runInNewContext } = require('node:vm');
const { readBanner, isPanelProcess } = require('../src/shell/smoke');

function page() {
  let text = '', changed, timeout, disconnected = false, cleared = false;
  const webContents = { executeJavaScript: script => runInNewContext(script, {
    document: { documentElement: {}, querySelector: () => text ? { textContent: text } : null },
    MutationObserver: class {
      constructor(callback) { changed = callback; }
      observe() {} disconnect() { disconnected = true; }
    },
    setTimeout: callback => { timeout = callback; return 1; },
    clearTimeout: () => { cleared = true; },
  }) };
  return { webContents, render(value) { text = value; changed?.(); }, expire() { timeout(); },
    get disconnected() { return disconnected; }, get cleared() { return cleared; } };
}

test('服务计数识别启动器真实标题，隔离其它 home 和内核进程', () => {
  const home = '/tmp/中文 home';
  assert.equal(isPanelProcess(`autome-web ${home} serve`, home), true);
  for (const command of [`autome-web ${home}-other serve`, `autome-web ${home} start`, `${home}/autome core`]) {
    assert.equal(isPanelProcess(command, home), false);
  }
});
test('横幅晚于 load 出现时等待，忽略中间离线状态并清理 observer', async () => {
  const p = page(); let settled = false;
  const result = readBanner(p.webContents, '内核比账本旧').then(value => { settled = true; return value; });
  await Promise.resolve(); assert.equal(settled, false);
  p.render('连接中断'); await Promise.resolve(); assert.equal(settled, false);
  p.render('内核比账本旧，请更新'); assert.match(await result, /内核比账本旧/);
  assert.equal(p.disconnected, true); assert.equal(p.cleared, true);
});
test('已存在的横幅及无需横幅的正常页面立即返回', async () => {
  const p = page(); assert.equal(await readBanner(p.webContents, ''), '');
  p.render('内核比账本旧'); assert.equal(await readBanner(p.webContents, '内核比账本旧'), '内核比账本旧');
});
test('横幅始终缺失时超时失败且清理 observer', async () => {
  const p = page(); const result = readBanner(p.webContents, '内核比账本旧');
  const rejected = assert.rejects(result, /等待横幅超时/); p.expire(); await rejected;
  assert.equal(p.disconnected, true);
});
