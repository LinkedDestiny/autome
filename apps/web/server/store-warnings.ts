export function filterSqliteWarning(target: Pick<NodeJS.Process, 'emitWarning'>) {
  const original = target.emitWarning.bind(target);
  target.emitWarning = ((...args: Parameters<typeof target.emitWarning>) => {
    const [warning, options] = args;
    const type = warning instanceof Error ? warning.name : typeof options === 'string' ? options : options?.type;
    const message = warning instanceof Error ? warning.message : warning;
    if (type === 'ExperimentalWarning' && message.startsWith('SQLite is an experimental feature')) return;
    Reflect.apply(original, target, args);
  }) as typeof target.emitWarning;
}
