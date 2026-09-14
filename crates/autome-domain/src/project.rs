//! Project lifecycle per plan §6.1. Kept minimal: full ProjectHome / identity
//! machinery lands with the application service in `automed`.

use serde::{Deserialize, Serialize};

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
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Registered,
            hold: ProjectHold::None,
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
        };
        assert!(state.can_start_task());
    }

    #[test]
    fn ready_but_on_hold_cannot_start_task() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::EnvironmentBlocked,
        };
        assert!(!state.can_start_task());
    }

    #[test]
    fn archived_ready_project_cannot_start_task() {
        let state = ProjectState {
            lifecycle: ProjectLifecycle::Archived,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::None,
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
}
