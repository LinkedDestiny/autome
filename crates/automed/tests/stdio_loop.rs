//! End-to-end verification of the real process loop in `main.rs`: stdin
//! frame -> Command -> dispatch -> SQLite -> Event -> stdout frame. This
//! spawns the actual `automed` binary as a child process and talks to it
//! over real OS pipes using the same framing/serde types Electron Main
//! would use, replacing the one-off manual subprocess/pipe check performed
//! during early M0 development with a committed, repeatable test.

use automed::ipc::{
    Command, Event, Outbound, PROTOCOL_VERSION, Reply, ReplyErrorCode, ReplyOutcome, encode_frame,
    read_frame,
};
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

fn recv(child: &mut Child) -> Outbound {
    let stdout = child.stdout.as_mut().expect("child stdout is piped");
    let frame = read_frame(stdout, MAX_FRAME_LEN).expect("read a frame from child stdout");
    serde_json::from_slice(&frame).expect("frame payload is a valid Outbound")
}

/// Every `Command` produces exactly one `Reply` before any `Event` it also
/// produces — most tests want to assert on the `Reply` and `Event` shapes
/// directly rather than repeat this match everywhere.
fn recv_reply(child: &mut Child) -> Reply {
    match recv(child) {
        Outbound::Reply(reply) => reply,
        Outbound::Event(event) => panic!("expected a Reply frame, got an Event: {event:?}"),
    }
}

fn recv_event(child: &mut Child) -> Event {
    match recv(child) {
        Outbound::Event(event) => event,
        Outbound::Reply(reply) => panic!("expected an Event frame, got a Reply: {reply:?}"),
    }
}

fn send_raw(child: &mut Child, payload: &[u8]) {
    let frame = encode_frame(payload);
    child
        .stdin
        .as_mut()
        .expect("child stdin is piped")
        .write_all(&frame)
        .expect("write to child stdin");
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
    let cmd = advance_nominal("run-e2e");

    send(&mut child, &cmd);
    let reply = recv_reply(&mut child);
    assert_eq!(reply.request_id, cmd.request_id);
    assert_eq!(reply.command_id, cmd.command_id);
    let (snapshot_seq, payload) = match reply.outcome {
        ReplyOutcome::Ok {
            snapshot_seq,
            payload,
        } => (snapshot_seq, payload),
        ReplyOutcome::Error { code, message } => {
            panic!("expected an Ok reply, got {code:?}: {message}")
        }
    };
    let event = recv_event(&mut child);

    assert_eq!(event.aggregate_id, "run-e2e");
    assert_eq!(event.aggregate_revision, 1);
    assert_eq!(event.event_type, "AdvanceNominal");
    assert_eq!(event.event_seq, 1);
    // The Reply's snapshot_seq/payload must agree with the Event that
    // follows it — a caller reading only the Reply already has the same
    // stream position and state a caller reading the Event would.
    assert_eq!(snapshot_seq, event.event_seq);
    assert_eq!(payload, event.payload);

    shutdown(child, &db_path);
}

#[test]
fn state_persists_across_process_restarts_against_the_same_db_path() {
    let db_path = temp_db_path("restart");

    let mut first = spawn_automed(&db_path);
    send(&mut first, &advance_nominal("run-restart"));
    let first_reply = recv_reply(&mut first);
    assert!(matches!(first_reply.outcome, ReplyOutcome::Ok { .. }));
    let first_event = recv_event(&mut first);
    assert_eq!(first_event.aggregate_revision, 1);
    drop(first.stdin.take());
    first.kill().ok();
    first.wait().ok();

    let mut second = spawn_automed(&db_path);
    send(&mut second, &advance_nominal("run-restart"));
    let second_reply = recv_reply(&mut second);
    assert!(matches!(second_reply.outcome, ReplyOutcome::Ok { .. }));
    let second_event = recv_event(&mut second);
    // A brand-new process pointed at the same db_path must see the prior
    // process's committed revision 1 and advance from there, not restart
    // the aggregate from scratch.
    assert_eq!(second_event.aggregate_revision, 2);

    shutdown(second, &db_path);
}

