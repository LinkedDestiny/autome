//! automed binary entrypoint: the blocking stdio loop described in plan
//! §3.1 — "stdout 仅承载协议帧，stderr 仅承载诊断信息". `tracing` is
//! therefore configured to write to stderr, never stdout, since stdout is
//! reserved exclusively for length-prefixed JSON-RPC frames read by
//! Electron Main.
//!
//! Every `Command` read from stdin produces exactly one `Reply` frame,
//! written before any `Event` frame it also produces — a caller waiting on
//! a response never hangs, even when dispatch fails or the frame is not a
//! valid `Command` at all (in which case `request_id`/`command_id` are
//! unknown, so the `Reply` carries empty strings for both and the failure
//! is also logged to stderr).

use automed::dispatch::{Ctx, handle_command, protocol_error_reply};
use automed::ipc::{Command, FrameError, Outbound, read_frame, write_frame};
use automed::store::Store;
use std::io;

fn write_outbound(writer: &mut impl io::Write, outbound: &Outbound) -> Result<(), FrameError> {
    let payload = serde_json::to_vec(outbound).expect("Outbound always serializes");
    write_frame(writer, &payload)
}

/// Placeholder ceiling until plan §3.2's frame-size limit is configured
/// from real environment/profile data.
const MAX_FRAME_LEN: u32 = 8 * 1024 * 1024;

fn main() {
    // `automed render-stream` is a filter, not the daemon: the session wrapper
    // pipes a CLI's `stream-json` output through it so the log a human opens
    // is readable. It must produce no tracing noise on stderr — stderr is
    // merged into the same pipe it is rendering.
    if std::env::args().nth(1).as_deref() == Some("render-stream") {
        // Both CLIs stream JSONL now, and their event vocabularies have
        // nothing in common, so the wrapper says which one it is piping. An
        // absent or unknown name falls back to Claude, which is what every
        // wrapper written before this argument existed passes.
        let runtime = std::env::args()
            .nth(2)
            .and_then(|s| autome_domain::role::Runtime::parse(&s))
            .unwrap_or(autome_domain::role::Runtime::Claude);
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        if let Err(e) = automed::stream_render::render_stream(runtime, stdin.lock(), &mut stdout) {
            eprintln!("render-stream: {e}");
            std::process::exit(1);
        }
        return;
    }

    // `automed protocol eval [<dir>]` is the gate a protocol version has to
    // pass. Also a filter rather than the daemon: a meta task's audit round
    // runs it as an ordinary command and reads the exit code, and the desktop
    // app calls the same function over IPC.
    if std::env::args().nth(1).as_deref() == Some("protocol")
        && std::env::args().nth(2).as_deref() == Some("eval")
    {
        let args: Vec<String> = std::env::args().skip(3).collect();
        // Defaults to the working directory, which is what a meta task's
        // audit round has checked out — the version being proposed, not the
        // one installed.
        let dir = std::path::PathBuf::from(
            args.iter()
                .find(|a| !a.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| ".".into()),
        );
        let (report, mut code) = automed::protocol::eval::run(&dir);
        print!("{report}");

        // The behaviour layer costs money, so it never runs unless asked for
        // by name. `--changed` is the audit round's form: the cases the
        // proposed changes reference, plus the three baselines.
        use automed::protocol::eval::Scope;
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
            let (behaviour, behaviour_code) =
                automed::protocol::eval::run_behaviour(&dir, scope, &tag);
            print!("{behaviour}");
            code = code.max(behaviour_code);
        }
        std::process::exit(code);
    }

    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let autome_home = automed::config_io::default_global_dir();
    // The ledger lives with the rest of this machine's Autome state, under
    // `AUTOME_HOME` (`~/.autome` by default) — one home, so "what does Autome
    // know" has one answer and one thing to back up.
    //
    // It used to default to `automed.sqlite3` in the working directory, which
    // meant running the core from a different directory silently started a
    // second, empty installation. That is not a hypothetical: this machine
    // accumulated four of them, and the desktop app's own copy sat in
    // Electron's `userData` where a dead prototype's file could squat on the
    // path unnoticed. `AUTOMED_DB_PATH` still overrides, for the tests that
    // need their own and for anyone who keeps state elsewhere on purpose.
    let db_path = std::env::var("AUTOMED_DB_PATH").unwrap_or_else(|_| {
        automed::config_io::default_db_path(&autome_home)
            .to_string_lossy()
            .into_owned()
    });
    let store = match Store::open(&db_path) {
        Ok(store) => store,
        Err(e) => {
            tracing::error!(error = %e, db_path, "failed to open the store");
            std::process::exit(1);
        }
    };
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    let mut ctx = Ctx::new(store, autome_home, home);

    tracing::info!(db_path, "automed core starting, reading commands on stdin");

    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    loop {
        let frame = match read_frame(&mut stdin_lock, MAX_FRAME_LEN) {
            Ok(frame) => frame,
            Err(automed::ipc::FrameError::UnexpectedEof) => {
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

        let outcome = handle_command(&mut ctx, &command);
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
}
