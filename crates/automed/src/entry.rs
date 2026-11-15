//! What the two executables share.
//!
//! `automed` is the core the desktop app talks to over stdio; `autome` is the
//! command a person types. Both open the same ledger, and both must answer
//! `render-stream` and `protocol eval` the same way: the session wrapper calls
//! back into whichever binary launched the session (`current_exe()`), so a
//! session started from the terminal renders its log through `autome`.
//!
//! The shared stdio entrypoint resolves the home for both binaries.

use std::path::{Path, PathBuf};

/// Where the ledger is: `AUTOMED_DB_PATH` if set, else under the home.
///
/// It used to default to `automed.sqlite3` in the working directory, which
/// meant running the core from a different directory silently started a
/// second, empty installation. One home, one ledger; the override is for the
/// test suites and for anyone who keeps state elsewhere on purpose.
pub fn ledger_path(autome_home: &Path) -> PathBuf {
    match std::env::var("AUTOMED_DB_PATH") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => crate::config_io::default_db_path(autome_home),
    }
}

/// `render-stream [runtime]`: a filter from a CLI's JSONL to a readable log.
/// Writes nothing else to stderr — stderr is merged into the same pipe.
pub fn render_stream_main(args: &[String]) -> i32 {
    // An absent or unknown name falls back to Claude, which is what every
    // wrapper written before the argument existed passes.
    let runtime = args
        .first()
        .and_then(|s| autome_domain::role::Runtime::parse(s))
        .unwrap_or(autome_domain::role::Runtime::Claude);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    match crate::stream_render::render_stream(runtime, stdin.lock(), &mut stdout) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("render-stream: {e}");
            1
        }
    }
}

/// `protocol eval [<dir>] [--changed | --behaviour] [--tag X]`: the gate a
/// protocol change has to pass before it ships. Prints the report and returns
/// the exit code.
pub fn protocol_eval_main(args: &[String]) -> i32 {
    // Defaults to the working directory: the version being proposed, not the
    // one built into this binary.
    let dir = PathBuf::from(
        args.iter()
            .find(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| ".".into()),
    );
    let (report, mut code) = crate::protocol::eval::run(&dir);
    print!("{report}");

    // The behaviour layer costs money, so it never runs unless asked for by
    // name. `--changed` is the audit round's form: the cases the proposed
    // changes reference, plus the three baselines.
    use crate::protocol::eval::Scope;
    let scope = if args.iter().any(|a| a == "--changed") {
        Some(Scope::Changed)
    } else if args.iter().any(|a| a == "--behaviour" || a == "--behavior") {
        Some(Scope::All)
    } else {
        None
    };
    if let Some(scope) = scope {
        let tag = args
            .iter()
            .position(|a| a == "--tag")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "未发布".into());
        let (behaviour, behaviour_code) = crate::protocol::eval::run_behaviour(&dir, scope, &tag);
        print!("{behaviour}");
        code = code.max(behaviour_code);
    }
    code
}

/// The version both binaries report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The CLI and standalone kernel share one compiled identity.
pub fn version_line(name: &str) -> String {
    format!("{name} {VERSION} ({})", effective_build_id())
}

/// Environment policy belongs to process entrypoints, never Ctx::new.
pub fn apply_kernel_env(ctx: &mut crate::dispatch::Ctx) {
    ctx.allow_older_kernel =
        explicit_adoption(std::env::var("AUTOME_ALLOW_OLDER_KERNEL").ok().as_deref());
    ctx.build_id = effective_build_id();
}

fn explicit_adoption(value: Option<&str>) -> bool {
    value == Some("1")
}

fn effective_build_id() -> String {
    #[cfg(debug_assertions)]
    {
        debug_build_id(
            crate::build_id::BUILD_ID,
            std::env::var("AUTOME_TEST_BUILD_ID").ok().as_deref(),
        )
    }
    #[cfg(not(debug_assertions))]
    {
        crate::build_id::BUILD_ID.into()
    }
}

#[cfg(debug_assertions)]
fn debug_build_id(compiled: &str, candidate: Option<&str>) -> String {
    use crate::build_id::{BuildId, Verdict, verdict};
    if let Some(candidate) = candidate {
        if candidate.parse::<BuildId>().is_ok()
            && matches!(
                verdict(candidate, Some(compiled)),
                Verdict::Older | Verdict::Same
            )
        {
            return candidate.into();
        }
        eprintln!("忽略 AUTOME_TEST_BUILD_ID：标识无效或不能往旧改");
    }
    compiled.into()
}

