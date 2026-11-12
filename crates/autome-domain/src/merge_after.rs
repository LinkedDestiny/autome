//! Pure eligibility decision for the host's after-merge command.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainVerdict {
    NotRun,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Run,
    Skip { reason: &'static str },
}

pub fn plan(workspace: bool, has_merge_commit: bool, main: MainVerdict) -> Plan {
    let reason = if workspace {
        "工作区项目不支持合并后命令"
    } else if !has_merge_commit {
        "没有单一的合并提交"
    } else {
        match main {
            MainVerdict::Failed => "验证 main 未通过",
            MainVerdict::Skipped => "验证 main 被跳过",
            MainVerdict::NotRun | MainVerdict::Passed => return Plan::Run,
        }
    };
    Plan::Skip { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_after_runs_when_the_main_check_passed_or_was_not_configured() {
        for main in [MainVerdict::NotRun, MainVerdict::Passed] {
            assert_eq!(plan(false, true, main), Plan::Run);
        }
    }

    #[test]
    fn merge_after_is_skipped_when_the_main_check_failed_or_was_skipped() {
        for (main, reason) in [
            (MainVerdict::Failed, "验证 main 未通过"),
            (MainVerdict::Skipped, "验证 main 被跳过"),
        ] {
            assert_eq!(plan(false, true, main), Plan::Skip { reason });
        }
    }

    #[test]
    fn merge_after_is_skipped_in_a_workspace_or_without_a_merge_commit() {
        for main in [
            MainVerdict::NotRun,
            MainVerdict::Passed,
            MainVerdict::Failed,
            MainVerdict::Skipped,
        ] {
            for has_commit in [false, true] {
                assert_eq!(
                    plan(true, has_commit, main),
                    Plan::Skip {
                        reason: "工作区项目不支持合并后命令"
                    }
                );
            }
            assert_eq!(
                plan(false, false, main),
                Plan::Skip {
                    reason: "没有单一的合并提交"
                }
            );
        }
    }
}
