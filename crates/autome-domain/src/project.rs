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

/// §5.1: `kind(new_product|existing_repository)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectKind {
    NewProduct,
    ExistingRepository,
}

/// §5.1: `locator = GreenfieldDestination { parent_identity, destination }
/// | ExistingRepository { repository_identity }`. Exactly one variant per
/// `ProjectKind` — `ProjectIdentity::new` enforces the pairing so a
/// `NewProduct` project can never carry an `ExistingRepository` locator or
/// vice versa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLocator {
    GreenfieldDestination {
        parent_identity: String,
        destination: String,
    },
    ExistingRepository {
        repository_identity: String,
    },
}

/// §5.1 identity fields that are fixed at creation and do not change with
/// `ProjectState` transitions: `id · display_name · kind · locator ·
/// project_home`. Deliberately excludes `initialization_receipt_id`,
/// `active_intent_revision`, `intent_hash`,
/// `active_config_override_revision` and `skill_binding_revision` — those
/// all reference aggregates (`ProjectIntentRevision`,
/// `ProjectInitializationReceipt`, config/skill bindings) that are not yet
/// persisted anywhere in this workspace; adding the fields now would only
/// produce permanently-`None` holes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    pub id: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub locator: ProjectLocator,
    pub project_home: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectIdentityError {
    MissingId,
    MissingDisplayName,
    MissingProjectHome,
    /// `kind` and `locator` named different project types (e.g.
    /// `NewProduct` with an `ExistingRepository` locator). §2.1 pins these
    /// as a strict one-to-one pairing; a mismatched triple is a protocol
    /// error, not a value this constructor can silently coerce.
    KindLocatorMismatch {
        kind: ProjectKind,
    },
    MissingLocatorField {
        field: &'static str,
    },
}

impl ProjectIdentity {
    pub fn new(
        id: &str,
        display_name: &str,
        kind: ProjectKind,
        locator: ProjectLocator,
        project_home: &str,
    ) -> Result<Self, ProjectIdentityError> {
        if id.trim().is_empty() {
            return Err(ProjectIdentityError::MissingId);
        }
        if display_name.trim().is_empty() {
            return Err(ProjectIdentityError::MissingDisplayName);
        }
        if project_home.trim().is_empty() {
            return Err(ProjectIdentityError::MissingProjectHome);
        }
        match (&kind, &locator) {
            (
                ProjectKind::NewProduct,
                ProjectLocator::GreenfieldDestination {
                    parent_identity,
                    destination,
                },
            ) => {
                if parent_identity.trim().is_empty() {
                    return Err(ProjectIdentityError::MissingLocatorField {
                        field: "parent_identity",
                    });
                }
                if destination.trim().is_empty() {
                    return Err(ProjectIdentityError::MissingLocatorField {
                        field: "destination",
                    });
                }
            }
            (
                ProjectKind::ExistingRepository,
                ProjectLocator::ExistingRepository {
                    repository_identity,
                },
            ) => {
                if repository_identity.trim().is_empty() {
                    return Err(ProjectIdentityError::MissingLocatorField {
                        field: "repository_identity",
                    });
                }
            }
            _ => return Err(ProjectIdentityError::KindLocatorMismatch { kind }),
        }

        Ok(Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind,
            locator,
            project_home: project_home.to_string(),
        })
    }
}

/// A candidate target's raw, orthogonal facts as observed by
/// `automed::target_probe` (git presence, HEAD resolvability, worktree
/// cleanliness, destination absence). Pure data — the probing itself is I/O
/// and lives in `automed`, mirroring the split this module's doc comment
/// already states between this crate (no I/O) and the application service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetInspection {
    pub is_git_repo: bool,
    pub head_resolvable: bool,
    pub worktree_clean: bool,
    pub destination_absent: bool,
}

/// Why a candidate target could not be turned into a `ProjectLocator`.
/// §2.2: "不支持非 Git 的现有代码库"; §8.1: "现有仓库要求存在可解析 HEAD；
/// 默认要求基线工作树干净"; §2.1: `new_product` 的初始化输入是"项目名 +
/// 尚不存在的目标目录".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRejection {
    NotAGitRepository,
    UnbornOrUnresolvableHead,
    DirtyWorktree,
    DestinationAlreadyExists,
    InvalidDestinationName,
}