#[test]
fn unrecognized_methods_produce_an_error_reply_and_the_loop_keeps_going() {
    let db_path = temp_db_path("unknown-method");
    let mut child = spawn_automed(&db_path);

    let mut bogus = advance_nominal("run-e2e-2");
    bogus.request_id = "req-bogus".into();
    bogus.command_id = "cmd-bogus".into();
    bogus.method = "run.teleport".into();
    send(&mut child, &bogus);
    send(&mut child, &advance_nominal("run-e2e-2"));

    // Unlike the old silent-failure behavior, an unknown method now
    // produces its own error Reply — correlated to the bogus command by
    // request_id/command_id — with no Event, and the loop keeps going and
    // still answers the next command correctly right after it.
    let bogus_reply = recv_reply(&mut child);
    assert_eq!(bogus_reply.request_id, "req-bogus");
    assert_eq!(bogus_reply.command_id, "cmd-bogus");
    match bogus_reply.outcome {
        ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::UnknownMethod),
        ReplyOutcome::Ok { .. } => panic!("expected an Error reply for an unknown method"),
    }

    let good_reply = recv_reply(&mut child);
    assert!(matches!(good_reply.outcome, ReplyOutcome::Ok { .. }));
    let event = recv_event(&mut child);
    assert_eq!(event.aggregate_id, "run-e2e-2");
    assert_eq!(event.event_type, "AdvanceNominal");
    assert_eq!(event.aggregate_revision, 1);

    shutdown(child, &db_path);
}

#[test]
fn a_single_write_command_produces_exactly_one_reply_and_one_event() {
    let db_path = temp_db_path("reply-event-pairing");
    let mut child = spawn_automed(&db_path);

    // Two commands sent back to back must come back as Reply, Event,
    // Reply, Event, in that order — never batched or reordered — proving
    // each command yields exactly one Reply plus exactly one Event, not
    // more, not fewer, not out of order.
    send(&mut child, &advance_nominal("run-pair-1"));
    send(&mut child, &advance_nominal("run-pair-2"));

    let reply1 = recv_reply(&mut child);
    assert!(matches!(reply1.outcome, ReplyOutcome::Ok { .. }));
    let event1 = recv_event(&mut child);
    assert_eq!(event1.aggregate_id, "run-pair-1");

    let reply2 = recv_reply(&mut child);
    assert!(matches!(reply2.outcome, ReplyOutcome::Ok { .. }));
    let event2 = recv_event(&mut child);
    assert_eq!(event2.aggregate_id, "run-pair-2");

    shutdown(child, &db_path);
}

#[test]
fn a_malformed_frame_gets_an_invalid_params_reply_instead_of_hanging() {
    let db_path = temp_db_path("malformed-frame");
    let mut child = spawn_automed(&db_path);

    send_raw(&mut child, b"this is not json at all");

    let reply = recv_reply(&mut child);
    // request_id/command_id are unknown for an undeserializable frame, so
    // the Reply carries empty strings for both rather than fabricating one.
    assert_eq!(reply.request_id, "");
    assert_eq!(reply.command_id, "");
    match reply.outcome {
        ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
        ReplyOutcome::Ok { .. } => panic!("expected an Error reply for a malformed frame"),
    }

    // The loop must still be alive afterwards.
    send(&mut child, &advance_nominal("run-after-malformed"));
    let good_reply = recv_reply(&mut child);
    assert!(matches!(good_reply.outcome, ReplyOutcome::Ok { .. }));
    let event = recv_event(&mut child);
    assert_eq!(event.aggregate_id, "run-after-malformed");

    shutdown(child, &db_path);
}

#[test]
fn a_command_missing_a_required_param_gets_an_invalid_params_reply_with_no_event() {
    let db_path = temp_db_path("missing-param");
    let mut child = spawn_automed(&db_path);

    let mut cmd = advance_nominal("unused");
    cmd.params = json!({});
    send(&mut child, &cmd);

    let reply = recv_reply(&mut child);
    assert_eq!(reply.request_id, cmd.request_id);
    match reply.outcome {
        ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
        ReplyOutcome::Ok { .. } => panic!("expected an Error reply for a missing aggregate_id"),
    }

    // No Event follows a failed write; the very next frame is the next
    // command's own Reply.
    send(&mut child, &advance_nominal("run-after-invalid-params"));
    let good_reply = recv_reply(&mut child);
    assert!(matches!(good_reply.outcome, ReplyOutcome::Ok { .. }));

    shutdown(child, &db_path);
}

#[test]
fn read_methods_round_trip_through_the_real_binary() {
    let db_path = temp_db_path("read-methods");
    let mut child = spawn_automed(&db_path);

    let queue_get = Command {
        request_id: "req-queue".into(),
        command_id: "cmd-queue".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "queue.get".into(),
        params: json!({}),
    };
    send(&mut child, &queue_get);
    let reply = recv_reply(&mut child);
    match reply.outcome {
        ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["revision"], 0),
        ReplyOutcome::Error { code, message } => panic!("expected Ok, got {code:?}: {message}"),
    }

    let project_list = Command {
        request_id: "req-projects".into(),
        command_id: "cmd-projects".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "project.list".into(),
        params: json!({}),
    };
    send(&mut child, &project_list);
    let reply = recv_reply(&mut child);
    match reply.outcome {
        ReplyOutcome::Ok { payload, .. } => assert_eq!(payload, json!({ "projects": [] })),
        ReplyOutcome::Error { code, message } => panic!("expected Ok, got {code:?}: {message}"),
    }

    // A read method produces no Event — the very next frame after both
    // reads above is free to be another command's own Reply.
    send(&mut child, &advance_nominal("run-after-reads"));
    let reply = recv_reply(&mut child);
    assert!(matches!(reply.outcome, ReplyOutcome::Ok { .. }));
    let event = recv_event(&mut child);
    assert_eq!(event.aggregate_id, "run-after-reads");

    shutdown(child, &db_path);
}

