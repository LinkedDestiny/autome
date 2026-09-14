//! End-to-end verification of the real process loop in `main.rs`: stdin
//! frame -> Command -> dispatch -> SQLite -> Event -> stdout frame. This
//! spawns the actual `automed` binary as a child process and talks to it
//! over real OS pipes using the same framing/serde types Electron Main
//! would use, replacing the one-off manual subprocess/pipe check performed
//! during early M0 development with a committed, repeatable test.

use automed::ipc::{Command, Event, PROTOCOL_VERSION, encode_frame, read_frame};
use serde_json::json;
use std::io::Write;
use std::process::{Child, Command as Process, Stdio};

const MAX_FRAME_LEN: u32 = 8 * 1024 * 1024;

fn temp_db_path(label: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "automed-e2e-{label}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
        .to_string_lossy()
        .into_owned()
}

fn spawn_automed(db_path: &str) -> Child {
    Process::new(env!("CARGO_BIN_EXE_automed"))
        .env("AUTOMED_DB_PATH", db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn the automed binary")
}

fn send(child: &mut Child, command: &Command) {
    let payload = serde_json::to_vec(command).expect("Command always serializes");
    let frame = encode_frame(&payload);
    child
        .stdin
        .as_mut()
        .expect("child stdin is piped")
        .write_all(&frame)
        .expect("write to child stdin");
}

fn recv(child: &mut Child) -> Event {
    let stdout = child.stdout.as_mut().expect("child stdout is piped");
    let frame = read_frame(stdout, MAX_FRAME_LEN).expect("read a frame from child stdout");
    serde_json::from_slice(&frame).expect("frame payload is a valid Event")
}

fn shutdown(mut child: Child, db_path: &str) {
    drop(child.stdin.take());
    child.kill().ok();
    child.wait().ok();
    std::fs::remove_file(db_path).ok();
}

fn advance_nominal(aggregate_id: &str) -> Command {
    Command {
        request_id: "req-1".into(),
        command_id: "cmd-1".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "run.advance_nominal".into(),
        params: json!({ "aggregate_id": aggregate_id }),
    }
}

#[test]
fn advance_nominal_round_trips_through_the_real_binary() {
    let db_path = temp_db_path("advance");
    let mut child = spawn_automed(&db_path);

    send(&mut child, &advance_nominal("run-e2e"));
    let event = recv(&mut child);

    assert_eq!(event.aggregate_id, "run-e2e");
    assert_eq!(event.aggregate_revision, 1);
    assert_eq!(event.event_type, "AdvanceNominal");
    assert_eq!(event.event_seq, 1);

    shutdown(child, &db_path);
}

#[test]
fn state_persists_across_process_restarts_against_the_same_db_path() {
    let db_path = temp_db_path("restart");

    let mut first = spawn_automed(&db_path);
    send(&mut first, &advance_nominal("run-restart"));
    let first_event = recv(&mut first);
    assert_eq!(first_event.aggregate_revision, 1);
    drop(first.stdin.take());
    first.kill().ok();
    first.wait().ok();

    let mut second = spawn_automed(&db_path);
    send(&mut second, &advance_nominal("run-restart"));
    let second_event = recv(&mut second);
    // A brand-new process pointed at the same db_path must see the prior
    // process's committed revision 1 and advance from there, not restart
    // the aggregate from scratch.
    assert_eq!(second_event.aggregate_revision, 2);

    shutdown(second, &db_path);
}

#[test]
fn unrecognized_methods_produce_no_frame_and_the_loop_keeps_going() {
    let db_path = temp_db_path("unknown-method");
    let mut child = spawn_automed(&db_path);

    let mut bogus = advance_nominal("run-e2e-2");
    bogus.method = "run.teleport".into();
    send(&mut child, &bogus);
    send(&mut child, &advance_nominal("run-e2e-2"));

    // main.rs logs dispatch failures to stderr and never writes a frame for
    // them, so the first frame read back must be the *second* command's
    // event, proving the unknown method neither crashed the loop nor wrote
    // a stray frame ahead of it.
    let event = recv(&mut child);
    assert_eq!(event.aggregate_id, "run-e2e-2");
    assert_eq!(event.event_type, "AdvanceNominal");
    assert_eq!(event.aggregate_revision, 1);

    shutdown(child, &db_path);
}
