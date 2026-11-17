export const idleProject = (done = 0) => done ? done + ' 个已完成' : '没有任务在跑';
export const projectColor = (id: string) => ['blue', 'teal', 'orange', 'purple', 'green', 'pink', 'yellow', 'red', 'brown'][Array.from(id).reduce((hash, ch) => (hash * 31 + ch.charCodeAt(0)) >>> 0, 0) % 9];
