function sameOrigin(url, origin) {
  try { return new URL(url).origin === origin; } catch { return false; }
}
function createWindow({ BrowserWindow, shell, preload, url }) {
  const origin = new URL(url).origin;
  const window = new BrowserWindow({ width: 1512, height: 944, webPreferences: {
    preload, sandbox: true, contextIsolation: true, nodeIntegration: false,
  } });
  window.webContents.on('will-navigate', (event, target) => {
    if (!sameOrigin(target, origin)) event.preventDefault();
  });
  window.webContents.setWindowOpenHandler(({ url: target }) => {
    try { if (['http:', 'https:'].includes(new URL(target).protocol)) void shell.openExternal(target); } catch { /* Invalid targets stay blocked. */ }
    return { action: 'deny' };
  });
  return window;
}
module.exports = { createWindow, sameOrigin };
