import { checkNodeVersion, MIN_NODE_VERSION } from './node-version.js';

if (!checkNodeVersion(process.versions.node)) {
  console.error(`Autome Web 需要 Node.js ${MIN_NODE_VERSION} 或更高版本，当前是 ${process.versions.node}。请升级 Node.js 后重试。`);
  process.exit(1);
}

const { runFromEnv } = await import('./main.ts');
await runFromEnv();
