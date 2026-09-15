//! Local environment probing: the four components of technical design §11
//! ("环境探测与安装"), observed from the real machine and reported as an
//! `autome_domain::environment::Environment`.
//!
//! §11 asks for two facts per component and nothing more: is it there, and
//! — for the two CLIs — is it logged in. The domain crate already encodes
//! that shape; this module is only the part that has to touch the file
//! system and spawn processes, so the discipline here is about keeping the
//! touching thin and the deciding pure:
//!
//! * every decision (which PATH entry wins, which token is the version,
//!   what a login answer means, what a plist says) is a free function over
//!   `&str`/predicates, testable without any of the four tools installed;
//! * the impure shell around them is three functions — look a name up on
//!   PATH, run a command with a deadline, stat a directory.
//!
//! Two rules from the design shape the CLI probes in particular.
//!
//! §7.2 ("CLI 参数映射"): the CLIs' own flags "按 CLI 版本可能变化，放在
//! adapter 表中，不硬编码在状态机里". The login probe is exactly such a
//! flag, so the argv forms live in the `LOGIN_PROBES` tables below. When a
//! CLI renames its status subcommand, that table is the one place to edit.
//!
//! §11 plus `Login::Unknown`'s own doc comment: a probe that cannot get an
//! answer must say `Unknown`, never `Expired`. `Expired` sends the user to
//! re-authenticate; if the real cause is that `claude auth status` was
//! renamed, re-authenticating fixes nothing and the user is left chasing a
//! phantom. So the classifier only returns `Expired` on an explicit
//! logged-out/expired marker, and everything else — unknown subcommand,
//! empty output, a spawn failure, a timeout — degrades to `Unknown` with
//! the argv forms we tried recorded in the detail string.
//!
//! No call here can panic or hang: every subprocess runs under a 5 second
//! deadline and is killed and reaped when it overruns (§17's accepted-risk
//! register covers the CLIs themselves; it does not extend to the daemon
//! blocking on one).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use autome_domain::environment::{Component, ComponentStatus, Environment, Login};

/// Deadline for every subprocess this module starts. One value, not one per
/// call site: the probe runs on app start, on foreground, and after an
/// install terminal closes (§11), and a user staring at a spinner cannot
/// tell which of the four is slow — bounding them identically keeps the
/// worst case arithmetic ("table length × this") obvious.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// How often the deadline loop checks on the child. Small enough that a
/// fast `--version` (single-digit milliseconds) is not visibly delayed,
/// large enough not to spin a core while waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// §11: iTerm2 is detected by the presence of the bundle, not by a binary
/// on PATH — it is a GUI app and there is nothing on PATH to find.
const ITERM_APP_PATH: &str = "/Applications/iTerm.app";

/// The plist key Apple documents as the user-visible version ("3.5.11"),
/// as opposed to `CFBundleVersion`, which is the build number and is what
/// a naive scan of the file finds first.
const ITERM_VERSION_KEY: &str = "CFBundleShortVersionString";

// ---------------------------------------------------------------------------
// §7.2 adapter tables
// ---------------------------------------------------------------------------

/// One candidate way to ask a CLI whether it is logged in.
///
/// `note` exists so that a future maintainer reading the table knows why a
/// form is in it and where it came from, rather than deleting a form that
/// looks redundant but covers an older release still in the wild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginProbe {
    pub args: &'static [&'static str],
    pub note: &'static str,
}

/// Claude Code login probes, most likely first.
///
/// Deliberately absent: any print-mode form (`claude -p "..."`,
/// `claude --print /status`). Print mode starts a real, billable session
/// and would turn a background environment probe into inference spend; a
/// probe that costs money is worse than an honest `Login::Unknown`.
const CLAUDE_LOGIN_PROBES: &[LoginProbe] = &[
    LoginProbe {
        args: &["auth", "status"],
        note: "the form the CLI documents today",
    },
    LoginProbe {
        args: &["login", "status"],
        note: "mirrors Codex's shape; covers a rename in either direction",
    },
    LoginProbe {
        args: &["auth", "whoami"],
        note: "some releases expose the account under whoami instead",
    },
];

/// Codex login probes, most likely first. `codex login status` is the form
/// the CLI has shipped; the rest are here for the same rename tolerance.
const CODEX_LOGIN_PROBES: &[LoginProbe] = &[
    LoginProbe {
        args: &["login", "status"],
        note: "the form the CLI documents today",
    },
    LoginProbe {
        args: &["auth", "status"],
        note: "mirrors Claude's shape; covers a rename in either direction",
    },
    LoginProbe {
        args: &["whoami"],
        note: "top-level fallback seen in some builds",
    },
];

/// The table for a component, empty for the two that have no login (§11:
/// Git and iTerm2 are "不适用").
pub fn login_probes(component: Component) -> &'static [LoginProbe] {
    match component {
        Component::Claude => CLAUDE_LOGIN_PROBES,
        Component::Codex => CODEX_LOGIN_PROBES,
        _ => &[],
    }
}

