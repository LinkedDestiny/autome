import { loadTaskDetail, type TaskPayload } from '../../app/components/task-detail-data.ts';
export function taskPageData(data: TaskPayload) {
  return loadTaskDetail({ gateway: {
    read: async request => ({ status: 200, body: { ok: true, payload: (request as { method: string }).method === 'task.get' ? data : {} } }),
    write: async () => { throw Error('read-only fixture'); },
  } }, data.task.id, new URL('http://localhost/tasks/' + data.task.id));
}
