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

    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let db_path =
        std::env::var("AUTOMED_DB_PATH").unwrap_or_else(|_| "automed.sqlite3".to_string());
    let store = match Store::open(&db_path) {
        Ok(store) => store,
        Err(e) => {
            tracing::error!(error = %e, db_path, "failed to open the store");
            std::process::exit(1);
        }
    };
    let autome_home = automed::config_io::default_global_dir();
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
