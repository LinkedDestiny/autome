'use strict';

// The PATH handed to the core.
//
// The bug these pin: the packaged app, launched from Finder, ran the core with
// launchd's `/usr/bin:/bin:/usr/sbin:/sbin`. `git` lives in `/usr/bin` and was
// found; `claude` (~/.local/bin) and `codex` (/opt/homebrew/bin) were not, and
// the environment panel told a correctly-installed machine that both CLIs were
// missing. Nothing about the core was wrong — it searched exactly the PATH it
// was given.

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  MARKER,
  parseLoginPath,
  mergePaths,
  shellQuery,
  resolveCorePath,
  corePathEnv,
} = require('../src/login-path');

/** The launchd PATH a Finder launch actually gets. */
const GUI_PATH = '/usr/bin:/bin:/usr/sbin:/sbin';
/** This machine's real PATH, abbreviated. */
const USER_PATH = '/Users/d/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin';

const answering = (pathValue) => () => `${MARKER}${pathValue}${MARKER}`;

test('the shell answer is read from between the markers, not from its chatter', () => {
  const stdout = `nvm: using v20\n${MARKER}${USER_PATH}${MARKER}`;
  assert.equal(parseLoginPath(stdout), USER_PATH);
});

test('a shell that printed no markers has not answered', () => {
  assert.equal(parseLoginPath('welcome to zsh\n'), null, 'rc chatter alone is not a PATH');
  assert.equal(parseLoginPath(`${MARKER}${USER_PATH}`), null, 'a truncated answer is no answer');
  assert.equal(parseLoginPath(`${MARKER}   ${MARKER}`), null, 'a blank PATH is no answer');
  assert.equal(parseLoginPath(null), null);
});

test('the login shell leads and the inherited entries follow, without duplicates', () => {
  // Order is the point: a session runs under the login shell, so the binary
  // the panel names must be the one that shell would run.
  const merged = mergePaths(USER_PATH, GUI_PATH);
  assert.equal(merged, `${USER_PATH}:/usr/sbin:/sbin`);
  assert.equal(merged.split(':').length, new Set(merged.split(':')).size);
});

test('empty segments never survive the merge', () => {
  // An empty PATH entry means "the current directory" to a POSIX exec, which
  // is not a place a daemon should look for `claude`.
  assert.equal(mergePaths('::/a:', '/a::/b'), '/a:/b');
  assert.equal(mergePaths('', null), '');
});

test('the query asks an interactive login shell, because that is where PATH is set', () => {
  // `.zshrc` — where a user's PATH usually comes from — is not read by a
  // non-interactive shell, which is the entire reason the GUI launch is short.
  const { args } = shellQuery('/bin/zsh');
  assert.deepEqual(args.slice(0, 1), ['-ilc']);
  assert.ok(args[1].includes(MARKER));
  // Braced, and the marker must not touch the `$`: underscores are legal in a
  // variable name, so `$PATH__autome_path__` expands to nothing and the shell
  // answers with an empty PATH between two markers. Cost the first version of
  // this file a silent fall back to the guessed directories.
  assert.ok(args[1].includes('${PATH}'), args[1]);
  assert.ok(!/\$PATH[A-Za-z0-9_]/.test(args[1]), args[1]);
});

test('fish is asked in its own language, since its PATH is a list', () => {
  const { args } = shellQuery('/opt/homebrew/bin/fish');
  assert.ok(args[1].includes('string join : $PATH'), args[1]);
});

test('a Finder launch is given the login shell PATH, so the CLIs are findable', () => {
  const resolved = resolveCorePath({
    platform: 'darwin',
    shell: '/bin/zsh',
    inherited: GUI_PATH,
    run: answering(USER_PATH),
  });
  for (const dir of ['/Users/d/.local/bin', '/opt/homebrew/bin']) {
    assert.ok(resolved.split(':').includes(dir), `${dir} must be searchable: ${resolved}`);
  }
  assert.ok(resolved.split(':').includes('/usr/bin'), 'the system tools stay searchable');
});

test('the shell is asked which shell it is, and with the right argv', () => {
  const calls = [];
  resolveCorePath({
    platform: 'darwin',
    shell: '/bin/bash',
    inherited: GUI_PATH,
    run: (shell, args) => {
      calls.push([shell, args]);
      return `${MARKER}${USER_PATH}${MARKER}`;
    },
  });
  assert.equal(calls.length, 1, 'one shell per resolution, not one per PATH entry');
  assert.equal(calls[0][0], '/bin/bash');
});

test('a shell that cannot be asked still leaves the install locations searchable', () => {
  // We cannot know this machine's PATH, but we do know where the install
  // recipes put things. Better than repeating the original bug in silence.
  const throwing = resolveCorePath({
    platform: 'darwin',
    shell: '/bin/zsh',
    home: '/Users/d',
    inherited: GUI_PATH,
    run: () => {
      throw new Error('ENOENT');
    },
  });
  for (const dir of ['/opt/homebrew/bin', '/usr/local/bin', '/Users/d/.local/bin']) {
    assert.ok(throwing.split(':').includes(dir), `${dir} missing from ${throwing}`);
  }
  assert.ok(throwing.startsWith(GUI_PATH), 'the inherited PATH still wins over a guess');

  const silent = resolveCorePath({
    platform: 'darwin',
    home: '/Users/d',
    inherited: GUI_PATH,
    run: () => 'command not found\n',
  });
  assert.equal(silent, throwing, 'a silent shell and a missing one are the same case');
});

test('a fallback without a home directory invents no path', () => {
  const resolved = resolveCorePath({
    platform: 'darwin',
    home: '',
    inherited: GUI_PATH,
    run: () => null,
  });
  assert.ok(!resolved.includes('.local/bin'), resolved);
  assert.ok(!resolved.includes('undefined'), resolved);
});

test('Windows has no login-shell problem and is left alone', () => {
  const resolved = resolveCorePath({
    platform: 'win32',
    inherited: 'C:\\Windows',
    run: () => {
      throw new Error('no shell should be spawned on win32');
    },
  });
  assert.equal(resolved, 'C:\\Windows');
});

test('a terminal launch gets no overlay at all', () => {
  // Started from a shell, the inherited PATH is already the right one; an
  // identical override would be noise in the spawn and in the log.
  const env = corePathEnv({
    platform: 'darwin',
    inherited: USER_PATH,
    run: answering(USER_PATH),
  });
  assert.equal(env, null);
});

test('a Finder launch gets an overlay carrying only PATH', () => {
  const env = corePathEnv({
    platform: 'darwin',
    inherited: GUI_PATH,
    run: answering(USER_PATH),
  });
  assert.deepEqual(Object.keys(env), ['PATH']);
  assert.notEqual(env.PATH, GUI_PATH);
});
