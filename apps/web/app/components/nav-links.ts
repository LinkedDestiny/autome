export const taskHref = (id: string) => `/tasks/${encodeURIComponent(id)}`;
export const projectHref = (id: string) => `/projects/${encodeURIComponent(id)}`;
export const newTaskHref = (projectId: string) => `${projectHref(projectId)}/tasks/new`;

export const settingsHref = (id: string) => `/loop?project=${encodeURIComponent(id)}`;
export const newProjectHref = () => '/projects/new';
export const envHref = () => '/env';