/// A single path component: non-empty, no path separators, not `.`/`..`, no
/// NUL, bounded length. The greenfield `destination` is the one path-shaped
/// value the Renderer is allowed to type directly (never a path the OS
/// picker returned) — it must never be usable to escape the parent
/// directory the picker already fixed.
fn validate_destination_name(name: &str) -> Result<(), TargetRejection> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.chars().count() > 255
    {
        return Err(TargetRejection::InvalidDestinationName);
    }
    Ok(())
}

/// Derives a `ProjectLocator` from orthogonal, already-probed facts about a
/// candidate target. Pure judgment only — never touches a filesystem or
/// spawns `git` itself (that is `automed::target_probe`'s job). Enforces
/// §2.1's strict kind↔locator pairing the same way `ProjectIdentity::new`
/// does, plus the three rejections named on `TargetRejection` above.
/// `identity` is `parent_identity` for `NewProduct` and
/// `repository_identity` for `ExistingRepository` — already computed by the
/// caller from a `TargetIdentityProbe`, not derived here.
pub fn locator_for(
    kind: ProjectKind,
    identity: &str,
    destination_name: Option<&str>,
    inspection: TargetInspection,
) -> Result<ProjectLocator, TargetRejection> {
    match kind {
        ProjectKind::NewProduct => {
            let destination = destination_name.unwrap_or("");
            validate_destination_name(destination)?;
            if !inspection.destination_absent {
                return Err(TargetRejection::DestinationAlreadyExists);
            }
            Ok(ProjectLocator::GreenfieldDestination {
                parent_identity: identity.to_string(),
                destination: destination.to_string(),
            })
        }
        ProjectKind::ExistingRepository => {
            if !inspection.is_git_repo {
                return Err(TargetRejection::NotAGitRepository);
            }
            if !inspection.head_resolvable {
                return Err(TargetRejection::UnbornOrUnresolvableHead);
            }
            if !inspection.worktree_clean {
                return Err(TargetRejection::DirtyWorktree);
            }
            Ok(ProjectLocator::ExistingRepository {
                repository_identity: identity.to_string(),
            })
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

    #[test]
    fn valid_new_product_identity_round_trips() {
        let identity = ProjectIdentity::new(
            "proj-1",
            "演示项目",
            ProjectKind::NewProduct,
            ProjectLocator::GreenfieldDestination {
                parent_identity: "parent-hash".to_string(),
                destination: "/tmp/demo".to_string(),
            },
            "/Library/Application Support/autome/projects/proj-1",
        )
        .unwrap();
        assert_eq!(identity.id, "proj-1");
        assert_eq!(identity.kind, ProjectKind::NewProduct);
    }

    #[test]
    fn valid_existing_repository_identity_round_trips() {
        let identity = ProjectIdentity::new(
            "proj-2",
            "既有仓库项目",
            ProjectKind::ExistingRepository,
            ProjectLocator::ExistingRepository {
                repository_identity: "repo-hash".to_string(),
            },
            "/Library/Application Support/autome/projects/proj-2",
        )
        .unwrap();
        assert_eq!(identity.kind, ProjectKind::ExistingRepository);
    }

    #[test]
    fn new_product_kind_rejects_existing_repository_locator() {
        let result = ProjectIdentity::new(
            "proj-3",
            "不匹配",
            ProjectKind::NewProduct,
            ProjectLocator::ExistingRepository {
                repository_identity: "repo-hash".to_string(),
            },
            "/tmp/home",
        );
        assert_eq!(
            result,
            Err(ProjectIdentityError::KindLocatorMismatch {
                kind: ProjectKind::NewProduct
            })
        );
    }

    #[test]
    fn existing_repository_kind_rejects_greenfield_locator() {
        let result = ProjectIdentity::new(
            "proj-4",
            "不匹配",
            ProjectKind::ExistingRepository,
            ProjectLocator::GreenfieldDestination {
                parent_identity: "parent-hash".to_string(),
                destination: "/tmp/demo".to_string(),
            },
            "/tmp/home",
        );
        assert_eq!(
            result,
            Err(ProjectIdentityError::KindLocatorMismatch {
                kind: ProjectKind::ExistingRepository
            })
        );
    }

    #[test]
    fn blank_fields_are_rejected() {
        assert_eq!(
            ProjectIdentity::new(
                "  ",
                "name",
                ProjectKind::ExistingRepository,
                ProjectLocator::ExistingRepository {
                    repository_identity: "repo-hash".to_string()
                },
                "/tmp/home",
            ),
            Err(ProjectIdentityError::MissingId)
        );
        assert_eq!(
            ProjectIdentity::new(
                "proj-5",
                "  ",
                ProjectKind::ExistingRepository,
                ProjectLocator::ExistingRepository {
                    repository_identity: "repo-hash".to_string()
                },
                "/tmp/home",
            ),
            Err(ProjectIdentityError::MissingDisplayName)
        );
        assert_eq!(
            ProjectIdentity::new(
                "proj-6",
                "name",
                ProjectKind::ExistingRepository,
                ProjectLocator::ExistingRepository {
                    repository_identity: "repo-hash".to_string()
                },
                "  ",
            ),
            Err(ProjectIdentityError::MissingProjectHome)
        );
        assert_eq!(
            ProjectIdentity::new(
                "proj-7",
                "name",
                ProjectKind::ExistingRepository,
                ProjectLocator::ExistingRepository {
                    repository_identity: "  ".to_string()
                },
                "/tmp/home",
            ),
            Err(ProjectIdentityError::MissingLocatorField {
                field: "repository_identity"
            })
        );
    }

    #[test]
    fn locator_for_greenfield_accepts_absent_destination() {
        let inspection = TargetInspection {
            is_git_repo: false,
            head_resolvable: false,
            worktree_clean: false,
            destination_absent: true,
        };
        let locator = locator_for(
            ProjectKind::NewProduct,
            "parent-hash",
            Some("my-new-app"),
            inspection,
        )
        .unwrap();
        assert_eq!(
            locator,
            ProjectLocator::GreenfieldDestination {
                parent_identity: "parent-hash".to_string(),
                destination: "my-new-app".to_string(),
            }
        );
    }

    #[test]
    fn locator_for_greenfield_rejects_existing_destination() {
        let inspection = TargetInspection {
            is_git_repo: false,
            head_resolvable: false,
            worktree_clean: false,
            destination_absent: false,
        };
        assert_eq!(
            locator_for(
                ProjectKind::NewProduct,
                "parent-hash",
                Some("taken"),
                inspection
            ),
            Err(TargetRejection::DestinationAlreadyExists)
        );
    }

    #[test]
    fn locator_for_greenfield_rejects_path_shaped_destination_name() {
        let inspection = TargetInspection {
            is_git_repo: false,
            head_resolvable: false,
            worktree_clean: false,
            destination_absent: true,
        };
        for bad in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert_eq!(
                locator_for(
                    ProjectKind::NewProduct,
                    "parent-hash",
                    Some(bad),
                    inspection
                ),
                Err(TargetRejection::InvalidDestinationName),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn locator_for_existing_repository_accepts_clean_resolvable_repo() {
        let inspection = TargetInspection {
            is_git_repo: true,
            head_resolvable: true,
            worktree_clean: true,
            destination_absent: false,
        };
        let locator = locator_for(
            ProjectKind::ExistingRepository,
            "repo-hash",
            None,
            inspection,
        )
        .unwrap();
        assert_eq!(
            locator,
            ProjectLocator::ExistingRepository {
                repository_identity: "repo-hash".to_string(),
            }
        );
    }

    #[test]
    fn locator_for_existing_repository_rejects_non_git_directory() {
        let inspection = TargetInspection {
            is_git_repo: false,
            head_resolvable: false,
            worktree_clean: true,
            destination_absent: false,
        };
        assert_eq!(
            locator_for(
                ProjectKind::ExistingRepository,
                "repo-hash",
                None,
                inspection
            ),
            Err(TargetRejection::NotAGitRepository)
        );
    }

    #[test]
    fn locator_for_existing_repository_rejects_unborn_head() {
        let inspection = TargetInspection {
            is_git_repo: true,
            head_resolvable: false,
            worktree_clean: true,
            destination_absent: false,
        };
        assert_eq!(
            locator_for(
                ProjectKind::ExistingRepository,
                "repo-hash",
                None,
                inspection
            ),
            Err(TargetRejection::UnbornOrUnresolvableHead)
        );
    }

    #[test]
    fn locator_for_existing_repository_rejects_dirty_worktree() {
        let inspection = TargetInspection {
            is_git_repo: true,
            head_resolvable: true,
            worktree_clean: false,
            destination_absent: false,
        };
        assert_eq!(
            locator_for(
                ProjectKind::ExistingRepository,
                "repo-hash",
                None,
                inspection
            ),
            Err(TargetRejection::DirtyWorktree)
        );
    }
}