/// Markers that mean "there is a session and it is usable". Checked *after*
/// the negative markers, because "Not logged in" contains "logged in".
const LOGGED_IN_MARKERS: &[&str] = &[
    "logged in",
    "signed in",
    "authenticated",
    "auth: ok",
    "status: ok",
    "credentials found",
    "已登录",
];

/// Markers that mean "there is no usable session". The user's fix is to run
/// `login_command()` (§11), so these — and only these — map to `Expired`.
const LOGGED_OUT_MARKERS: &[&str] = &[
    "not logged in",
    "not signed in",
    "not authenticated",
    "logged out",
    "signed out",
    "expired",
    "login required",
    "please log in",
    "please login",
    "please run",
    "no credentials",
    "no stored credentials",
    "unauthorized",
    "invalid api key",
    "未登录",
];

/// Markers that mean "the CLI did not understand the question". These are
/// the §7.2 case: the flag moved. They must never read as logged out.
const UNRECOGNISED_MARKERS: &[&str] = &[
    "unknown command",
    "unknown subcommand",
    "no such subcommand",
    "unrecognized",
    "unrecognised",
    "unexpected argument",
    "invalid subcommand",
    "invalid value",
    "usage:",
    "did you mean",
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Probes all four components (§11). One `checked_at` for the whole sweep,
/// so the UI can say "as of HH:MM" without four slightly different times.
pub fn probe_all() -> Environment {
    let path_var = std::env::var("PATH").unwrap_or_default();
    probe_all_with(&path_var, Path::new(ITERM_APP_PATH), &now_rfc3339())
}

/// Probes one component against the real machine.
pub fn probe(component: Component) -> ComponentStatus {
    let now = now_rfc3339();
    let path_var = std::env::var("PATH").unwrap_or_default();
    probe_with(component, &path_var, Path::new(ITERM_APP_PATH), &now)
}

/// Whether an install recipe's prerequisite (`InstallRecipe::prerequisite`,
/// i.e. `brew` or `npm`) is available. §11: "Homebrew 或 npm 缺失时先给出
/// 它们的安装命令" — the caller needs this answer before it offers to run
/// the recipe. Kept generic over the name rather than an enum of two, since
/// the recipes are data in the domain crate and may grow a third.
pub fn has_prerequisite(name: &str) -> bool {
    let path_var = std::env::var("PATH").unwrap_or_default();
    find_on_path(&path_var, name, is_executable_file).is_some()
}

// ---------------------------------------------------------------------------
// Probes, with their environment injected so tests need none of the tools
// ---------------------------------------------------------------------------

/// The seam `probe_all` is a two-line wrapper over: the same sweep, but
/// told what PATH is, where the iTerm2 bundle would be, and what time it
/// is. Tests drive this, so the suite never depends on which of the four
/// tools the machine running it happens to have.
fn probe_all_with(path_var: &str, iterm_app: &Path, now: &str) -> Environment {
    Environment {
        components: Component::ALL
            .into_iter()
            .map(|c| probe_with(c, path_var, iterm_app, now))
            .collect(),
        checked_at: now.to_string(),
    }
}

/// The seam every test uses: the same logic as `probe`, but told what PATH
/// is and where the iTerm2 bundle would be.
fn probe_with(
    component: Component,
    path_var: &str,
    iterm_app: &Path,
    now: &str,
) -> ComponentStatus {
    match component {
        Component::Git => probe_binary(Component::Git, "git", path_var, now),
        Component::Claude => probe_binary(Component::Claude, "claude", path_var, now),
        Component::Codex => probe_binary(Component::Codex, "codex", path_var, now),
        Component::ITerm2 => probe_iterm2(iterm_app, now),
    }
}

/// §11's first two columns for a PATH-resident tool: run `--version`, keep
/// the path. Then, for the two CLIs, walk the §7.2 login table.
///
/// A binary that is not on PATH yields `ComponentStatus::missing`, whose
/// login is `NotApplicable` rather than `Unknown`: with no binary there is
/// no login question to be uncertain about, and a UI showing "登录态未知"
/// next to "未安装" is noise on top of the real problem.
fn probe_binary(component: Component, binary: &str, path_var: &str, now: &str) -> ComponentStatus {
    let Some(binary_path) = find_on_path(path_var, binary, is_executable_file) else {
        return ComponentStatus::missing(component, now);
    };

    let version = match run_capture(&binary_path, &["--version"], PROBE_TIMEOUT) {
        RunOutcome::Completed(capture) => extract_version(&capture.stdout, &capture.stderr),
        // The binary exists and is executable; a failed or slow `--version`
        // does not make it absent, it makes its version unknown. Reporting
        // `present: false` here would send the user to reinstall a tool
        // that is sitting right there on PATH.
        RunOutcome::TimedOut | RunOutcome::Failed(_) => None,
    };

    ComponentStatus {
        component,
        present: true,
        version,
        path: Some(binary_path.to_string_lossy().into_owned()),
        login: probe_login(component, &binary_path),
        checked_at: now.to_string(),
    }
}

/// §11's iTerm2 row: the bundle's existence and its version. No process is
/// spawned — opening the app to ask it its version would be absurd, and
/// `Info.plist` is right there.
fn probe_iterm2(app_dir: &Path, now: &str) -> ComponentStatus {
    if !app_dir.is_dir() {
        return ComponentStatus::missing(Component::ITerm2, now);
    }
    let plist = std::fs::read_to_string(app_dir.join("Contents/Info.plist")).unwrap_or_default();
    ComponentStatus {
        component: Component::ITerm2,
        present: true,
        version: plist_string_value(&plist, ITERM_VERSION_KEY),
        path: Some(app_dir.to_string_lossy().into_owned()),
        login: Login::NotApplicable,
        checked_at: now.to_string(),
    }
}

/// Walks the §7.2 table until one form answers. The first `LoggedIn` or
/// `Expired` wins; forms that are not understood, fail to spawn or overrun
/// the deadline are skipped, and if the table runs out the result is
/// `Unknown` naming every form tried — that string is the bug report.
fn probe_login(component: Component, binary_path: &Path) -> Login {
    let probes = login_probes(component);
    if probes.is_empty() {
        return Login::NotApplicable;
    }

    let mut tried: Vec<String> = Vec::new();
    for probe in probes {
        let rendered = format!(
            "{} {}",
            binary_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| binary_path.to_string_lossy().into_owned()),
            probe.args.join(" ")
        );
        match run_capture(binary_path, probe.args, PROBE_TIMEOUT) {
            RunOutcome::Completed(capture) => {
                match classify_login(&capture.stdout, &capture.stderr, capture.exit_code) {
                    LoginVerdict::LoggedIn { account_hint } => return Login::Ok { account_hint },
                    LoginVerdict::LoggedOut => return Login::Expired,
                    LoginVerdict::Unrecognised => tried.push(rendered),
                }
            }
            RunOutcome::TimedOut => tried.push(format!("{rendered} (超时)")),
            RunOutcome::Failed(err) => tried.push(format!("{rendered} ({err})")),
        }
    }

    Login::Unknown {
        detail: format!(
            "无法判定登录态，已尝试：{}。CLI 参数可能已变更，请更新 env_probe 的 LOGIN_PROBES 表（§7.2）。",
            tried.join("; ")
        ),
    }
}

