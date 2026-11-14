// Wait for the observable UI condition, not merely the document load event.
function readBanner(webContents, expected) {
  return webContents.executeJavaScript(`new Promise((resolve, reject) => {
    const expected = ${JSON.stringify(expected)};
    const read = () => document.querySelector('[data-testid=live-banner]')?.textContent ?? '';
    if (!expected || read().includes(expected)) { resolve(read()); return; }
    const observer = new MutationObserver(() => {
      if (read().includes(expected)) { clearTimeout(timer); observer.disconnect(); resolve(read()); }
    });
    const timer = setTimeout(() => {
      observer.disconnect(); reject(new Error('等待横幅超时：' + read()));
    }, 10000);
    observer.observe(document.documentElement, { childList: true, subtree: true, characterData: true });
  })`);
}

function isPanelProcess(command, home) {
  return command === `autome-web ${home} serve`;
}

module.exports = { readBanner, isPanelProcess };
