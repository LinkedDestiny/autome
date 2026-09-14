//! Project lifecycle per plan §6.1. Kept minimal: full ProjectHome / identity
//! machinery lands with the application service in `automed`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLifecycle {
    Active,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectPhase {
    Registered,
    Inspecting,
    AwaitingTrust,
    Initializing,
    ResolvingIntent,
    ResolvingConfig,
    CheckingEnvironmentAndSkills,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectHold {
    None,
    IdentityChanged,
    IntentUnresolved,
    ConfigInvalid,
    EnvironmentBlocked,
    SkillsBlocked,
    InitializationFailed,
}

/// Ordered nominal path from §6.1. Used to reject phase skips.
pub const PROJECT_NOMINAL_PATH: [ProjectPhase; 8] = [
    ProjectPhase::Registered,
    ProjectPhase::Inspecting,
    ProjectPhase::AwaitingTrust,
    ProjectPhase::Initializing,
    ProjectPhase::ResolvingIntent,
    ProjectPhase::ResolvingConfig,
    ProjectPhase::CheckingEnvironmentAndSkills,
    ProjectPhase::Ready,
];

impl ProjectPhase {
    pub fn next_nominal(self) -> Option<ProjectPhase> {
        let idx = PROJECT_NOMINAL_PATH.iter().position(|p| *p == self)?;
        PROJECT_NOMINAL_PATH.get(idx + 1).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectState {
    pub lifecycle: ProjectLifecycle,
    pub phase: ProjectPhase,
    pub hold: ProjectHold,
    /// Increments whenever repository/directory identity changes (§6.1:
    /// "identity 变化会递增 project revision 并要求 reinitialize"). Not a
    /// generic edit counter — only `ProjectEvent::IdentityChanged` advances
    /// it.
    pub revision: u32,
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Registered,
            hold: ProjectHold::None,
            revision: 1,
        }
    }

    /// A project may accept new Tasks / start new Runs only when active,
    /// Ready and not on hold (plan §6.1: hold 禁止创建新 Task 或启动新 Run).
    pub fn can_start_task(&self) -> bool {
        self.lifecycle == ProjectLifecycle::Active
            && self.phase == ProjectPhase::Ready
            && self.hold == ProjectHold::None
    }
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

/// Every transition arrow implied by §6.1. As with `run::apply`, this is an
/// exhaustive match so a new ProjectEvent variant without a handler is a
/// compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectEvent {
    /// Advance one step along the nominal path. Only legal when active,
    /// hold=None, and the current phase has a next nominal phase.
    AdvanceNominal,
    /// Repository/directory identity changed. Legal from any phase while
    /// active (plan §6.1 does not scope this to a single phase): arms
    /// hold=IdentityChanged, forces phase back to Initializing, and
    /// increments `revision`.
    IdentityChanged,
    /// Caller has completed reinitialization after an identity change.
    /// Only legal while hold=IdentityChanged; clears the hold so nominal
    /// advance from Initializing can resume.
    ReinitializationConfirmed,
    /// project_intent could not be resolved. Only legal at
    /// ResolvingIntent with no existing hold.
    IntentUnresolved,
    /// project_intent has been resolved. Only legal while
    /// hold=IntentUnresolved.
    IntentResolved,
    /// Effective config failed validation. Legal from any active,
    /// unheld state (mirrors `run::RunEvent::ConfigDriftDetected`, which
    /// is not scoped to a single phase either).
    ConfigInvalidated,
    /// Effective config has been revalidated. Only legal while
    /// hold=ConfigInvalid.
    ConfigRevalidated,
    /// Environment readiness check failed. Only legal at
    /// CheckingEnvironmentAndSkills with no existing hold.
    EnvironmentBlocked,
    /// Environment readiness has been confirmed. Only legal while
    /// hold=EnvironmentBlocked.
    EnvironmentUnblocked,
    /// Skill binding check failed. Only legal at
    /// CheckingEnvironmentAndSkills with no existing hold.
    SkillsBlocked,
    /// Skill bindings have been confirmed. Only legal while
    /// hold=SkillsBlocked.
    SkillsUnblocked,
    /// Initialization failed. Only legal at Initializing with no existing
    /// hold.
    InitializationFailed,
    /// Caller is retrying initialization. Only legal while
    /// hold=InitializationFailed.
    InitializationRetried,
    /// Archive the project. Legal from any active state. Callers must
    /// have already verified there is no non-terminal Run and no open
    /// Environment/Skill/Project transaction (plan §6.1: "存在非终态 Run 或
    /// Environment/Skill/Project transaction 时，archive 命令直接拒绝") —
    /// this reducer does not have visibility into Run/transaction state
    /// and so cannot check that predicate itself, exactly as
    /// `run::RunEvent::CompletionRecorded` assumes its precondition was
    /// already evaluated by the caller.
    Archived,
    /// Reactivate an archived project. Per §6.1 ("恢复时必须重新检查身份、
    /// 配置、环境和 Skills") this re-enters the recheck sequence from
    /// Initializing rather than resuming at the phase last held.
    Reactivated,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("project is archived; event {0:?} is not legal on an archived project")]
    ProjectIsArchived(ProjectEvent),
    #[error("event {event:?} is not legal from phase={phase:?} hold={hold:?}")]
    IllegalTransition {
        phase: ProjectPhase,
        hold: ProjectHold,
        event: ProjectEvent,
    },
}

