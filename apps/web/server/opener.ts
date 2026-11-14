import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
export async function openPanel(url: string, { platform = process.platform, executable = process.env.AUTOME_WEB_OPENER, log = console.log } = {}) {
  if (executable || platform === 'darwin') await promisify(execFile)(executable ?? '/usr/bin/open', [url]);
  else log(`请在浏览器里打开 ${url}`);
}
