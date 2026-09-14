//! Run state machine per plan §6.2. This is the authoritative reducer: a pure
//! function from (RunState, RunEvent) to RunState or a rejection. No I/O, no
//! agent output can reach this module directly — callers in `automed` are
//! responsible for producing events only from verified sources (plan D2/D4).

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Received,
    ResolvingProjectContext,
    DiscoveringFacts,
    DraftingContract,
    ContractReview,
    PlanningGraph,
    GraphReview,
    CheckingReadiness,
    ContractFrozen,
    Ready,
    Executing,
    Repairing,
    Replanning,
    Integrating,
    FinalVerifying,
    FinalAuditing,
    DeliveryRehearsing,
    Delivering,
    DeliveredTreeChecking,
}

/// Ordered nominal path from §6.2.
pub const RUN_NOMINAL_PATH: [RunPhase; 19] = [
    RunPhase::Received,
    RunPhase::ResolvingProjectContext,
    RunPhase::DiscoveringFacts,
    RunPhase::DraftingContract,
    RunPhase::ContractReview,
    RunPhase::PlanningGraph,
    RunPhase::GraphReview,
    RunPhase::CheckingReadiness,
    RunPhase::ContractFrozen,
    RunPhase::Ready,
    RunPhase::Executing,
    RunPhase::Repairing,
    RunPhase::Replanning,
    RunPhase::Integrating,
    RunPhase::FinalVerifying,
    RunPhase::FinalAuditing,
    RunPhase::DeliveryRehearsing,
    RunPhase::Delivering,
    RunPhase::DeliveredTreeChecking,
];

/// The phases that participate in the *linear nominal* advance
/// (Received..=FinalAuditing minus the branch-only phases Repairing/Replanning),
/// per the "nominal path" line in §6.2 which skips Repairing/Replanning.
const RUN_LINEAR_NOMINAL_PATH: [RunPhase; 17] = [
    RunPhase::Received,
    RunPhase::ResolvingProjectContext,
    RunPhase::DiscoveringFacts,
    RunPhase::DraftingContract,
    RunPhase::ContractReview,
    RunPhase::PlanningGraph,
    RunPhase::GraphReview,
    RunPhase::CheckingReadiness,
    RunPhase::ContractFrozen,
    RunPhase::Ready,
    RunPhase::Executing,
    RunPhase::Integrating,
    RunPhase::FinalVerifying,
    RunPhase::FinalAuditing,
    RunPhase::DeliveryRehearsing,
    RunPhase::Delivering,
    RunPhase::DeliveredTreeChecking,
];

