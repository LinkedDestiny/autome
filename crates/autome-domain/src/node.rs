//! TaskGraph node state machine per plan §6.3.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Ready,
    Producing,
    ClaimSubmitted,
    Verifying,
    Evaluating,
    Accepted,
    RepairRequired,
    Inconclusive,
    ProtocolViolation,
    Isolated,
    ReplanRequired,
    NeedsHuman,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeEvent {
    BecomeReady,
    StartProducing,
    ClaimSubmitted,
    StartVerifying,
    VerificationPassed,
    VerificationFailedRepairable,
    VerificationInconclusive,
    VerificationProtocolViolation,
    EvaluationAccepted,
    EvaluationNeedsRepair,
    EvaluationNeedsReplan,
    EvaluationNeedsHuman,
    RepairReady,
    /// Inconclusive was proven transient and retry budget remains.
    RetryAfterInconclusive,
    /// Control-plane integrity intact: isolate the candidate and restart in
    /// a fresh clone. Modeled as a single event; the caller is responsible
    /// for actually provisioning the fresh clone before re-emitting
    /// BecomeReady.
    IsolateAndRetry,
    HumanDecisionRecorded,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("event {event:?} is not legal from node status {status:?}")]
pub struct NodeTransitionError {
    pub status: NodeStatus,
    pub event: NodeEvent,
}

pub fn apply(status: NodeStatus, event: NodeEvent) -> Result<NodeStatus, NodeTransitionError> {
    use NodeEvent as E;
    use NodeStatus as S;

    let reject = || Err(NodeTransitionError { status, event });

    match (status, event) {
        (S::Pending, E::BecomeReady) => Ok(S::Ready),
        (S::Ready, E::StartProducing) => Ok(S::Producing),
        (S::Producing, E::ClaimSubmitted) => Ok(S::ClaimSubmitted),
        (S::ClaimSubmitted, E::StartVerifying) => Ok(S::Verifying),

        (S::Verifying, E::VerificationPassed) => Ok(S::Evaluating),
        (S::Verifying, E::VerificationFailedRepairable) => Ok(S::RepairRequired),
        (S::Verifying, E::VerificationInconclusive) => Ok(S::Inconclusive),
        (S::Verifying, E::VerificationProtocolViolation) => Ok(S::ProtocolViolation),

        (S::Evaluating, E::EvaluationAccepted) => Ok(S::Accepted),
        (S::Evaluating, E::EvaluationNeedsRepair) => Ok(S::RepairRequired),
        (S::Evaluating, E::EvaluationNeedsReplan) => Ok(S::ReplanRequired),
        (S::Evaluating, E::EvaluationNeedsHuman) => Ok(S::NeedsHuman),

        (S::RepairRequired, E::RepairReady) => Ok(S::Ready),
        (S::Inconclusive, E::RetryAfterInconclusive) => Ok(S::Ready),
        (S::ProtocolViolation, E::IsolateAndRetry) => Ok(S::Isolated),
        (S::Isolated, E::RepairReady) => Ok(S::Ready),
        (S::NeedsHuman, E::HumanDecisionRecorded) => Ok(S::Evaluating),

        _ => reject(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_path_accepts_in_order() {
        let mut status = NodeStatus::Pending;
        for event in [
            NodeEvent::BecomeReady,
            NodeEvent::StartProducing,
            NodeEvent::ClaimSubmitted,
            NodeEvent::StartVerifying,
            NodeEvent::VerificationPassed,
            NodeEvent::EvaluationAccepted,
        ] {
            status = apply(status, event).unwrap();
        }
        assert_eq!(status, NodeStatus::Accepted);
    }

    #[test]
    fn cannot_skip_verifying() {
        let status = NodeStatus::ClaimSubmitted;
        assert!(apply(status, NodeEvent::VerificationPassed).is_err());
    }

    #[test]
    fn inconclusive_with_no_safe_retry_stays_inconclusive_until_explicit_retry() {
        let status = NodeStatus::Inconclusive;
        // No arbitrary escape hatch: only RetryAfterInconclusive is legal.
        assert!(apply(status, NodeEvent::EvaluationAccepted).is_err());
        assert_eq!(
            apply(status, NodeEvent::RetryAfterInconclusive).unwrap(),
            NodeStatus::Ready
        );
    }

    #[test]
    fn protocol_violation_requires_isolation_before_retry() {
        let status = NodeStatus::ProtocolViolation;
        assert!(apply(status, NodeEvent::RepairReady).is_err());
        let isolated = apply(status, NodeEvent::IsolateAndRetry).unwrap();
        assert_eq!(isolated, NodeStatus::Isolated);
        assert_eq!(
            apply(isolated, NodeEvent::RepairReady).unwrap(),
            NodeStatus::Ready
        );
    }

    #[test]
    fn needs_human_returns_to_evaluating_not_accepted_directly() {
        let status = NodeStatus::NeedsHuman;
        let back = apply(status, NodeEvent::HumanDecisionRecorded).unwrap();
        assert_eq!(back, NodeStatus::Evaluating);
    }

    #[test]
    fn accepted_is_a_dead_end_in_this_reducer() {
        // Re-planning on Accepted nodes is handled at the Run/TaskGraph
        // level (§6.6): a new Run starts all nodes from Pending again.
        let status = NodeStatus::Accepted;
        for event in [
            NodeEvent::BecomeReady,
            NodeEvent::StartProducing,
            NodeEvent::EvaluationNeedsRepair,
        ] {
            assert!(apply(status, event).is_err());
        }
    }
}
