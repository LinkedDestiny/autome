import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

const web = resolve(import.meta.dirname, '../..');
export function ensureBuilt() {
  // Nested vitest inherits the marker: it must not rewrite a running suite's build.
  if (process.env.AUTOME_WEB_TEST_BUILD === web) return;
  execFileSync('npm', ['run', 'build'], { cwd: web, stdio: 'pipe', timeout: 60_000 });
  process.env.AUTOME_WEB_TEST_BUILD = web;
}