// ---------------------------------------------------------------------------
// Pure part: PATH scanning
// ---------------------------------------------------------------------------

/// First entry of `path_var` holding an executable called `binary`.
///
/// `is_executable` is a parameter so the whole search is testable against a
/// synthetic PATH and a synthetic file system. Two deliberate choices:
///
/// * empty segments are skipped. POSIX says an empty PATH entry means the
///   current directory; resolving a daemon's tooling relative to whatever
///   directory it happens to be in is a foot-gun, and no user ever meant
///   it by typing `PATH=$PATH:`.
/// * first match wins, matching what the shell would have run. Reporting a
///   different binary from the one a session will actually start would make
///   the environment panel lie.
fn find_on_path<F>(path_var: &str, binary: &str, is_executable: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    if binary.is_empty() {
        return None;
    }
    path_var
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(binary))
        .find(|candidate| is_executable(candidate))
}

/// Real-file-system predicate for `find_on_path`: a regular file (after
/// symlink resolution, which `metadata` does) with at least one execute
/// bit. A directory called `git` on PATH is not git.
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

// ---------------------------------------------------------------------------
// Pure part: version extraction
// ---------------------------------------------------------------------------

/// The version number out of a `--version` line.
///
/// The three tools disagree about shape — `git version 2.47.1`,
/// `1.0.88 (Claude Code)`, `codex-cli 0.20.0` — and all three answers are
/// the first dotted numeric token, so that is the rule rather than three
/// per-tool parsers. stderr is consulted only if stdout has nothing:
/// some tools print their banner there, but stdout is the contract.
///
/// Returns `None` rather than the raw line when nothing parses. A version
/// field containing "usage: git [--version] ..." is worse than an empty
/// one: the UI would render it as a version.
fn extract_version(stdout: &str, stderr: &str) -> Option<String> {
    version_in(stdout).or_else(|| version_in(stderr))
}

fn version_in(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
        let token = match token.strip_prefix('v') {
            Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
            _ => token,
        };
        let looks_like_version = token.starts_with(|c: char| c.is_ascii_digit())
            && token.contains('.')
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+');
        looks_like_version.then(|| token.to_string())
    })
}

// ---------------------------------------------------------------------------
// Pure part: login classification
// ---------------------------------------------------------------------------