impl RunPhase {
    pub fn next_linear_nominal(self) -> Option<RunPhase> {
        let idx = RUN_LINEAR_NOMINAL_PATH.iter().position(|p| *p == self)?;
        RUN_LINEAR_NOMINAL_PATH.get(idx + 1).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockedReason {
    TargetChanged,
    DeliveryIntegrityMismatch,
    VerificationInconclusive,
    EnvironmentNotReady,
    MissingAuthoritativeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunHold {
    None,
    AwaitingClarification,
    AwaitingPlanApproval,
    AwaitingHumanAcceptance,
    AwaitingDeliveryApproval,
    AwaitingContractAmendment,
    AwaitingCorrectionClassification,
    AwaitingConfiguredHumanReview,
    ConfigurationInvalidated,
    Paused,
    Blocked(BlockedReason),
    BudgetExhausted,
    Stalled,
    UnknownOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunTerminal {
    None,
    Completed,
    Superseded,
    ProtocolFailed,
    Infeasible,
    Cancelled,
}

/// Where a readiness (re)check originated from, per §6.2's
/// "CheckingReadiness + same profile/capabilities + <origin>" rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadinessOrigin {
    NodeCandidate,
    PostIntegration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphReviewOrigin {
    InitialPlan,
    Replan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub phase: RunPhase,
    pub hold: RunHold,
    pub terminal: RunTerminal,
}

impl RunState {
    pub fn received() -> Self {
        Self {
            phase: RunPhase::Received,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal != RunTerminal::None
    }
}

/// Every transition arrow enumerated in plan §6.2. This is intentionally an
/// exhaustive match in `apply` below: adding a RunEvent variant without
/// handling it is a compile error, which is the mechanical guard against
/// silently-permissive transitions that D2 requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    /// Advance one step along the linear nominal path. Only legal with
    /// hold=None, terminal=None, and a phase that has a next nominal phase.
    AdvanceNominal,
    ContractReviewRejected,
    GraphReviewRejected {
        origin: GraphReviewOrigin,
    },
    ReadinessReady,
    PlanApproved,
    ReadinessInvalidated,
    ReadinessConfirmed {
        origin: ReadinessOrigin,
    },
    ReadinessCapabilityChanged,
    ConfiguredHumanReviewRequired,
    ConfiguredHumanReviewPassed,
    ConfigDriftDetected,
    RepairRequested,
    RepairCompleted,
    ReplanRequested,
    ReplanGraphDrafted,
    GraphReviewPassed {
        origin: GraphReviewOrigin,
    },
    FinalAuditPassed,
    DeliveryRehearsalPassed,
    DeliveryApproved,
    DeliveryReceiptWritten,
    CandidateChangedBeforeDelivery,
    TargetContextChanged,
    TargetWorktreeFingerprintChanged,
    DeliveredTreeMismatchObserved,
    DeliveryOutcomeUnknown,
    /// Only legal from DeliveredTreeChecking; callers must have already
    /// evaluated the full §7 completion predicate (see `completion` module)
    /// before emitting this event. The reducer does not compute the
    /// predicate itself so this module stays a pure state machine.
    CompletionRecorded,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("run is terminal ({0:?}); no further transitions are legal")]
    RunIsTerminal(RunTerminal),
    #[error("event {event:?} is not legal from phase={phase:?} hold={hold:?}")]
    IllegalTransition {
        phase: RunPhase,
        hold: RunHold,
        event: RunEvent,
    },
}

pub fn apply(state: RunState, event: RunEvent) -> Result<RunState, TransitionError> {
    if state.is_terminal() {
        return Err(TransitionError::RunIsTerminal(state.terminal));
    }

    let reject = || {
        Err(TransitionError::IllegalTransition {
            phase: state.phase,
            hold: state.hold,
            event,
        })
    };

    use RunHold as H;
    use RunPhase as P;

    match event {
        RunEvent::AdvanceNominal => {
            if state.hold != H::None {
                return reject();
            }
            match state.phase.next_linear_nominal() {
                Some(next) => Ok(RunState {
                    phase: next,
                    ..state
                }),
                None => reject(),
            }
        }
        RunEvent::ContractReviewRejected => {
            if state.phase == P::ContractReview && state.hold == H::None {
                Ok(RunState {
                    phase: P::DraftingContract,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::GraphReviewRejected { .. } => {
            if state.phase == P::GraphReview && state.hold == H::None {
                Ok(RunState {
                    phase: P::PlanningGraph,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ReadinessReady => {
            if state.phase == P::CheckingReadiness && state.hold == H::None {
                Ok(RunState {
                    hold: H::AwaitingPlanApproval,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::PlanApproved => {
            if state.phase == P::CheckingReadiness && state.hold == H::AwaitingPlanApproval {
                Ok(RunState {
                    phase: P::ContractFrozen,
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ReadinessInvalidated => {
            // Legal from any non-hold phase after ContractFrozen; drift can
            // be observed right up to delivery.
            if state.hold == H::None && phase_at_or_after_contract_frozen(state.phase) {
                Ok(RunState {
                    phase: P::CheckingReadiness,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ReadinessConfirmed { origin } => {
            if state.phase == P::CheckingReadiness && state.hold == H::None {
                let phase = match origin {
                    ReadinessOrigin::NodeCandidate => P::Executing,
                    ReadinessOrigin::PostIntegration => P::FinalVerifying,
                };
                Ok(RunState { phase, ..state })
            } else {
                reject()
            }
        }
        RunEvent::ReadinessCapabilityChanged => {
            if state.phase == P::CheckingReadiness && state.hold == H::None {
                Ok(RunState {
                    hold: H::AwaitingContractAmendment,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ConfiguredHumanReviewRequired => {
            if state.hold == H::None {
                Ok(RunState {
                    hold: H::AwaitingConfiguredHumanReview,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ConfiguredHumanReviewPassed => {
            if state.hold == H::AwaitingConfiguredHumanReview {
                Ok(RunState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ConfigDriftDetected => {
            if !state.is_terminal() {
                Ok(RunState {
                    hold: H::ConfigurationInvalidated,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::RepairRequested => {
            if matches!(
                state.phase,
                P::Executing
                    | P::Integrating
                    | P::FinalVerifying
                    | P::FinalAuditing
                    | P::DeliveryRehearsing
            ) && state.hold == H::None
            {
                Ok(RunState {
                    phase: P::Repairing,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::RepairCompleted => {
            if state.phase == P::Repairing && state.hold == H::None {
                Ok(RunState {
                    phase: P::Executing,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ReplanRequested => {
            if matches!(
                state.phase,
                P::Executing
                    | P::Integrating
                    | P::FinalVerifying
                    | P::FinalAuditing
                    | P::DeliveryRehearsing
            ) && state.hold == H::None
            {
                Ok(RunState {
                    phase: P::Replanning,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::ReplanGraphDrafted => {
            if state.phase == P::Replanning && state.hold == H::None {
                Ok(RunState {
                    phase: P::PlanningGraph,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::GraphReviewPassed { origin } => {
            if state.phase == P::GraphReview && state.hold == H::None {
                match origin {
                    GraphReviewOrigin::InitialPlan => Ok(RunState {
                        phase: P::CheckingReadiness,
                        ..state
                    }),
                    // Replan approval supersedes *this* Run; the new Run is
                    // constructed by the caller (automed), not this reducer.
                    GraphReviewOrigin::Replan => Ok(RunState {
                        terminal: RunTerminal::Superseded,
                        ..state
                    }),
                }
            } else {
                reject()
            }
        }
        RunEvent::FinalAuditPassed => {
            if state.phase == P::FinalAuditing && state.hold == H::None {
                Ok(RunState {
                    phase: P::DeliveryRehearsing,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::DeliveryRehearsalPassed => {
            if state.phase == P::DeliveryRehearsing && state.hold == H::None {
                Ok(RunState {
                    hold: H::AwaitingDeliveryApproval,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::DeliveryApproved => {
            if state.phase == P::DeliveryRehearsing && state.hold == H::AwaitingDeliveryApproval {
                Ok(RunState {
                    phase: P::Delivering,
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::DeliveryReceiptWritten => {
            if state.phase == P::Delivering && state.hold == H::None {
                Ok(RunState {
                    phase: P::DeliveredTreeChecking,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::CandidateChangedBeforeDelivery => {
            if matches!(
                state.phase,
                P::FinalVerifying
                    | P::FinalAuditing
                    | P::DeliveryRehearsing
                    | P::Delivering
                    | P::DeliveredTreeChecking
            ) {
                Ok(RunState {
                    phase: P::Integrating,
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::TargetContextChanged => {
            if !state.is_terminal() {
                Ok(RunState {
                    hold: H::AwaitingContractAmendment,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::TargetWorktreeFingerprintChanged => {
            if !state.is_terminal() {
                Ok(RunState {
                    hold: H::Blocked(BlockedReason::TargetChanged),
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::DeliveredTreeMismatchObserved => {
            if state.phase == P::DeliveredTreeChecking {
                Ok(RunState {
                    hold: H::Blocked(BlockedReason::DeliveryIntegrityMismatch),
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::DeliveryOutcomeUnknown => {
            if state.phase == P::Delivering {
                Ok(RunState {
                    hold: H::UnknownOutcome,
                    ..state
                })
            } else {
                reject()
            }
        }
        RunEvent::CompletionRecorded => {
            if state.phase == P::DeliveredTreeChecking && state.hold == H::None {
                Ok(RunState {
                    terminal: RunTerminal::Completed,
                    ..state
                })
            } else {
                reject()
            }
        }
    }
}

fn phase_at_or_after_contract_frozen(phase: RunPhase) -> bool {
    let frozen_idx = RUN_LINEAR_NOMINAL_PATH
        .iter()
        .position(|p| *p == RunPhase::ContractFrozen)
        .expect("ContractFrozen is in the linear nominal path");
    match RUN_LINEAR_NOMINAL_PATH.iter().position(|p| *p == phase) {
        Some(idx) => idx >= frozen_idx,
        None => matches!(phase, RunPhase::Repairing | RunPhase::Replanning),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_run_rejects_every_event() {
        let state = RunState {
            phase: RunPhase::Executing,
            hold: RunHold::None,
            terminal: RunTerminal::Cancelled,
        };
        let err = apply(state, RunEvent::AdvanceNominal).unwrap_err();
        assert_eq!(err, TransitionError::RunIsTerminal(RunTerminal::Cancelled));
    }

    #[test]
    fn cannot_skip_phases_via_advance_nominal() {
        // Received -> Executing directly is not a legal single advance.
        let state = RunState::received();
        let after_one = apply(state, RunEvent::AdvanceNominal).unwrap();
        assert_eq!(after_one.phase, RunPhase::ResolvingProjectContext);
        assert_ne!(after_one.phase, RunPhase::Executing);
    }

    #[test]
    fn contract_review_reject_loops_back_to_drafting() {
        let state = RunState {
            phase: RunPhase::ContractReview,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let after = apply(state, RunEvent::ContractReviewRejected).unwrap();
        assert_eq!(after.phase, RunPhase::DraftingContract);
    }

    #[test]
    fn plan_approval_requires_awaiting_plan_approval_hold() {
        let state = RunState {
            phase: RunPhase::CheckingReadiness,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        // No prior ReadinessReady: approval must be rejected.
        assert!(apply(state, RunEvent::PlanApproved).is_err());

        let ready = apply(state, RunEvent::ReadinessReady).unwrap();
        assert_eq!(ready.hold, RunHold::AwaitingPlanApproval);

        let frozen = apply(ready, RunEvent::PlanApproved).unwrap();
        assert_eq!(frozen.phase, RunPhase::ContractFrozen);
        assert_eq!(frozen.hold, RunHold::None);
    }

    #[test]
    fn readiness_confirmed_routes_by_origin() {
        let state = RunState {
            phase: RunPhase::CheckingReadiness,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let node = apply(
            state,
            RunEvent::ReadinessConfirmed {
                origin: ReadinessOrigin::NodeCandidate,
            },
        )
        .unwrap();
        assert_eq!(node.phase, RunPhase::Executing);

        let post = apply(
            state,
            RunEvent::ReadinessConfirmed {
                origin: ReadinessOrigin::PostIntegration,
            },
        )
        .unwrap();
        assert_eq!(post.phase, RunPhase::FinalVerifying);
    }

    #[test]
    fn repair_loop_returns_to_executing_not_forward() {
        let state = RunState {
            phase: RunPhase::FinalAuditing,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let repairing = apply(state, RunEvent::RepairRequested).unwrap();
        assert_eq!(repairing.phase, RunPhase::Repairing);
        let executing = apply(repairing, RunEvent::RepairCompleted).unwrap();
        assert_eq!(executing.phase, RunPhase::Executing);
    }

    #[test]
    fn replan_approval_from_replan_origin_supersedes_run() {
        let state = RunState {
            phase: RunPhase::GraphReview,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let after = apply(
            state,
            RunEvent::GraphReviewPassed {
                origin: GraphReviewOrigin::Replan,
            },
        )
        .unwrap();
        assert_eq!(after.terminal, RunTerminal::Superseded);
    }

    #[test]
    fn initial_plan_graph_review_pass_goes_to_readiness_not_superseded() {
        let state = RunState {
            phase: RunPhase::GraphReview,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let after = apply(
            state,
            RunEvent::GraphReviewPassed {
                origin: GraphReviewOrigin::InitialPlan,
            },
        )
        .unwrap();
        assert_eq!(after.phase, RunPhase::CheckingReadiness);
        assert_eq!(after.terminal, RunTerminal::None);
    }

    #[test]
    fn delivery_integrity_mismatch_is_not_unknown_outcome() {
        let state = RunState {
            phase: RunPhase::DeliveredTreeChecking,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let after = apply(state, RunEvent::DeliveredTreeMismatchObserved).unwrap();
        assert_eq!(
            after.hold,
            RunHold::Blocked(BlockedReason::DeliveryIntegrityMismatch)
        );
    }

    #[test]
    fn completion_only_legal_from_delivered_tree_checking_with_no_hold() {
        let wrong_phase = RunState {
            phase: RunPhase::FinalAuditing,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        assert!(apply(wrong_phase, RunEvent::CompletionRecorded).is_err());

        let on_hold = RunState {
            phase: RunPhase::DeliveredTreeChecking,
            hold: RunHold::Blocked(BlockedReason::DeliveryIntegrityMismatch),
            terminal: RunTerminal::None,
        };
        assert!(apply(on_hold, RunEvent::CompletionRecorded).is_err());

        let ready = RunState {
            phase: RunPhase::DeliveredTreeChecking,
            hold: RunHold::None,
            terminal: RunTerminal::None,
        };
        let completed = apply(ready, RunEvent::CompletionRecorded).unwrap();
        assert_eq!(completed.terminal, RunTerminal::Completed);
    }

    #[test]
    fn full_nominal_walk_reaches_delivered_tree_checking() {
        let mut state = RunState::received();
        while let Ok(next) = apply(state, RunEvent::AdvanceNominal) {
            state = next;
        }
        assert_eq!(state.phase, RunPhase::DeliveredTreeChecking);
    }
}
