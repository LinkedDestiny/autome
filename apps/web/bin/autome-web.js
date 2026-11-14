import { checkNodeVersion, MIN_NODE_VERSION } from '../server/node-version.js';
if (!checkNodeVersion(process.versions.node)) {
  console.error(`Autome Web 需要 Node.js ${MIN_NODE_VERSION} 或更高版本，当前是 ${process.versions.node}。请升级 Node.js 后重试。`);
  process.exit(1);
}
(await import('../server/store-warnings.ts')).filterSqliteWarning(process);
try { process.exitCode = await (await import('../server/launcher.ts')).launch(process.argv.slice(2)); }
catch (error) { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; }