/// What one candidate argv form told us. `Unrecognised` is the honest "ask
/// the next form" answer and is what everything ambiguous collapses to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LoginVerdict {
    LoggedIn { account_hint: Option<String> },
    LoggedOut,
    Unrecognised,
}

/// Classifies one login probe's output.
///
/// Order matters and is the whole point:
///
/// 1. "did not understand the question" wins outright, even on exit code 0,
///    because a help dump mentions logging in and would otherwise be read
///    as an answer;
/// 2. then the logged-out markers, before the logged-in ones, because
///    "Not logged in" and "token expired" both contain a positive marker as
///    a substring;
/// 3. only then the positive markers;
/// 4. anything left — silence, a novel phrasing, a bare non-zero exit — is
///    `Unrecognised`, never `Expired` (see the module header).
fn classify_login(stdout: &str, stderr: &str, exit_code: Option<i32>) -> LoginVerdict {
    let combined = format!("{stdout}\n{stderr}").to_lowercase();

    if contains_any(&combined, UNRECOGNISED_MARKERS) {
        return LoginVerdict::Unrecognised;
    }
    if contains_any(&combined, LOGGED_OUT_MARKERS) {
        return LoginVerdict::LoggedOut;
    }
    if contains_any(&combined, LOGGED_IN_MARKERS) {
        return LoginVerdict::LoggedIn {
            account_hint: account_hint(stdout).or_else(|| account_hint(stderr)),
        };
    }
    // A zero exit with no recognisable words is not evidence of a session;
    // a non-zero exit with none is not evidence of its absence. Both are
    // the same "we cannot tell".
    let _ = exit_code;
    LoginVerdict::Unrecognised
}

fn contains_any(haystack_lowercase: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| haystack_lowercase.contains(m))
}

/// A short, non-secret hint about which account is logged in — an email if
/// the CLI printed one, otherwise whatever follows "logged in as". Only a
/// hint: it goes on a dashboard card, so it must never carry a token.
fn account_hint(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_lowercase();
        for lead in ["logged in as ", "signed in as ", "account: ", "user: "] {
            if let Some(pos) = lower.find(lead) {
                let hint = line[pos + lead.len()..]
                    .trim()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c == '.' || c == ',');
                if !hint.is_empty() {
                    return Some(hint.to_string());
                }
            }
        }
    }
    text.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '@' && c != '.'))
        .find(|t| t.contains('@') && t.contains('.') && t.len() > 3)
        .map(|t| t.to_string())
}

// ---------------------------------------------------------------------------
// Pure part: Info.plist
// ---------------------------------------------------------------------------

/// Reads `<key>NAME</key><string>VALUE</string>` out of an XML plist.
///
/// No plist crate: the alternative is a dependency and a full XML parser to
/// read one string out of one file that Apple's own tooling generates in a
/// fixed shape. The scan is deliberately strict — if another `<key>` shows
/// up before the `<string>`, the key had a non-string value (or the file is
/// malformed) and the answer is `None`, not the next key's value. A wrong
/// version is worse than no version: the user would compare it against a
/// release note and conclude their install is broken.
///
/// A binary plist (bplist00) simply contains none of these tags and yields
/// `None`, which is the correct answer for "cannot read this".
fn plist_string_value(plist: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let after = plist.split_once(&key_tag)?.1;
    let open = after.find("<string>")?;
    if after[..open].contains("<key>") {
        return None;
    }
    let value_start = open + "<string>".len();
    let rest = &after[value_start..];
    let end = rest.find("</string>")?;
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

// ---------------------------------------------------------------------------
// Impure part: bounded subprocess execution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Capture {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RunOutcome {
    Completed(Capture),
    TimedOut,
    Failed(String),
}

/// Runs a command, captures both streams, and gives up after `timeout`.
///
/// Hand-rolled rather than `wait_with_output()` because that call blocks
/// forever, and a CLI that hangs waiting for a TTY would take the daemon's
/// probe thread with it. The shape is: pipe both streams into reader
/// threads (so a child writing more than a pipe buffer cannot deadlock
/// against us), then poll `try_wait` until the deadline, then kill and reap.
///
/// stdin is `/dev/null` on purpose: a CLI that decides to prompt gets EOF
/// and exits instead of waiting for input that will never come.
///
/// On timeout the reader threads are left detached; killing the child
/// closes the pipes, so they finish on their own, and we do not join them
/// — the point of the deadline is not to block here.
fn run_capture(program: &Path, args: &[&str], timeout: Duration) -> RunOutcome {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return RunOutcome::Failed(e.to_string()),
    };

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || read_to_string_lossy(stdout_pipe));
    let stderr_reader = std::thread::spawn(move || read_to_string_lossy(stderr_pipe));

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // `join` can only fail if a reader panicked; reading a pipe
                // to end of file does not, but an empty string is still a
                // usable answer if it somehow did.
                let stdout = stdout_reader.join().unwrap_or_default();
                let stderr = stderr_reader.join().unwrap_or_default();
                return RunOutcome::Completed(Capture {
                    stdout,
                    stderr,
                    exit_code: status.code(),
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait(); // reap, so we do not leave a zombie
                    return RunOutcome::TimedOut;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return RunOutcome::Failed(e.to_string());
            }
        }
    }
}

