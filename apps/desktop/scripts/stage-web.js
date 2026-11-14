const { rmSync } = require('node:fs');
const { resolve } = require('node:path');
const { execFileSync } = require('node:child_process');
const output = resolve(__dirname, '../web');
rmSync(output, { recursive: true, force: true });
execFileSync(process.execPath, [resolve(__dirname, '../../web/scripts/stage-panel.ts'), output], { stdio: 'inherit' });