pub fn apply(state: ProjectState, event: ProjectEvent) -> Result<ProjectState, TransitionError> {
    use ProjectEvent as E;
    use ProjectHold as H;
    use ProjectLifecycle as L;
    use ProjectPhase as P;

    if state.lifecycle == L::Archived && event != E::Reactivated {
        return Err(TransitionError::ProjectIsArchived(event));
    }

    let reject = || {
        Err(TransitionError::IllegalTransition {
            phase: state.phase,
            hold: state.hold,
            event,
        })
    };

    match event {
        E::AdvanceNominal => {
            if state.hold != H::None {
                return reject();
            }
            match state.phase.next_nominal() {
                Some(next) => Ok(ProjectState {
                    phase: next,
                    ..state
                }),
                None => reject(),
            }
        }
        E::IdentityChanged => Ok(ProjectState {
            phase: P::Initializing,
            hold: H::IdentityChanged,
            revision: state.revision + 1,
            ..state
        }),
        E::ReinitializationConfirmed => {
            if state.hold == H::IdentityChanged {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::IntentUnresolved => {
            if state.phase == P::ResolvingIntent && state.hold == H::None {
                Ok(ProjectState {
                    hold: H::IntentUnresolved,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::IntentResolved => {
            if state.hold == H::IntentUnresolved {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::ConfigInvalidated => {
            if state.hold == H::None {
                Ok(ProjectState {
                    hold: H::ConfigInvalid,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::ConfigRevalidated => {
            if state.hold == H::ConfigInvalid {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::EnvironmentBlocked => {
            if state.phase == P::CheckingEnvironmentAndSkills && state.hold == H::None {
                Ok(ProjectState {
                    hold: H::EnvironmentBlocked,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::EnvironmentUnblocked => {
            if state.hold == H::EnvironmentBlocked {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::SkillsBlocked => {
            if state.phase == P::CheckingEnvironmentAndSkills && state.hold == H::None {
                Ok(ProjectState {
                    hold: H::SkillsBlocked,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::SkillsUnblocked => {
            if state.hold == H::SkillsBlocked {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::InitializationFailed => {
            if state.phase == P::Initializing && state.hold == H::None {
                Ok(ProjectState {
                    hold: H::InitializationFailed,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::InitializationRetried => {
            if state.hold == H::InitializationFailed {
                Ok(ProjectState {
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
        E::Archived => Ok(ProjectState {
            lifecycle: L::Archived,
            ..state
        }),
        E::Reactivated => {
            if state.lifecycle == L::Archived {
                Ok(ProjectState {
                    lifecycle: L::Active,
                    phase: P::Initializing,
                    hold: H::None,
                    ..state
                })
            } else {
                reject()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_project_cannot_start_task() {
        assert!(!ProjectState::new().can_start_task());
    }

    #[test]
    fn ready_active_no_hold_can_start_task() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::None,
            revision: 1,
        };
        assert!(state.can_start_task());
    }

    #[test]
    fn ready_but_on_hold_cannot_start_task() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::EnvironmentBlocked,
            revision: 1,
        };
        assert!(!state.can_start_task());
    }

    #[test]
    fn archived_ready_project_cannot_start_task() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Archived,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::None,
            revision: 1,
        };
        assert!(!state.can_start_task());
    }

    #[test]
    fn nominal_path_is_linear_and_terminates_at_ready() {
        let mut phase = ProjectPhase::Registered;
        let mut steps = 0;
        while let Some(next) = phase.next_nominal() {
            phase = next;
            steps += 1;
            assert!(steps <= PROJECT_NOMINAL_PATH.len());
        }
        assert_eq!(phase, ProjectPhase::Ready);
    }

    #[test]
    fn advance_nominal_walks_the_full_path_to_ready() {
        let mut state = ProjectState::new();
        for _ in 0..PROJECT_NOMINAL_PATH.len() - 1 {
            state = apply(state, ProjectEvent::AdvanceNominal).unwrap();
        }
        assert_eq!(state.phase, ProjectPhase::Ready);
        assert!(apply(state, ProjectEvent::AdvanceNominal).is_err());
    }

    #[test]
    fn advance_nominal_is_blocked_by_any_hold() {
        let state = ProjectState {
            hold: ProjectHold::ConfigInvalid,
            ..ProjectState::new()
        };
        assert_eq!(
            apply(state, ProjectEvent::AdvanceNominal),
            Err(TransitionError::IllegalTransition {
                phase: ProjectPhase::Registered,
                hold: ProjectHold::ConfigInvalid,
                event: ProjectEvent::AdvanceNominal,
            })
        );
    }

    #[test]
    fn archived_project_rejects_advance_nominal() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Archived,
            ..ProjectState::new()
        };
        assert_eq!(
            apply(state, ProjectEvent::AdvanceNominal),
            Err(TransitionError::ProjectIsArchived(
                ProjectEvent::AdvanceNominal
            ))
        );
    }

    #[test]
    fn identity_changed_arms_hold_resets_phase_and_bumps_revision() {
        let state = ProjectState {
            phase: ProjectPhase::Ready,
            revision: 3,
            ..ProjectState::new()
        };
        let next = apply(state, ProjectEvent::IdentityChanged).unwrap();
        assert_eq!(next.phase, ProjectPhase::Initializing);
        assert_eq!(next.hold, ProjectHold::IdentityChanged);
        assert_eq!(next.revision, 4);
    }

    #[test]
    fn reinitialization_confirmed_only_legal_while_identity_changed() {
        let armed = apply(ProjectState::new(), ProjectEvent::IdentityChanged).unwrap();
        let cleared = apply(armed, ProjectEvent::ReinitializationConfirmed).unwrap();
        assert_eq!(cleared.hold, ProjectHold::None);

        let fresh = ProjectState::new();
        assert!(apply(fresh, ProjectEvent::ReinitializationConfirmed).is_err());
    }

    #[test]
    fn intent_unresolved_only_legal_at_resolving_intent_phase() {
        let too_early = ProjectState::new();
        assert!(apply(too_early, ProjectEvent::IntentUnresolved).is_err());

        let at_phase = ProjectState {
            phase: ProjectPhase::ResolvingIntent,
            ..ProjectState::new()
        };
        let held = apply(at_phase, ProjectEvent::IntentUnresolved).unwrap();
        assert_eq!(held.hold, ProjectHold::IntentUnresolved);

        let resolved = apply(held, ProjectEvent::IntentResolved).unwrap();
        assert_eq!(resolved.hold, ProjectHold::None);
        assert_eq!(resolved.phase, ProjectPhase::ResolvingIntent);
    }

    #[test]
    fn config_invalidated_and_revalidated_round_trip_from_any_unheld_phase() {
        let state = ProjectState {
            phase: ProjectPhase::Ready,
            ..ProjectState::new()
        };
        let invalidated = apply(state, ProjectEvent::ConfigInvalidated).unwrap();
        assert_eq!(invalidated.hold, ProjectHold::ConfigInvalid);
        let revalidated = apply(invalidated, ProjectEvent::ConfigRevalidated).unwrap();
        assert_eq!(revalidated.hold, ProjectHold::None);
        assert_eq!(revalidated.phase, ProjectPhase::Ready);
    }

    #[test]
    fn environment_and_skills_holds_only_legal_at_checking_phase() {
        let too_early = ProjectState::new();
        assert!(apply(too_early, ProjectEvent::EnvironmentBlocked).is_err());
        assert!(apply(ProjectState::new(), ProjectEvent::SkillsBlocked).is_err());

        let at_phase = ProjectState {
            phase: ProjectPhase::CheckingEnvironmentAndSkills,
            ..ProjectState::new()
        };
        let env_blocked = apply(at_phase, ProjectEvent::EnvironmentBlocked).unwrap();
        assert_eq!(env_blocked.hold, ProjectHold::EnvironmentBlocked);
        let env_unblocked = apply(env_blocked, ProjectEvent::EnvironmentUnblocked).unwrap();
        assert_eq!(env_unblocked.hold, ProjectHold::None);

        let at_phase = ProjectState {
            phase: ProjectPhase::CheckingEnvironmentAndSkills,
            ..ProjectState::new()
        };
        let skills_blocked = apply(at_phase, ProjectEvent::SkillsBlocked).unwrap();
        assert_eq!(skills_blocked.hold, ProjectHold::SkillsBlocked);
        let skills_unblocked = apply(skills_blocked, ProjectEvent::SkillsUnblocked).unwrap();
        assert_eq!(skills_unblocked.hold, ProjectHold::None);
    }

    #[test]
    fn initialization_failed_only_legal_at_initializing_and_retry_clears_it() {
        let wrong_phase = ProjectState {
            phase: ProjectPhase::Ready,
            ..ProjectState::new()
        };
        assert!(apply(wrong_phase, ProjectEvent::InitializationFailed).is_err());

        let at_phase = ProjectState {
            phase: ProjectPhase::Initializing,
            ..ProjectState::new()
        };
        let failed = apply(at_phase, ProjectEvent::InitializationFailed).unwrap();
        assert_eq!(failed.hold, ProjectHold::InitializationFailed);
        let retried = apply(failed, ProjectEvent::InitializationRetried).unwrap();
        assert_eq!(retried.hold, ProjectHold::None);
        assert_eq!(retried.phase, ProjectPhase::Initializing);
    }

    #[test]
    fn archived_then_reactivated_restarts_the_recheck_sequence() {
        let ready = ProjectState {
            phase: ProjectPhase::Ready,
            revision: 2,
            ..ProjectState::new()
        };
        let archived = apply(ready, ProjectEvent::Archived).unwrap();
        assert_eq!(archived.lifecycle, ProjectLifecycle::Archived);

        let reactivated = apply(archived, ProjectEvent::Reactivated).unwrap();
        assert_eq!(reactivated.lifecycle, ProjectLifecycle::Active);
        assert_eq!(reactivated.phase, ProjectPhase::Initializing);
        assert_eq!(reactivated.hold, ProjectHold::None);
        assert_eq!(reactivated.revision, 2);
    }

    #[test]
    fn reactivated_is_illegal_on_an_active_project() {
        assert!(apply(ProjectState::new(), ProjectEvent::Reactivated).is_err());
    }

    #[test]
    fn archived_project_rejects_every_active_only_event_except_reactivated() {
        let archived = apply(ProjectState::new(), ProjectEvent::Archived).unwrap();
        for event in [
            ProjectEvent::AdvanceNominal,
            ProjectEvent::IdentityChanged,
            ProjectEvent::IntentUnresolved,
            ProjectEvent::ConfigInvalidated,
            ProjectEvent::EnvironmentBlocked,
            ProjectEvent::SkillsBlocked,
            ProjectEvent::InitializationFailed,
            ProjectEvent::Archived,
        ] {
            assert_eq!(
                apply(archived, event),
                Err(TransitionError::ProjectIsArchived(event))
            );
        }
    }
}
