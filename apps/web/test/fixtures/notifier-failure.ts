import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// The notification fixture needs a terminal failure, not a delayed retry.
export function configureFailureRepo(repo: string) {
  mkdirSync(join(repo, '.autome'), { recursive: true });
  writeFileSync(join(repo, '.autome/config.toml'), '[retry]\nattempts = 0\nbackoff_seconds = 0\n');
}
