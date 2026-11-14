const { sameOrigin } = require('./window');
function registerIpc({ ipcMain, dialog, Notification, window, url }) {
  const origin = new URL(url).origin;
  function authorize(event) {
    if (!sameOrigin(event.senderFrame?.url, origin)) throw new Error('拒绝非面板来源');
  }
  ipcMain.handle('autome:pick-directory', async event => {
    authorize(event);
    const result = await dialog.showOpenDialog(window, { properties: ['openDirectory'] });
    return result.canceled ? null : result.filePaths[0] ?? null;
  });
  ipcMain.handle('autome:notify', (event, input) => {
    authorize(event);
    if (!input || typeof input.title !== 'string' || input.title.length > 200 ||
        typeof input.body !== 'string' || input.body.length > 1000 ||
        (input.href !== null && (typeof input.href !== 'string' || input.href.length > 500))) throw new Error('通知参数无效');
    const target = input.href === null ? null : new URL(input.href, origin);
    if (target && target.origin !== origin) throw new Error('拒绝跨源通知链接');
    const notification = new Notification({ title: input.title, body: input.body });
    notification.on('click', () => {
      if (window.isDestroyed()) return;
      if (window.isMinimized()) window.restore();
      window.show(); window.focus();
      if (target) void window.loadURL(target.href);
    });
    notification.show();
  });
}
module.exports = { registerIpc };
