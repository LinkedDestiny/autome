const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('autome', {
  pickDirectory: () => ipcRenderer.invoke('autome:pick-directory'),
  notify: input => ipcRenderer.invoke('autome:notify', input),
});