fn read_to_string_lossy<R: std::io::Read>(pipe: Option<R>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut buf = Vec::new();
    match std::io::Read::read_to_end(&mut pipe, &mut buf) {
        Ok(_) => String::from_utf8_lossy(&buf).into_owned(),
        Err(_) => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Impure part: the clock, with a pure formatter
// ---------------------------------------------------------------------------

/// `ComponentStatus::checked_at` as UTC RFC 3339, to the second.
///
/// Formatted by hand rather than pulled from a date crate because this
/// module has exactly one timestamp and the conversion is twenty lines of
/// arithmetic that can be tested against known epochs. A clock before the
/// Unix epoch (a machine with a badly wrong date) formats as a pre-1970
/// instant rather than panicking.
fn now_rfc3339() -> String {
    let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    rfc3339_from_unix_secs(secs)
}

fn rfc3339_from_unix_secs(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Hinnant's
/// `civil_from_days`). Integer arithmetic only, no leap-second notion —
/// which is what RFC 3339 wants for a wall-clock stamp anyway.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A unique scratch directory under the system temp dir. No `tempfile`
    /// dependency: the crate has none today and one throwaway directory per
    /// test does not justify adding one. Cleaned up by `Scratch::drop`, and
    /// harmless if a panicking test skips that — it lives in the temp dir.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = format!(
                "autome-env-probe-{}-{}-{}-{}",
                label,
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0)
            );
            let dir = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&dir).expect("temp dir is writable in tests");
            Scratch(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn file(&self, name: &str, contents: &str, executable: bool) -> PathBuf {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent dir is creatable");
            }
            std::fs::write(&path, contents).expect("temp file is writable");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = if executable { 0o755 } else { 0o644 };
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                    .expect("temp file permissions are settable");
            }
            let _ = executable;
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // --- version extraction ------------------------------------------------

    #[test]
    fn git_version_line_yields_the_number() {
        assert_eq!(
            extract_version("git version 2.47.1\n", ""),
            Some("2.47.1".to_string())
        );
    }

    #[test]
    fn apple_git_suffix_does_not_confuse_the_scan() {
        assert_eq!(
            extract_version("git version 2.39.5 (Apple Git-154)\n", ""),
            Some("2.39.5".to_string())
        );
    }

    #[test]
    fn claude_version_line_yields_the_number() {
        assert_eq!(
            extract_version("1.0.88 (Claude Code)\n", ""),
            Some("1.0.88".to_string())
        );
    }

    #[test]
    fn codex_version_line_yields_the_number() {
        assert_eq!(
            extract_version("codex-cli 0.20.0\n", ""),
            Some("0.20.0".to_string())
        );
        assert_eq!(
            extract_version("codex-cli v0.21.0-alpha.1\n", ""),
            Some("0.21.0-alpha.1".to_string())
        );
    }

    #[test]
    fn version_falls_back_to_stderr_when_stdout_is_silent() {
        assert_eq!(
            extract_version("", "some-tool 3.2.1\n"),
            Some("3.2.1".to_string())
        );
    }

    #[test]
    fn unparseable_version_output_is_none_not_the_raw_line() {
        assert_eq!(extract_version("", ""), None);
        assert_eq!(extract_version("usage: git [--help]\n", ""), None);
    }

    // --- PATH scanning -----------------------------------------------------

    #[test]
    fn path_scan_returns_the_first_match_in_path_order() {
        let found = find_on_path("/a:/b:/c", "claude", |p| {
            p == Path::new("/b/claude") || p == Path::new("/c/claude")
        });
        assert_eq!(found, Some(PathBuf::from("/b/claude")));
    }

    #[test]
    fn path_scan_skips_entries_that_are_not_executable() {
        // /a/git exists but is not executable, /b/git is.
        let found = find_on_path("/a:/b", "git", |p| p == Path::new("/b/git"));
        assert_eq!(found, Some(PathBuf::from("/b/git")));
    }

    #[test]
    fn path_scan_skips_empty_segments_and_can_find_nothing() {
        let seen = std::cell::RefCell::new(Vec::new());
        let found = find_on_path("::/a:", "codex", |p| {
            seen.borrow_mut().push(p.to_path_buf());
            false
        });
        assert_eq!(found, None);
        assert_eq!(seen.into_inner(), vec![PathBuf::from("/a/codex")]);
    }

    #[test]
    fn path_scan_on_a_real_directory_honours_the_execute_bit() {
        let empty = Scratch::new("path-empty");
        let real = Scratch::new("path-real");
        real.file("tool", "#!/bin/sh\nexit 0\n", true);
        real.file("not-a-tool", "plain text\n", false);

        let path_var = format!("{}:{}", empty.path().display(), real.path().display());
        assert_eq!(
            find_on_path(&path_var, "tool", is_executable_file),
            Some(real.path().join("tool"))
        );
        assert_eq!(
            find_on_path(&path_var, "not-a-tool", is_executable_file),
            None
        );
        assert_eq!(find_on_path(&path_var, "absent", is_executable_file), None);
    }

    #[test]
    fn a_directory_on_path_is_not_a_binary() {
        let scratch = Scratch::new("path-dir");
        std::fs::create_dir_all(scratch.path().join("git")).expect("dir is creatable");
        let path_var = scratch.path().display().to_string();
        assert_eq!(find_on_path(&path_var, "git", is_executable_file), None);
    }

    // --- Info.plist --------------------------------------------------------

    const REALISTIC_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>iTerm2</string>
	<key>CFBundleVersion</key>
	<string>3.5.11</string>
	<key>CFBundleShortVersionString</key>
	<string>3.5.11</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.15</string>
