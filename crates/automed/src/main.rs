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

const USAGE: &str = "automed — Autome 的内核（桌面端经 stdio 与它通信）

用法：
  automed                         读 stdin 上的 JSON-RPC 帧（桌面端用）
  automed render-stream [runtime] 把 CLI 的 JSONL 渲染成可读日志（会话包装脚本用）
  automed protocol eval [dir] [--changed|--behaviour] [--tag X]
  automed --version | --help

人在终端里用的是 `autome` 命令，见 `autome --help`。
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // A filter, not the daemon: the session wrapper pipes a CLI's
        // `stream-json` output through it so the log a human opens is
        // readable. It must produce no tracing noise on stderr — stderr is
        // merged into the same pipe it is rendering.
        Some("render-stream") => std::process::exit(automed::entry::render_stream_main(&args[1..])),
        Some("protocol") if args.get(1).map(String::as_str) == Some("eval") => {
            std::process::exit(automed::entry::protocol_eval_main(&args[2..]))
        }
        Some("--help" | "-h" | "help") => {
            print!("{USAGE}");
            return;
        }
        Some("--version" | "-V") => {
            println!("automed {}", automed::entry::VERSION);
            return;
        }
        _ => {}
    }

    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let autome_home = automed::config_io::default_global_dir();
    // The ledger lives with the rest of this machine's Autome state, under
    // `AUTOME_HOME` (`~/.autome` by default) — one home, so "what does Autome
    // know" has one answer and one thing to back up. See `entry::ledger_path`.
    let db_path = automed::entry::ledger_path(&autome_home)
        .to_string_lossy()
        .into_owned();
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
