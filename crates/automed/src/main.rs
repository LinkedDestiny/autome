//! automed binary entrypoint: the blocking stdio loop described in plan
//! §3.1 — "stdout 仅承载协议帧，stderr 仅承载诊断信息". `tracing` is
//! therefore configured to write to stderr, never stdout, since stdout is
//! reserved exclusively for length-prefixed JSON-RPC frames read by
//! Electron Main.
//!
//! This is the M0 thin slice: one aggregate (Run), one method
//! (`run.advance_nominal`), no error-reply envelope yet (the plan has not
//! yet specified one at the level of detail modeled so far) — malformed
//! commands and dispatch failures are logged to stderr and the loop
//! continues rather than crashing the process or fabricating a fake Event.

use automed::dispatch::dispatch;
use automed::ipc::{Command, read_frame, write_frame};
use automed::store::EventStore;
use std::io;

/// Placeholder ceiling until plan §3.2's frame-size limit is configured
/// from real environment/profile data.
const MAX_FRAME_LEN: u32 = 8 * 1024 * 1024;

fn main() {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let db_path =
        std::env::var("AUTOMED_DB_PATH").unwrap_or_else(|_| "automed.sqlite3".to_string());
    let mut store = match EventStore::open(&db_path) {
        Ok(store) => store,
        Err(e) => {
            tracing::error!(error = %e, db_path, "failed to open event store");
            std::process::exit(1);
        }
    };

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
                continue;
            }
        };

        match dispatch(&mut store, &command) {
            Ok(event) => {
                let payload = serde_json::to_vec(&event).expect("Event always serializes");
                if let Err(e) = write_frame(&mut stdout_lock, &payload) {
                    tracing::error!(?e, "failed to write event frame to stdout");
                    break;
                }
            }
            Err(e) => {
                tracing::error!(
                    request_id = %command.request_id,
                    method = %command.method,
                    ?e,
                    "command dispatch failed"
                );
            }
        }
    }
}
