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
}