</dict>
</plist>
"#;

    #[test]
    fn plist_scan_reads_the_short_version_string() {
        assert_eq!(
            plist_string_value(REALISTIC_PLIST, ITERM_VERSION_KEY),
            Some("3.5.11".to_string())
        );
    }

    #[test]
    fn plist_scan_does_not_return_a_neighbouring_keys_value() {
        let plist = r#"<dict>
	<key>CFBundleShortVersionString</key>
	<key>CFBundleVersion</key>
	<string>9999</string>
</dict>"#;
        assert_eq!(plist_string_value(plist, ITERM_VERSION_KEY), None);
    }

    #[test]
    fn malformed_or_binary_plists_yield_none_rather_than_rubbish() {
        assert_eq!(
            plist_string_value(
                "<key>CFBundleShortVersionString</key><string>3.5",
                ITERM_VERSION_KEY
            ),
            None,
            "truncated value"
        );
        assert_eq!(
            plist_string_value("bplist00\u{0}\u{1}rubbish", ITERM_VERSION_KEY),
            None,
            "binary plist"
        );
        assert_eq!(
            plist_string_value("", ITERM_VERSION_KEY),
            None,
            "empty file"
        );
        assert_eq!(
            plist_string_value(
                "<key>CFBundleShortVersionString</key><string>   </string>",
                ITERM_VERSION_KEY
            ),
            None,
            "blank value is not a version"
        );
    }

    // --- login classification ----------------------------------------------

    #[test]
    fn a_positive_answer_is_logged_in() {
        assert_eq!(
            classify_login("Logged in using ChatGPT\n", "", Some(0)),
            LoginVerdict::LoggedIn { account_hint: None }
        );
    }

    #[test]
    fn a_positive_answer_carries_an_account_hint_when_one_is_printed() {
        assert_eq!(
            classify_login("Logged in as dannie@example.com\n", "", Some(0)),
            LoginVerdict::LoggedIn {
                account_hint: Some("dannie@example.com".to_string())
            }
        );
        assert_eq!(
            classify_login("Authenticated.\nAccount: work-team\n", "", Some(0)),
            LoginVerdict::LoggedIn {
                account_hint: Some("work-team".to_string())
            }
        );
    }

    #[test]
    fn not_logged_in_is_logged_out_even_though_it_contains_logged_in() {
        assert_eq!(
            classify_login("Not logged in. Run `codex login`.\n", "", Some(1)),
            LoginVerdict::LoggedOut
        );
    }

    #[test]
    fn an_expired_session_is_logged_out_even_on_a_zero_exit() {
        assert_eq!(
            classify_login("Logged in, but the token has expired.\n", "", Some(0)),
            LoginVerdict::LoggedOut
        );
    }

    #[test]
    fn a_renamed_subcommand_is_unrecognised_never_expired() {
        // The §7.2 case: the CLI moved its flag. Calling this `Expired`
        // would send the user to re-authenticate for nothing.
        assert_eq!(
            classify_login("", "error: unrecognized subcommand 'auth'\n", Some(2)),
            LoginVerdict::Unrecognised
        );
        assert_eq!(
            classify_login(
                "Usage: claude [options] [command]\n  login  Log in\n",
                "",
                Some(0)
            ),
            LoginVerdict::Unrecognised,
            "a help dump mentions login but answers nothing"
        );
    }

    #[test]
    fn silence_and_novel_phrasings_are_unrecognised() {
        assert_eq!(classify_login("", "", Some(0)), LoginVerdict::Unrecognised);
        assert_eq!(
            classify_login("session vault sealed\n", "", Some(3)),
            LoginVerdict::Unrecognised,
            "a phrasing no marker covers must not be guessed at"
        );
    }

    // --- subprocess execution ----------------------------------------------

    #[test]
    fn a_completed_command_captures_both_streams_and_its_exit_code() {
        let outcome = run_capture(
            Path::new("/bin/sh"),
            &["-c", "echo out; echo err 1>&2; exit 7"],
            Duration::from_secs(5),
        );
        match outcome {
            RunOutcome::Completed(capture) => {
                assert_eq!(capture.stdout.trim(), "out");
                assert_eq!(capture.stderr.trim(), "err");
                assert_eq!(capture.exit_code, Some(7));
            }
            other => panic!("expected a completed run, got {other:?}"),
        }
    }

    #[test]
    fn a_hung_command_is_killed_at_the_deadline() {
        let started = Instant::now();
        let outcome = run_capture(
            Path::new("/bin/sh"),
            &["-c", "sleep 30"],
            Duration::from_millis(200),
        );
        assert_eq!(outcome, RunOutcome::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline, not the child, decides when we return"
        );
    }

    #[test]
    fn a_command_that_cannot_be_spawned_fails_rather_than_panicking() {
        let outcome = run_capture(
            Path::new("/definitely/not/here/autome-nonexistent"),
            &["--version"],
            Duration::from_secs(1),
        );
        assert!(matches!(outcome, RunOutcome::Failed(_)), "got {outcome:?}");
    }

    // --- whole-component probes --------------------------------------------

    #[test]
    fn a_binary_absent_from_path_probes_as_not_present_without_panicking() {
        let empty = Scratch::new("probe-empty");
        let path_var = empty.path().display().to_string();
        for component in [Component::Git, Component::Claude, Component::Codex] {
            let status = probe_with(component, &path_var, Path::new("/nonexistent.app"), "t");
            assert_eq!(status.component, component);
            assert!(!status.present, "{component:?} should not be present");
            assert_eq!(status.version, None);
            assert_eq!(status.path, None);
            assert_eq!(status.login, Login::NotApplicable);
            assert_eq!(status.checked_at, "t");
            assert!(!status.is_ready());
        }
    }

    #[test]
    fn an_absent_iterm2_bundle_probes_as_not_present() {
        let status = probe_with(
            Component::ITerm2,
            "",
            Path::new("/Applications/DefinitelyNotITerm.app"),
            "t",
        );
        assert!(!status.present);
        assert_eq!(status.login, Login::NotApplicable);
    }

    #[test]
    fn a_present_iterm2_bundle_reports_its_short_version_string() {
        let scratch = Scratch::new("iterm-app");
        let app = scratch.path().join("iTerm.app");
        std::fs::create_dir_all(app.join("Contents")).expect("bundle dir is creatable");
        std::fs::write(app.join("Contents/Info.plist"), REALISTIC_PLIST)
            .expect("plist is writable");

        let status = probe_with(Component::ITerm2, "", &app, "t");
        assert!(status.present);
        assert_eq!(status.version, Some("3.5.11".to_string()));
        assert_eq!(status.path, Some(app.display().to_string()));
        assert_eq!(status.login, Login::NotApplicable);
        assert!(status.is_ready());
    }

    #[test]
    fn a_bundle_without_a_readable_plist_is_still_present_with_no_version() {
        let scratch = Scratch::new("iterm-noplist");
        let app = scratch.path().join("iTerm.app");
        std::fs::create_dir_all(&app).expect("bundle dir is creatable");
        let status = probe_with(Component::ITerm2, "", &app, "t");
        assert!(status.present, "the app is there even if the plist is not");
        assert_eq!(status.version, None);
    }

    #[test]
    fn a_fake_git_on_path_is_probed_for_version_and_has_no_login() {
        let scratch = Scratch::new("fake-git");
        scratch.file("git", "#!/bin/sh\necho 'git version 2.47.1'\n", true);
        let path_var = scratch.path().display().to_string();

        let status = probe_with(Component::Git, &path_var, Path::new("/nonexistent"), "t");
        assert!(status.present);
        assert_eq!(status.version, Some("2.47.1".to_string()));
        assert_eq!(
            status.path,
            Some(scratch.path().join("git").display().to_string())
        );
        assert_eq!(status.login, Login::NotApplicable, "§11: Git 登录态不适用");
        assert!(status.is_ready());
    }

    #[test]
    fn a_cli_that_answers_the_first_login_probe_is_reported_logged_in() {
        // Stands in for the real CLI: any argv, one answer.
        let scratch = Scratch::new("fake-claude-ok");
        scratch.file(
            "claude",
            "#!/bin/sh\necho 'Logged in as dannie@example.com'\n",
            true,
        );
        let path_var = scratch.path().display().to_string();
        let status = probe_with(Component::Claude, &path_var, Path::new("/nonexistent"), "t");
        assert!(status.present);
        assert_eq!(
            status.login,
            Login::Ok {
                account_hint: Some("dannie@example.com".to_string())
            }
        );
        assert!(status.is_ready());
    }

    #[test]
    fn a_cli_whose_login_flags_all_moved_yields_unknown_listing_what_was_tried() {
        let scratch = Scratch::new("fake-codex-moved");
        scratch.file(
            "codex",
            "#!/bin/sh\n\
             case \"$1\" in\n\
             --version) echo 'codex-cli 0.20.0'; exit 0;;\n\
             *) echo 'error: unrecognized subcommand' 1>&2; exit 2;;\n\
             esac\n",
            true,
        );
        let path_var = scratch.path().display().to_string();
        let status = probe_with(Component::Codex, &path_var, Path::new("/nonexistent"), "t");

        assert!(status.present, "the binary is there; only the flag moved");
        assert_eq!(status.version, Some("0.20.0".to_string()));
        match &status.login {
            Login::Unknown { detail } => {
                for probe in CODEX_LOGIN_PROBES {
                    assert!(
                        detail.contains(&probe.args.join(" ")),
                        "detail should name every form tried, missing {:?}: {detail}",
                        probe.args
                    );
                }
            }
            other => panic!("expected Unknown, never Expired, got {other:?}"),
        }
        assert!(
            !status.login.is_blocking(),
            "an unprobeable login must not block the runtime"
        );
    }

    #[test]
    fn a_logged_out_cli_is_reported_expired() {
        let scratch = Scratch::new("fake-codex-out");
        scratch.file(
            "codex",
            "#!/bin/sh\necho 'Not logged in. Run codex login.' 1>&2\nexit 1\n",
            true,
        );
        let path_var = scratch.path().display().to_string();
        let status = probe_with(Component::Codex, &path_var, Path::new("/nonexistent"), "t");
        assert_eq!(status.login, Login::Expired);
        assert!(!status.is_ready());
    }

    // --- tables and clock ---------------------------------------------------

    #[test]
    fn only_the_two_clis_have_login_probe_tables() {
        assert!(!login_probes(Component::Claude).is_empty());
        assert!(!login_probes(Component::Codex).is_empty());
        assert!(login_probes(Component::Git).is_empty());
        assert!(login_probes(Component::ITerm2).is_empty());
        for probe in CLAUDE_LOGIN_PROBES.iter().chain(CODEX_LOGIN_PROBES) {
            assert!(!probe.args.is_empty(), "a probe needs argv");
            assert!(!probe.note.is_empty(), "a probe needs a reason to exist");
        }
    }

    #[test]
    fn the_worst_case_probe_time_stays_within_a_users_patience() {
        // Four components, at most one --version each plus the login table.
        let subprocesses = 2 + CLAUDE_LOGIN_PROBES.len() + CODEX_LOGIN_PROBES.len();
        assert!(
            PROBE_TIMEOUT * subprocesses as u32 <= Duration::from_secs(45),
            "a longer table needs a shorter deadline, or a rethink"
        );
    }

    #[test]
    fn timestamps_format_as_utc_rfc3339() {
        assert_eq!(rfc3339_from_unix_secs(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            rfc3339_from_unix_secs(1_700_000_000),
            "2023-11-14T22:13:20Z"
        );
        assert_eq!(
            rfc3339_from_unix_secs(1_709_164_800),
            "2024-02-29T00:00:00Z"
        );
        assert_eq!(rfc3339_from_unix_secs(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn a_sweep_returns_all_four_components_in_order_under_one_timestamp() {
        let empty = Scratch::new("sweep");
        let path_var = empty.path().display().to_string();
        let env = probe_all_with(
            &path_var,
            Path::new("/nonexistent.app"),
            "2026-09-15T00:00:00Z",
        );

        let seen: Vec<Component> = env.components.iter().map(|c| c.component).collect();
        assert_eq!(seen, Component::ALL.to_vec());
        assert_eq!(env.checked_at, "2026-09-15T00:00:00Z");
        for status in &env.components {
            assert_eq!(status.checked_at, env.checked_at);
            assert!(!status.present);
        }
        assert!(!env.can_run_anything(), "no git means nothing can run");
    }

    #[test]
    fn probing_the_real_machine_does_not_panic() {
        // Deliberately cheap: iTerm2 is a stat, and a real sweep would
        // spend the whole login table against whichever CLIs this machine
        // has installed. Correctness lives in the tests above; this one
        // only proves the real wiring runs.
        let status = probe(Component::ITerm2);
        assert_eq!(status.component, Component::ITerm2);
        assert!(!status.checked_at.is_empty());
        assert!(status.checked_at.ends_with('Z'));
    }

    #[test]
    fn a_prerequisite_that_is_not_installed_is_reported_missing() {
        assert!(!has_prerequisite("autome-definitely-not-installed"));
        assert!(!has_prerequisite(""), "an empty name matches nothing");
    }
}
