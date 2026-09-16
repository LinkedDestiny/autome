'use strict';

// The preload bridge. Technical design §15: the preload exposes no generic
// IPC, no Node, no `process`, no filesystem and no shell — only named
// functions, one per method the gates allow.
//
// The discipline that matters is the absence of `invoke(method, params)`. A
// generic passthrough would make the allowlists in src/ipc-gate.js and
// src/write-gate.js advisory: anything the renderer could name, it could call.
// With named functions, adding a capability means editing three files on
// purpose.
//
// Reads never mutate; writes always do. They are two channels with two
// independent allowlists, and nothing here merges them.

const { contextBridge, ipcRenderer } = require('electron');

function read(method) {
  return (params) => ipcRenderer.invoke('autome:read', { method, params: params || {} });
}

function write(op) {
  return (params) => ipcRenderer.invoke('autome:write', { op, params: params || {} });
}

contextBridge.exposeInMainWorld('autome', {
  read: {
    dashboard: () => read('dashboard.get')(),
    listProjects: () => read('project.list')(),
    getProject: (projectId) => read('project.get')({ project_id: projectId }),
    getTask: (taskId) => read('task.get')({ task_id: taskId }),
    taskChanges: (taskId) => read('task.changes')({ task_id: taskId }),
    sessionLog: (sessionId) => read('session.log')({ session_id: sessionId }),
    // `projectId` is optional: omitted means the global scope.
    getConfig: (projectId) => read('config.get')(projectId ? { project_id: projectId } : {}),
    validateConfig: (projectId) =>
      read('config.validate')(projectId ? { project_id: projectId } : {}),
    listSkills: (projectId) => read('skills.list')(projectId ? { project_id: projectId } : {}),
    onboardingArtefacts: (projectId) =>
      read('project.onboarding.artefacts')({ project_id: projectId }),
    environment: () => read('env.get')(),
    installRecipe: (component) => read('env.install_recipe')({ component }),
    eventsSince: (afterSeq) => read('events.since')({ after_seq: afterSeq }),
  },

  write: {
    // Main owns the directory picker; the renderer never names a path.
    pickProject: () => write('project.pick')(),
    removeProject: (projectId) => write('project.remove')({ project_id: projectId }),
    advanceOnboarding: (projectId) =>
      write('project.onboarding.advance')({ project_id: projectId }),
    skipOnboarding: (projectId) => write('project.onboarding.skip')({ project_id: projectId }),
    runOnboarding: (projectId) => write('project.onboarding.run')({ project_id: projectId }),
    saveOnboardingFile: (projectId, path, content) =>
      write('project.onboarding.save')({ project_id: projectId, path, content }),

    createTask: ({ projectId, request, attachments, docRefs }) =>
      write('task.create')({
        project_id: projectId,
        request,
        attachments: attachments || [],
        doc_refs: docRefs || [],
      }),
    approveTask: (taskId) => write('task.approve')({ task_id: taskId }),
    rejectTask: (taskId, feedback) => write('task.reject')({ task_id: taskId, feedback }),
    mergeTask: (taskId) => write('task.merge')({ task_id: taskId }),
    pauseTask: (taskId) => write('task.pause')({ task_id: taskId }),
    resumeTask: (taskId) => write('task.resume')({ task_id: taskId }),
    stopTask: (taskId) => write('task.stop')({ task_id: taskId }),
    cancelTask: (taskId) => write('task.cancel')({ task_id: taskId }),
    decide: ({ taskId, kind, itemId, disposition, ruling }) =>
      write('task.decide')({
        task_id: taskId,
        kind,
        item_id: itemId,
        disposition,
        ruling: ruling || null,
      }),
    extendBudget: (taskId, extraRounds) =>
      write('task.extend_budget')({ task_id: taskId, extra_rounds: extraRounds }),
    rerunFrom: (taskId, node) => write('task.rerun_from')({ task_id: taskId, node }),
    archiveTask: (taskId) => write('task.archive')({ task_id: taskId }),
    restoreTask: (taskId) => write('task.restore')({ task_id: taskId }),

    setRole: ({ projectId, role, enabled, runtime, model, effort, skills }) => {
      const params = { role };
      if (projectId) params.project_id = projectId;
      if (enabled !== undefined) params.enabled = enabled;
      if (runtime !== undefined) params.runtime = runtime;
      if (model !== undefined) params.model = model;
      // `null` is meaningful: "use the CLI default", distinct from absent.
      if (effort !== undefined) params.effort = effort;
      if (skills !== undefined) params.skills = skills;
      return write('config.set_role')(params);
    },
    resetRole: (projectId, role) => write('config.reset_role')({ project_id: projectId, role }),
    setLoop: ({ projectId, parallel, designRounds, budgetFactor }) => {
      const params = {};
      if (projectId) params.project_id = projectId;
      if (parallel !== undefined) params.parallel = parallel;
      if (designRounds !== undefined) params.design_rounds = designRounds;
      if (budgetFactor !== undefined) params.budget_factor = budgetFactor;
      return write('config.set_loop')(params);
    },

    // `force` is E-04's button: the user just changed something and is
    // waiting to see it, so it skips the core's probe rate limit.
    setTheme: (theme) => write('config.set_theme')({ theme }),

    detectEnvironment: (force) => write('env.detect')({ force: Boolean(force) }),
    install: (component) => write('env.install')({ component }),
    login: (component) => write('env.login')({ component }),

    tick: () => write('scheduler.tick')(),

    // Opening things is by identifier, never by path: Main resolves the
    // path from the core's own answer.
    open: ({ kind, projectId, taskId, sessionId, name }) =>
      write('open.path')({
        kind,
        project_id: projectId,
        task_id: taskId,
        session_id: sessionId,
        name,
      }),
    openTerminal: (taskId) => write('open.terminal')({ task_id: taskId }),
  },

  // Core-pushed events. The renderer subscribes once and re-reads what it
  // needs; it never receives business state through this channel, only the
  // fact that something changed.
  onEvent: (handler) => {
    const listener = (_event, payload) => handler(payload);
    ipcRenderer.on('autome:event', listener);
    return () => ipcRenderer.removeListener('autome:event', listener);
  },

  onCoreStatus: (handler) => {
    const listener = (_event, payload) => handler(payload);
    ipcRenderer.on('autome:core-status', listener);
    return () => ipcRenderer.removeListener('autome:core-status', listener);
  },
});