/// Runs a `git` command against `dir` for building a real fixture repo --
/// mirrors `dispatch.rs`'s own test-module helper of the same shape, since
/// this file is a separate integration-test binary with no access to it.
fn fixture_git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("fixture git command should spawn");
    assert!(status.success(), "fixture git {args:?} failed in {dir:?}");
}

fn init_repo_with_one_commit(dir: &std::path::Path) {
    fixture_git(dir, &["init", "--quiet"]);
    fixture_git(
        dir,
        &[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=test",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "initial",
        ],
    );
}

fn temp_repo_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("automed-e2e-repo-{label}-{}", uuid::Uuid::new_v4()))
}

#[test]
fn register_target_then_create_from_target_then_project_list_shows_the_real_name() {
    let db_path = temp_db_path("register-create-list");
    let repo_dir = temp_repo_dir("register-create-list");
    std::fs::create_dir_all(&repo_dir).expect("create temp git repo dir");
    init_repo_with_one_commit(&repo_dir);

    let mut child = spawn_automed(&db_path);

    // Step 1: register_target -- the only Main-only method exercised here
    // at the real-binary level. It produces a Reply and, unlike a write
    // command, no Event: the very next frame is the next command's Reply.
    let register_cmd = Command {
        request_id: "req-register".into(),
        command_id: "cmd-register".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "project.register_target".into(),
        params: json!({ "kind": "ExistingRepository", "path": repo_dir.to_string_lossy() }),
    };
    send(&mut child, &register_cmd);
    let register_reply = recv_reply(&mut child);
    let target_id = match register_reply.outcome {
        ReplyOutcome::Ok { payload, .. } => {
            assert_eq!(payload["summary"]["kind"], "ExistingRepository");
            assert_eq!(payload["summary"]["is_git_repo"], true);
            assert_eq!(payload["summary"]["head_resolvable"], true);
            assert_eq!(payload["summary"]["worktree_clean"], true);
            payload["target_id"]
                .as_str()
                .expect("register_target payload must contain target_id")
                .to_string()
        }
        ReplyOutcome::Error { code, message } => {
            panic!("register_target failed: {code:?} {message}")
        }
    };

    // Step 2: create_from_target -- takes only the already-persisted
    // target_id plus scalars, never a raw path. Produces a Reply followed
    // by exactly one Event landing at IntentUnresolved.
    let real_display_name = "真实项目名称 E2E";
    let create_cmd = Command {
        request_id: "req-create".into(),
        command_id: "cmd-create".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "project.create_from_target".into(),
        params: json!({
            "target_id": target_id,
            "display_name": real_display_name,
            "trust_confirmed": true,
        }),
    };
    send(&mut child, &create_cmd);
    let create_reply = recv_reply(&mut child);
    assert!(matches!(create_reply.outcome, ReplyOutcome::Ok { .. }));
    let create_event = recv_event(&mut child);
    assert_eq!(create_event.event_type, "IntentUnresolved");

    // Step 3: project.list must now see the real display name the user
    // typed, through the real binary, not a fixture-seeded one.
    let project_list = Command {
        request_id: "req-projects".into(),
        command_id: "cmd-projects".into(),
        expected_revision: None,
        protocol_version: PROTOCOL_VERSION,
        method: "project.list".into(),
        params: json!({}),
    };
    send(&mut child, &project_list);
    let list_reply = recv_reply(&mut child);
    match list_reply.outcome {
        ReplyOutcome::Ok { payload, .. } => {
            let projects = payload["projects"]
                .as_array()
                .expect("projects must be an array");
            assert_eq!(projects.len(), 1);
            assert_eq!(projects[0]["id"], create_event.aggregate_id);
            assert_eq!(projects[0]["display_name"], real_display_name);
        }
        ReplyOutcome::Error { code, message } => panic!("expected Ok, got {code:?}: {message}"),
    }

    shutdown(child, &db_path);
    std::fs::remove_dir_all(&repo_dir).ok();
}
