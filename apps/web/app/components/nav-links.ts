export const taskHref = (id: string) => `/tasks/${encodeURIComponent(id)}`;
export const projectHref = (id: string) => `/projects/${encodeURIComponent(id)}`;
export const newTaskHref = (projectId: string) => `${projectHref(projectId)}/tasks/new`;
