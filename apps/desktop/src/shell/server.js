const { spawn } = require('node:child_process');
const { join } = require('node:path');

function createServer({ webRoot, core, executable = process.execPath, env = process.env, spawnProcess = spawn }) {
  function run(args) {
    return new Promise((resolve, reject) => {
      const child = spawnProcess(executable, [join(webRoot, 'bin/autome-web.js'), ...args], {
        env: { ...env, ELECTRON_RUN_AS_NODE: '1', AUTOME_NODE: executable, AUTOME_BUNDLED_CORE: core },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let stdout = '', stderr = '';
      const timer = setTimeout(() => { child.kill(); reject(new Error('面板启动器超时')); }, 35000);
      child.stdout.on('data', data => { stdout += data; });
      child.stderr.on('data', data => { stderr += data; });
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('close', code => { clearTimeout(timer); resolve({ code, stdout, stderr }); });
    });
  }
  return {
    async start() {
      const result = await run(['start', '--json']);
      if (result.code !== 0) throw new Error(result.stderr || result.stdout || '面板启动失败');
      const value = JSON.parse(result.stdout.trim());
      const url = new URL(value.url);
      if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port || url.username || url.password ||
          !Number.isInteger(value.pid) || value.pid <= 1 || typeof value.instance !== 'string' || !value.instance || typeof value.reused !== 'boolean') throw new Error('面板启动器返回无效实例');
      return value;
    },
    async stop(instance) {
      if (instance.reused) return 'reused';
      const result = await run(['stop', '--instance', instance.instance, '--wait', '0']);
      if (result.code === 0) return 'stopped';
      if (result.stdout.includes('门禁还在跑')) return 'gates';
      throw new Error(result.stderr || result.stdout || '面板停止失败');
    },
  };
}
module.exports = { createServer };
