'use strict';

// Plan §9.4: "preload 不暴露通用 IPC、Node、process、文件、shell 或 URL 打开
// 能力". This exposes exactly five named, read-only functions — one per
// method main.js's `autome:read` handler allows (src/ipc-gate.js is the
// single source of truth for that allowlist) — never a generic
// `invoke(method, params)` passthrough, and never a write method. A
// renderer with this bridge can ask "what is the state of X" and nothing
// else: no shell, no URL, no argv (§5.9).

const { contextBridge, ipcRenderer } = require('electron');

function readMethod(method) {
  return (params) => ipcRenderer.invoke('autome:read', { method, params: params || {} });
}

contextBridge.exposeInMainWorld('automeRead', {
  listProjects: readMethod('project.list'),
  getProject: (projectId) => readMethod('project.get')({ aggregate_id: projectId }),
  listTasks: (projectId) => readMethod('task.list')({ project_id: projectId }),
  getTask: (projectId, taskId) => readMethod('task.get')({ project_id: projectId, task_id: taskId }),
  getQueue: readMethod('queue.get'),
});

// §8.2's write path. Exactly two named functions, never a generic
// invoke(op, params) passthrough — same discipline as automeRead above,
// just for the write channel. Neither function ever carries a filesystem
// path: `pickProjectTarget` only sends the two-value `kind` enum (Main
// owns the actual `dialog.showOpenDialog` call and only ever hands back
// an already-persisted `target_id` + display summary); `createProject`
// only sends that `target_id` plus scalars.
function writeOp(op) {
  return (params) => ipcRenderer.invoke('autome:write', { op, params: params || {} });
}

contextBridge.exposeInMainWorld('automeWrite', {
  pickProjectTarget: (kind) => writeOp('project.pick_target')({ kind }),
  createProject: ({ targetId, displayName, trustConfirmed, destinationName }) =>
    writeOp('project.create_from_target')({
      target_id: targetId,
      display_name: displayName,
      trust_confirmed: trustConfirmed,
      destination_name: destinationName === undefined ? null : destinationName,
    }),
});
