'use strict';

// The PATH the core is given, which is not the PATH the process inherits.
//
// A GUI app launched from Finder or the Dock inherits launchd's PATH —
// `/usr/bin:/bin:/usr/sbin:/sbin` — and nothing else. The user's own PATH is
// assembled by their shell's rc files, which no GUI launch ever runs. The
// core's `env_probe` resolves `claude`, `codex`, `npm` and `brew` on PATH, so
// under the launchd default it finds `/usr/bin/git`, finds none of the others,
// and reports a fully-provisioned machine as "未安装" — with an install button
// that then refuses because `npm` is "missing" too.
//
// So Main resolves the login shell's PATH once and hands it to the core. Not a
// hardcoded list of likely directories: the sessions the core launches run in
// a terminal tab under that same login shell (launcher.rs §7), so the login
// shell's PATH is *the* answer to "what will a session actually find". Any
// other answer would make the environment panel disagree with the sessions it
// is describing.
//
// Everything here is pure except `resolveCorePath`'s one injected runner, so
// the suite exercises the parsing and merging without spawning a shell.

const { spawnSync } = require('node:child_process');

/// Wraps the shell's answer so rc-file chatter — a version notice, a greeting,
/// a `nvm` banner — cannot be mistaken for PATH. Only the text between the two
/// markers is read.
const MARKER = '__autome_path__';

/// A login shell that has not answered in this long is not going to. Three
/// seconds is generous for the ~90ms this costs on a normal machine, and the
/// fallback below is usable, so waiting longer buys nothing.
const RESOLVE_TIMEOUT_MS = 3000;

/// Used when `SHELL` is unset — the macOS default since Catalina.
const DEFAULT_SHELL = '/bin/zsh';

/// Where the install recipes in `autome_domain::environment` put things:
/// Homebrew's two prefixes and npm's user-level bin. Appended *only* when the
/// login shell could not be read at all, so that a machine we cannot ask is
/// still given the benefit of the doubt rather than told it has nothing.
const FALLBACK_DIRS = ['/opt/homebrew/bin', '/usr/local/bin', '.local/bin'];

/**
 * The PATH between the markers, or `null` if the shell did not answer.
 *
 * `null` rather than an empty string, because "the shell printed nothing
 * usable" and "the shell said PATH is empty" both mean "ask someone else".
 */
function parseLoginPath(stdout) {
  if (typeof stdout !== 'string') return null;
  const opening = stdout.indexOf(MARKER);
  if (opening < 0) return null;
  const rest = stdout.slice(opening + MARKER.length);
  const closing = rest.indexOf(MARKER);
  if (closing < 0) return null;
  const value = rest.slice(0, closing).trim();
  return value || null;
}

/**
 * The entries of every argument, in order, without duplicates.
 *
 * The login shell's PATH goes first: it is what a session will search, so the
 * binary the panel names is the binary a session will run. The inherited PATH
 * is kept behind it rather than dropped — it is where `osascript`, `git` and
 * the system tools live, and a core that could not find them would be a worse
 * bug than the one this fixes.
 */
function mergePaths(...paths) {
  const seen = new Set();
  const merged = [];
  for (const candidate of paths) {
    for (const entry of String(candidate || '').split(':')) {
      if (!entry || seen.has(entry)) continue;
      seen.add(entry);
      merged.push(entry);
    }
  }
  return merged.join(':');
}

/**
 * How to ask one shell for its PATH.
 *
 * `-ilc` because the user's PATH usually comes from `.zshrc`/`.bashrc`, which
 * a non-interactive shell does not read — that is the whole reason the GUI
 * launch is missing it. fish keeps PATH as a list rather than a colon string,
 * so it gets its own one-liner instead of a mangled answer.
 */
function shellQuery(shell) {
  const name = String(shell || '').split('/').pop();
  if (name === 'fish') {
    return { args: ['-ilc', `printf '%s' "${MARKER}"(string join : $PATH)"${MARKER}"`] };
  }
  // Braces are load-bearing: the marker is made of underscores and letters,
  // which are exactly the characters a shell accepts in a variable name, so
  // `$PATH<marker>` expands the variable `PATH__autome_path__` — empty — and
  // the answer comes back as a bare marker with no PATH in it.
  return { args: ['-ilc', `printf '%s' "${MARKER}\${PATH}${MARKER}"`] };
}

/**
 * The real runner: one login shell, bounded, with stdin closed so a shell that
 * decides to prompt gets EOF instead of hanging the app's startup.
 */
function spawnShell(shell, args) {
  const result = spawnSync(shell, args, {
    encoding: 'utf8',
    timeout: RESOLVE_TIMEOUT_MS,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  return result && typeof result.stdout === 'string' ? result.stdout : null;
}

/**
 * The PATH to give the core, with its environment injected so tests need no
 * shell.
 *
 * On Windows there is no login-shell problem to solve and no `-ilc` to solve
 * it with, so the inherited PATH is returned untouched.
 */
function resolveCorePath({
  platform = process.platform,
  shell = process.env.SHELL || DEFAULT_SHELL,
  home = process.env.HOME || '',
  inherited = process.env.PATH || '',
  run = spawnShell,
} = {}) {
  if (platform === 'win32') return inherited;

  let stdout = null;
  try {
    stdout = run(shell, shellQuery(shell).args);
  } catch {
    // A shell that is not there or cannot be spawned is the fallback case,
    // not a reason to take the app down before it has a window.
    stdout = null;
  }

  const loginPath = parseLoginPath(stdout);
  if (loginPath) return mergePaths(loginPath, inherited);

  const fallback = FALLBACK_DIRS.map((dir) =>
    dir.startsWith('/') ? dir : home ? `${home}/${dir}` : null
  ).filter(Boolean);
  return mergePaths(inherited, fallback.join(':'));
}

/**
 * The environment overlay for `AutomedSidecar`: just PATH, and only when it
 * actually differs from what the core would have inherited anyway. Returning
 * `null` in the common terminal-launch case keeps the spawn free of a
 * redundant override and makes the log line below mean something.
 */
function corePathEnv(options = {}) {
  const inherited = options.inherited || process.env.PATH || '';
  const resolved = resolveCorePath(options);
  if (!resolved || resolved === inherited) return null;
  return { PATH: resolved };
}

module.exports = {
  MARKER,
  RESOLVE_TIMEOUT_MS,
  FALLBACK_DIRS,
  parseLoginPath,
  mergePaths,
  shellQuery,
  resolveCorePath,
  corePathEnv,
};
