const { app, BrowserWindow, ipcMain, dialog, Notification, shell } = require('electron');
const { join } = require('node:path');
const { createServer } = require('./src/shell/server');
const { createWindow } = require('./src/shell/window');
const { registerIpc } = require('./src/shell/ipc');
const webRoot = join(app.isPackaged ? process.resourcesPath : __dirname, 'web');
const core = app.isPackaged ? join(process.resourcesPath, 'core/autome') : join(__dirname, '../../target/debug/autome');
const server = createServer({ webRoot, core });
let instance, quitting = false, starting;
app.on('before-quit', event => {
  if (quitting) return;
  event.preventDefault();
  quitting = true;
  void (async () => {
    try {
      await starting;
      if (instance && await server.stop(instance) === 'gates') {
        new Notification({ title: '门禁还在跑', body: '面板服务留在后台；用 autome web stop 停止' }).show();
      }
    } catch (error) { console.error(error); }
    finally { app.quit(); }
  })();
});
app.on('window-all-closed', () => app.quit());
starting = app.whenReady().then(async () => {
  instance = await server.start();
  if (quitting) return;
  try {
    const response = await fetch(new URL('/api/instance', instance.url), { signal: AbortSignal.timeout(2000) });
    const info = await response.json();
    if (info.web_version !== require('./package.json').version) console.error(`面板版本：${info.web_version}；薄壳版本：${app.getVersion()}`);
  } catch (error) { console.error('读取面板版本失败：', error.message); }
  const window = createWindow({ BrowserWindow, shell, preload: join(__dirname, 'preload.js'), url: instance.url });
  registerIpc({ ipcMain, dialog, Notification, window, url: instance.url });
  await window.loadURL(instance.url);
  if (process.env.AUTOME_SHELL_SMOKE === '1') {
    const banner = await window.webContents.executeJavaScript("document.querySelector('[data-testid=live-banner]')?.textContent ?? ''");
    console.log(JSON.stringify({ url: instance.url, serverPid: instance.pid, reused: instance.reused, banner }));
    await new Promise(resolve => {
      const timer = setTimeout(resolve, 10000);
      process.stdin.once('data', () => { clearTimeout(timer); resolve(); });
      process.stdin.resume();
    });
    process.stdin.pause();
    app.quit();
  }
}).catch(error => { console.error(error); process.exitCode = 1; app.quit(); });
