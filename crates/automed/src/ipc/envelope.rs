//! Command/Event envelopes per plan §3.2.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

/// §3.2: "所有命令必须包含：request_id · command_id · expected_revision ·
/// protocol_version · method · params". `expected_revision` is `None` only
/// for commands that are not scoped to a specific aggregate revision (e.g.
/// the very first command against a brand-new aggregate); every field here
/// is otherwise required, not defaulted, so a command missing one fails to
/// deserialize instead of silently proceeding with an assumed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub request_id: String,
    pub command_id: String,
    pub expected_revision: Option<u64>,
    pub protocol_version: u32,
    pub method: String,
    pub params: Value,
}

/// §3.2: "所有事件必须包含：event_seq · event_id · aggregate_id ·
/// aggregate_revision · event_type · occurred_at · payload". `event_seq` is
/// the global, gap-free stream position a Renderer uses to detect a missed
/// event and force a resync (§3.2: "遇到序号缺口，废弃局部缓存并重新同步");
/// allocating it is the emitting side's job, not this envelope's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub event_seq: u64,
    pub event_id: String,
    pub aggregate_id: String,
    pub aggregate_revision: u64,
    pub event_type: String,
    pub occurred_at: String,
    pub payload: Value,
}

/// Everything written to stdout is one of these two frame kinds, tagged by
/// `"frame"` in the JSON so Electron Main's `sidecar.js` can dispatch on it
/// without guessing. `Event` keeps its §3.2 shape untouched — it is still
/// the broadcast stream a Renderer subscribes to and resyncs against via
/// `event_seq`. `Reply` is new: a plain RPC response correlated to a
/// `Command` by `request_id`, per §3.2 "命令响应丢失时按 command_id 查询,
/// 不盲目重发" — every `Command` produces exactly one `Reply`, even on
/// failure, so a caller waiting on a response never hangs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Outbound {
    Event(Event),
    Reply(Reply),
}

/// The RPC response to exactly one `Command`. `snapshot_seq` is the
/// `events` table's max `seq` as of when this reply was built — even on a
/// write, so a caller knows the stream position to resume subscribing
/// from (§3.2's "先取带 snapshot_seq 的快照，再从 snapshot_seq + 1 订阅",
/// applied uniformly to every reply, not just reads).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub request_id: String,
    pub command_id: String,
    pub protocol_version: u32,
    pub outcome: ReplyOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReplyOutcome {
    Ok {
        snapshot_seq: u64,
        payload: Value,
    },
    Error {
        code: ReplyErrorCode,
        message: String,
    },
}

/// Kept deliberately narrow: each variant is a distinct condition a caller
/// needs to branch on differently (e.g. `ProtocolViolation` vs `NotFound`
/// are never safe to conflate — see plan §5.1's cross-project id-leak
/// rule), not a general-purpose bag for every internal failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyErrorCode {
    UnknownMethod,
    InvalidParams,
    NotFound,
    RevisionConflict,
    ProtocolViolation,
    /// A domain reducer rejected the event given the aggregate's current
    /// state (e.g. advancing a Run past its terminal phase). Distinct from
    /// `Internal`: this is an expected, well-formed rejection, not a bug —
    /// collapsing the two would make real internal failures harder to spot.
    TransitionRejected,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_round_trips_through_json() {
        let command = Command {
            request_id: "req-1".into(),
            command_id: "cmd-1".into(),
            expected_revision: Some(3),
            protocol_version: PROTOCOL_VERSION,
            method: "run.advance".into(),
            params: json!({"aggregate_id": "run-1"}),
        };
        let json = serde_json::to_string(&command).unwrap();
        let decoded: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, command);
    }

    #[test]
    fn command_missing_protocol_version_fails_to_deserialize() {
        let raw = r#"{
            "request_id": "req-1",
            "command_id": "cmd-1",
            "expected_revision": null,
            "method": "run.advance",
            "params": {}
        }"#;
        assert!(serde_json::from_str::<Command>(raw).is_err());
    }

    #[test]
    fn event_round_trips_through_json() {
        let event = Event {
            event_seq: 42,
            event_id: "evt-1".into(),
            aggregate_id: "run-1".into(),
            aggregate_revision: 7,
            event_type: "AdvanceNominal".into(),
            occurred_at: "2026-09-14T00:00:00Z".into(),
            payload: json!({"phase": "Executing"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn event_missing_event_seq_fails_to_deserialize() {
        let raw = r#"{
            "event_id": "evt-1",
            "aggregate_id": "run-1",
            "aggregate_revision": 7,
            "event_type": "AdvanceNominal",
            "occurred_at": "2026-09-14T00:00:00Z",
            "payload": {}
        }"#;
        assert!(serde_json::from_str::<Event>(raw).is_err());
    }

    #[test]
    fn outbound_event_round_trips_and_carries_the_frame_tag() {
        let event = Event {
            event_seq: 1,
            event_id: "evt-1".into(),
            aggregate_id: "run-1".into(),
            aggregate_revision: 1,
            event_type: "AdvanceNominal".into(),
            occurred_at: "2026-09-14T00:00:00Z".into(),
            payload: json!({}),
        };
        let outbound = Outbound::Event(event.clone());
        let value = serde_json::to_value(&outbound).unwrap();
        assert_eq!(value.get("frame").and_then(|v| v.as_str()), Some("event"));
        let decoded: Outbound = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, Outbound::Event(event));
    }

    #[test]
    fn outbound_reply_ok_round_trips_and_carries_the_frame_tag() {
        let reply = Reply {
            request_id: "req-1".into(),
            command_id: "cmd-1".into(),
            protocol_version: PROTOCOL_VERSION,
            outcome: ReplyOutcome::Ok {
                snapshot_seq: 3,
                payload: json!({"projects": []}),
            },
        };
        let outbound = Outbound::Reply(reply.clone());
        let value = serde_json::to_value(&outbound).unwrap();
        assert_eq!(value.get("frame").and_then(|v| v.as_str()), Some("reply"));
        assert_eq!(
            value.pointer("/outcome/status").and_then(|v| v.as_str()),
            Some("ok")
        );
        let decoded: Outbound = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, Outbound::Reply(reply));
    }

    #[test]
    fn outbound_reply_error_round_trips_with_its_code() {
        let reply = Reply {
            request_id: "req-2".into(),
            command_id: "cmd-2".into(),
            protocol_version: PROTOCOL_VERSION,
            outcome: ReplyOutcome::Error {
                code: ReplyErrorCode::ProtocolViolation,
                message: "task does not belong to project".into(),
            },
        };
        let value = serde_json::to_value(Outbound::Reply(reply.clone())).unwrap();
        assert_eq!(
            value.pointer("/outcome/status").and_then(|v| v.as_str()),
            Some("error")
        );
        assert_eq!(
            value.pointer("/outcome/code").and_then(|v| v.as_str()),
            Some("protocol_violation")
        );
        let decoded: Outbound = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, Outbound::Reply(reply));
    }

    #[test]
    fn outbound_event_and_reply_are_never_confused_by_a_missing_frame_tag() {
        let raw = r#"{"request_id":"r","command_id":"c","protocol_version":1,"outcome":{"status":"ok","snapshot_seq":0,"payload":{}}}"#;
        assert!(serde_json::from_str::<Outbound>(raw).is_err());
    }
}