#[cfg(all(test, debug_assertions))]
mod kernel_env_tests {
    use super::*;

    #[test]
    fn kernel_adopt_env_is_explicit() {
        for value in [
            None,
            Some(""),
            Some("0"),
            Some("true"),
            Some("01"),
            Some(" 1"),
            Some("1\n"),
        ] {
            assert!(!explicit_adoption(value));
        }
        assert!(explicit_adoption(Some("1")));
        let current = "0000002-20200101T000000Z-000000000002";
        let old = "0000001-20200101T000000Z-000000000001";
        assert_eq!(debug_build_id(current, Some(old)), old);
        for value in [
            None,
            Some(""),
            Some("invalid"),
            Some(current),
            Some("9999999-29991231T000000Z-ffffffffffff"),
            Some("0000002-20200101T000000Z-000000000003"),
        ] {
            assert_eq!(debug_build_id(current, value), current);
        }
    }
}

use crate::dispatch::{Ctx, handle_command, protocol_error_reply};
use crate::ipc::{Command, FrameError, Outbound, read_frame, write_frame};
use crate::store::Store;
use std::io;

fn write_outbound(writer: &mut impl io::Write, outbound: &Outbound) -> Result<(), FrameError> {
    let payload = serde_json::to_vec(outbound).expect("Outbound always serializes");
    write_frame(writer, &payload)
}

/// Placeholder ceiling until plan §3.2's frame-size limit is configured
/// from real environment/profile data.
const MAX_FRAME_LEN: u32 = 8 * 1024 * 1024;

/// Serve RPC frames with diagnostics exclusively on stderr.
pub fn serve_stdio() -> i32 {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let autome_home = crate::config_io::default_global_dir();
    // The ledger lives with the rest of this machine's Autome state, under
    // `AUTOME_HOME` (`~/.autome` by default) — one home, so "what does Autome
    // know" has one answer and one thing to back up. See `entry::ledger_path`.
    let db_path = crate::entry::ledger_path(&autome_home)
        .to_string_lossy()
        .into_owned();
    let store = match Store::open(&db_path) {
        Ok(store) => store,
        Err(e) => {
            tracing::error!(error = %e, db_path, "failed to open the store");
            return 1;
        }
    };
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    let mut ctx = Ctx::new(store, autome_home, home);

    crate::entry::apply_kernel_env(&mut ctx);

    tracing::info!(
        db_path,
        build_id = %ctx.build_id,
        "automed core starting, reading commands on stdin"
    );

    let mut registration = None;
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    loop {
        let frame = match read_frame(&mut stdin_lock, MAX_FRAME_LEN) {
            Ok(frame) => frame,
            Err(crate::ipc::FrameError::UnexpectedEof) => {
                tracing::info!("stdin closed, shutting down");
                break;
            }
            Err(e) => {
                tracing::error!(?e, "failed to read frame from stdin");
                break;
            }
        };

        let command: Command = match serde_json::from_slice(&frame) {
            Ok(command) => command,
            Err(e) => {
                tracing::error!(error = %e, "received a frame that is not a valid Command");
                let reply = protocol_error_reply(format!("frame is not a valid Command: {e}"));
                if let Err(e) = write_outbound(&mut stdout_lock, &Outbound::Reply(reply)) {
                    tracing::error!(?e, "failed to write reply frame to stdout");
                    break;
                }
                continue;
            }
        };

        let ticking = command.method == "scheduler.tick";
        if ticking && registration.is_none() {
            match crate::driver_lock::register(ctx.store.path(), "autome core", None) {
                Ok(value) => registration = Some(value),
                Err(error) => {
                    tracing::error!(%error, "driver registration failed");
                    return 1;
                }
            }
        }
        let outcome = handle_command(&mut ctx, &command);
        if ticking
            && let Some(registration) = &registration
            && let Err(error) = registration.touch()
        {
            tracing::error!(%error, "driver heartbeat failed");
            return 1;
        }
        if let Err(e) = write_outbound(&mut stdout_lock, &Outbound::Reply(outcome.reply)) {
            tracing::error!(?e, "failed to write reply frame to stdout");
            break;
        }
        let mut write_failed = false;
        for event in outcome.events {
            if let Err(e) = write_outbound(&mut stdout_lock, &Outbound::Event(event)) {
                tracing::error!(?e, "failed to write event frame to stdout");
                write_failed = true;
                break;
            }
        }
        if write_failed {
            break;
        }
    }
    0
}
