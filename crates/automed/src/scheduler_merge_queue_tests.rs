// Real worktrees, ledger, host jobs, and scheduler transitions.
fn mq_world(tag: &str) -> World {
    let w = queue_world(tag);
    let path = config_io::project_config_path(&w.repo);
    let text = std::fs::read_to_string(&path).unwrap() + "\n[merge]\nauto = true\n";
    std::fs::write(path, text).unwrap();
    git::commit_paths(&w.repo, &[".autome/config.toml"], "config").unwrap();
    w
}
fn mq_task(w: &mut World, id: &str) -> TaskRecord {
    let t = queue_task(w, id);
    let wt = w.repo.join(".worktree").join(id);
    git::commit_paths(&wt, &[&t.doc_dir()], "task").unwrap();
    w.set_state(
        id,
        TaskState::Active {
            node: Node::MergeGate,
        },
    );
    w.ctx.store.get_task(id).unwrap()
}
fn mq_move(w: &World) {
    std::fs::write(w.repo.join("new-main"), "new\n").unwrap();
    git::commit_paths(&w.repo, &["new-main"], "main moved").unwrap();
}
fn mq_complete(w: &mut World, t: &TaskRecord) {
    queue_wait(|| {
        queue_run(w, t);
        w.state(&t.id) != t.state
    });
}
fn mq_seq(w: &World, t: &TaskRecord, kind: &str) -> u64 {
    w.ctx
        .store
        .last_event_of(&t.id, &[kind])
        .unwrap()
        .unwrap()
        .0
}
#[test]
fn merge_queue_a_dequeued_gate_rebases_onto_the_moved_main_first() {
    let mut w = mq_world("mq-dequeue");
    let a = queue_task(&mut w, "A");
    let b = mq_task(&mut w, "B");
    let blocker = queue_job(&w, &a, "gate", "block", None);
    let before = git::head_sha(&w.repo.join(".worktree/B")).unwrap();
    queue_run(&mut w, &b);
    mq_move(&w);
    queue_run(&mut w, &b);
    assert_eq!(git::head_sha(&w.repo.join(".worktree/B")).unwrap(), before);
    assert_eq!(queue_count(&w, &b, "merge.rebased"), 0);
    blocker.clear();
    queue_run(&mut w, &b);
    assert!(git::is_ancestor(&w.repo, "main", &b.branch()).unwrap());
    assert_eq!(queue_count(&w, &b, "gate.started"), 1);
    assert!(mq_seq(&w, &b, "merge.rebased") < mq_seq(&w, &b, "gate.started"));
    let event = w
        .ctx
        .store
        .last_event(&b.id, "merge.rebased")
        .unwrap()
        .unwrap();
    assert_eq!(event["at"], "dequeue");
    assert_eq!(
        event["repos"][0]["base_sha"],
        git::head_sha(&w.repo).unwrap()
    );
}
#[test]
fn merge_queue_an_up_to_date_branch_is_not_rebased_at_dequeue() {
    let mut w = mq_world("mq-current");
    let b = mq_task(&mut w, "B");
    queue_run(&mut w, &b);
    mq_complete(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "merge.rebased"), 0);
    assert_eq!(
        w.state(&b.id),
        TaskState::Active {
            node: Node::Merging
        }
    );
    assert_eq!(queue_count(&w, &b, "merge.auto"), 0);
    assert_eq!(queue_count(&w, &b, "merge.direct"), 1);
    let mut w = mq_world("mq-source");
    let b = mq_task(&mut w, "B");
    w.ctx
        .store
        .append_event("merge.auto", &b.id, json!({"active":true,"by":"behind"}))
        .unwrap();
    queue_run(&mut w, &b);
    mq_complete(&mut w, &b);
    assert_eq!(
        w.ctx
            .store
            .last_event(&b.id, "merge.direct")
            .unwrap()
            .unwrap()["by"],
        "behind"
    );
}
#[test]
fn merge_queue_reuse_requires_an_up_to_date_branch() {
    for behind in [false, true] {
        let mut w = mq_world("mq-reuse");
        let b = mq_task(&mut w, "B");
        let head = git::head_sha(&w.repo.join(".worktree/B")).unwrap();
        w.ctx
            .store
            .append_event("gate.result", &b.id, json!({"ok":true,"sha":head}))
            .unwrap();
        if behind {
            mq_move(&w);
        }
        queue_run(&mut w, &b);
        assert_eq!(queue_count(&w, &b, "gate.reused"), u32::from(!behind));
        assert_eq!(queue_count(&w, &b, "gate.started"), u32::from(behind));
        assert!(crate::merge_queue::holds(&w.ctx, &w.ctx.store.get_task(&b.id).unwrap()).unwrap());
    } // Read failures fail closed, before a host job is launched.
    let mut w = mq_world("mq-read-failure");
    let b = mq_task(&mut w, "B");
    let wt = w.repo.join(".worktree/B");
    let saved = w.root.join("saved-worktree");
    std::fs::rename(&wt, &saved).unwrap();
    assert!(run_core_step(&mut w.ctx, &b, Node::MergeGate).is_err());
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    std::fs::rename(&saved, &wt).unwrap();
    let config = config_io::project_config_path(&w.repo);
    let saved = w.root.join("saved-config");
    std::fs::rename(&config, &saved).unwrap();
    std::fs::create_dir(&config).unwrap();
    assert!(run_core_step(&mut w.ctx, &b, Node::MergeGate).is_err());
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    std::fs::remove_dir(&config).unwrap();
    std::fs::rename(&saved, &config).unwrap();
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.started"), 1);
}
#[test]
fn merge_queue_a_second_merge_gate_waits_outside_the_host_queue_while_the_section_is_held() {
    let mut w = mq_world("mq-wait");
    let a = mq_task(&mut w, "A");
    let b = mq_task(&mut w, "B");
    queue_run(&mut w, &a);
    for _ in 0..3 {
        queue_run(&mut w, &b);
    }
    assert_eq!(queue_count(&w, &b, "gate.queued"), 1);
    assert_eq!(queue_count(&w, &b, "merge.waiting"), 1);
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    assert!(crate::gate_queue::waiters(&w.ctx).unwrap().is_empty());
    mq_complete(&mut w, &a);
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    let a = w.ctx.store.get_task(&a.id).unwrap();
    run_core_step(&mut w.ctx, &a, Node::Merging).unwrap();
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.started"), 1);
}
#[test]
fn merge_queue_a_merge_waits_while_another_task_holds_the_section() {
    let mut w = mq_world("mq-merge-wait");
    let a = mq_task(&mut w, "A");
    let b = mq_task(&mut w, "B");
    w.set_state(
        &b.id,
        TaskState::Active {
            node: Node::Merging,
        },
    );
    let b = w.ctx.store.get_task(&b.id).unwrap();
    queue_run(&mut w, &a);
    let head = git::head_sha(&w.repo).unwrap();
    run_core_step(&mut w.ctx, &b, Node::Merging).unwrap();
    assert_eq!(git::head_sha(&w.repo).unwrap(), head);
    assert_eq!(queue_count(&w, &b, "merge.waiting"), 1);
    advance(
        &mut w.ctx,
        &a.id,
        &Trigger::GateDone {
            status: None,
            node: Node::MergeGate,
            outcome: GateOutcome::Failed {
                head: "head".into(),
                command: "false".into(),
                tail: "test".into(),
            },
        },
    )
    .unwrap();
    run_core_step(&mut w.ctx, &b, Node::Merging).unwrap();
    assert_ne!(git::head_sha(&w.repo).unwrap(), head);
}
#[test]
fn merge_queue_every_exit_releases_the_section() {
    let triggers = vec![
        Trigger::GateSameFailure {
            node: Node::MergeGate,
            count: 3,
            summary: "same".into(),
            log: "test".into(),
        },
        Trigger::Pause,
        Trigger::Stop,
        Trigger::Cancel,
        Trigger::GateDone {
            status: None,
            node: Node::MergeGate,
            outcome: GateOutcome::Failed {
                head: "head".into(),
                command: "false".into(),
                tail: "test".into(),
            },
        },
        Trigger::SetupFailed {
            command: "false".into(),
            exit_code: 1,
            log: "test".into(),
        },
        Trigger::CoreStepDone {
            node: Node::MergeGate,
            result: CoreStepResult::Conflict {
                files: vec!["x".into()],
                detail: "conflict".into(),
            },
        },
    ];
    for trigger in triggers {
        let mut w = mq_world("mq-release");
        let a = mq_task(&mut w, "A");
        crate::merge_queue::enter(&mut w.ctx, &a).unwrap();
        advance(&mut w.ctx, &a.id, &trigger).unwrap();
        assert_eq!(
            crate::merge_queue::holder(&w.ctx, &w.project_id).unwrap(),
            None,
            "{trigger:?}"
        );
        assert_eq!(queue_count(&w, &a, "merge.unlocked"), 1);
    }
    for result in [
        CoreStepResult::Ok,
        CoreStepResult::Blocked {
            detail: "dirty".into(),
        },
        CoreStepResult::Behind {
            repos: vec![],
            detail: "behind".into(),
        },
        CoreStepResult::Failed {
            detail: "failure".into(),
        },
    ] {
        let mut w = mq_world("mq-merge-release");
        let a = mq_task(&mut w, "A");
        crate::merge_queue::enter(&mut w.ctx, &a).unwrap();
        w.set_state(
            &a.id,
            TaskState::Active {
                node: Node::Merging,
            },
        );
        advance(
            &mut w.ctx,
            &a.id,
            &Trigger::CoreStepDone {
                node: Node::Merging,
                result,
            },
        )
        .unwrap();
        assert_eq!(queue_count(&w, &a, "merge.unlocked"), 1);
    }
    for mode in [
        "vanished",
        "head_changed",
        "removed_gate",
        "revoked",
        "timeout",
    ] {
        let mut w = mq_world("mq-retain");
        let a = mq_task(&mut w, "A");
        crate::merge_queue::enter(&mut w.ctx, &a).unwrap();
        let head = git::head_sha(&w.repo.join(".worktree/A")).unwrap();
        let job = queue_job(&w, &a, "merge_gate", &head[..12], None);
        if mode == "vanished" {
            std::fs::write(&job.started, "1").unwrap();
            queue_run(&mut w, &a);
            assert!(!job.exists());
        } else if mode == "head_changed" {
            let wt = w.repo.join(".worktree/A");
            std::fs::write(wt.join("change"), "new").unwrap();
            git::commit_paths(&wt, &["change"], "new head").unwrap();
            queue_run(&mut w, &a);
        } else if mode == "removed_gate" {
            std::fs::write(
                config_io::project_config_path(&w.repo),
                "[merge]\nauto = true\n",
            )
            .unwrap();
            queue_run(&mut w, &a);
            assert_eq!(
                w.state(&a.id),
                TaskState::Active {
                    node: Node::Merging
                }
            );
        } else {
            if mode == "revoked" {
                w.ctx
                    .store
                    .append_event("merge.auto", &a.id, json!({"active":false,"by":"user"}))
                    .unwrap();
            }
            queue_end(&job, if mode == "timeout" { 124 } else { 0 });
            queue_run(&mut w, &a);
            assert_eq!(queue_count(&w, &a, "merge.unlocked"), 1);
            assert_eq!(
                crate::merge_queue::holder(&w.ctx, &w.project_id).unwrap(),
                None
            );
            continue;
        }
        assert_eq!(
            crate::merge_queue::holder(&w.ctx, &w.project_id).unwrap(),
            Some(a.id.clone())
        );
        assert_eq!(queue_count(&w, &a, "merge.unlocked"), 0);
    }
}
fn mq_setup_case(pending: bool) {
    let mut w = mq_world("mq-setup");
    let release = w.root.join("release");
    let path = config_io::project_config_path(&w.repo);
    let command = format!(
        "if [ -f new-main ]; then while [ ! -f '{}' ]; do sleep 0.01; done; fi",
        release.display()
    );
    let text = std::fs::read_to_string(&path).unwrap()
        + &format!("\n[setup]\ncommand = \"{command}\"\nwatch = ['new-main']\n");
    std::fs::write(path, text).unwrap();
    git::commit_paths(&w.repo, &[".autome/config.toml"], "setup").unwrap();
    let b = mq_task(&mut w, "B");
    let c = mq_task(&mut w, "C");
    let _stop = QueueStop(w.repo.clone(), b.id.clone());
    if !pending {
        std::fs::write(&release, "go").unwrap();
    }
    queue_run(&mut w, &b);
    let setup = crate::gate::Job::at(&w.repo, &b.id, "setup", "worktree");
    queue_wait(|| setup.exit.exists() && !setup.pid.exists());
    mq_move(&w);
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "setup.started"), 2);
    assert_eq!(queue_count(&w, &b, "merge.rebased"), 1);
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    for _ in 0..3 {
        queue_run(&mut w, &c);
        assert_eq!(
            crate::merge_queue::holder(&w.ctx, &w.project_id).unwrap(),
            Some(b.id.clone())
        );
        assert_eq!(queue_count(&w, &c, "merge.locked"), 0);
    }
    std::fs::write(&release, "go").unwrap();
    mq_complete(&mut w, &b);
    let b_now = w.ctx.store.get_task(&b.id).unwrap();
    run_core_step(&mut w.ctx, &b_now, Node::Merging).unwrap();
    mq_complete(&mut w, &c);
    assert!(mq_seq(&w, &b, "gate.started") < mq_seq(&w, &c, "gate.started"));
    assert!(mq_seq(&w, &b, "merge.unlocked") < mq_seq(&w, &c, "merge.locked"));
}
#[test]
fn merge_queue_a_dequeue_rebase_that_changes_setup_inputs_reruns_setup_first() {
    mq_setup_case(false);
}
#[test]
fn merge_queue_a_pending_setup_after_the_dequeue_rebase_keeps_the_section() {
    mq_setup_case(true);
}
#[test]
fn merge_queue_a_reusable_gate_does_not_jump_an_earlier_queued_task() {
    let mut w = mq_world("mq-fifo-reuse");
    let a = mq_task(&mut w, "A");
    let c = mq_task(&mut w, "C");
    let b = mq_task(&mut w, "B");
    crate::merge_queue::enter(&mut w.ctx, &a).unwrap();
    queue_run(&mut w, &b);
    queue_run(&mut w, &c);
    let head = git::head_sha(&w.repo.join(".worktree/C")).unwrap();
    w.ctx
        .store
        .append_event("gate.result", &c.id, json!({"ok":true,"sha":head}))
        .unwrap();
    advance(
        &mut w.ctx,
        &a.id,
        &Trigger::GateDone {
            status: None,
            node: Node::MergeGate,
            outcome: GateOutcome::Failed {
                head: "head".into(),
                command: "false".into(),
                tail: "test".into(),
            },
        },
    )
    .unwrap();
    queue_run(&mut w, &c);
    assert_eq!(queue_count(&w, &c, "merge.locked"), 0);
    queue_run(&mut w, &b);
    advance(&mut w.ctx, &b.id, &Trigger::Stop).unwrap();
    queue_run(&mut w, &c);
    assert!(mq_seq(&w, &b, "merge.locked") < mq_seq(&w, &c, "merge.locked"));
    assert!(mq_seq(&w, &b, "merge.locked") < mq_seq(&w, &c, "gate.reused"));
}

fn ma_call(w: &mut World, id: &str, cancel: bool) -> crate::ipc::ReplyOutcome {
    ma_params(w, json!({"task_id":id,"cancel":cancel}))
}
fn ma_params(w: &mut World, params: serde_json::Value) -> crate::ipc::ReplyOutcome {
    crate::dispatch::handle_command(
        &mut w.ctx,
        &crate::ipc::Command {
            request_id: "merge-auth".into(),
            command_id: crate::store::new_id("auth"),
            expected_revision: None,
            protocol_version: crate::ipc::PROTOCOL_VERSION,
            method: "task.merge".into(),
            params,
        },
    )
    .reply
    .outcome
}
fn ma_ok(out: crate::ipc::ReplyOutcome, expected: &str) {
    match out {
        crate::ipc::ReplyOutcome::Ok { payload, .. } => {
            assert_eq!(payload["merge_request"], expected)
        }
        other => panic!("{other:?}"),
    }
}
#[test]
fn merge_auth_cancel_is_rejected_once_the_merge_has_started() {
    let mut w = mq_world("ma-started");
    let t = mq_task(&mut w, "T-1");
    assert!(crate::merge_queue::enter(&mut w.ctx, &t).unwrap());
    ma_ok(ma_call(&mut w, &t.id, true), "revoked");
    assert!(!crate::merge_queue::holds(&w.ctx, &w.ctx.store.get_task(&t.id).unwrap()).unwrap());
    assert_eq!(
        w.ctx
            .store
            .last_event(&t.id, "merge.unlocked")
            .unwrap()
            .unwrap()["reason"],
        "cancel"
    );
    // Complete the revoked gate: it must stop at the card.
    mq_complete(&mut w, &t);
    assert_eq!(
        w.state(&t.id),
        TaskState::Active {
            node: Node::AwaitMerge
        }
    );
    w.ctx
        .store
        .append_event("merge.auto", &t.id, json!({"active":true,"by":"user"}))
        .unwrap();
    w.set_state(
        &t.id,
        TaskState::Active {
            node: Node::Merging,
        },
    );
    let count = queue_count(&w, &t, "merge.auto");
    assert!(matches!(
        ma_call(&mut w, &t.id, true),
        crate::ipc::ReplyOutcome::Error { .. }
    ));
    assert_eq!(queue_count(&w, &t, "merge.auto"), count);
    let t = w.ctx.store.get_task(&t.id).unwrap();
    run_core_step(&mut w.ctx, &t, Node::Merging).unwrap();
    assert!(w.ctx.store.get_task(&t.id).unwrap().merge_commit.is_some());
}
#[test]
fn merge_auth_cancel_overrides_the_project_switch() {
    let mut w = mq_world("ma-cancel");
    let t = mq_task(&mut w, "T-1");
    w.set_state(
        &t.id,
        TaskState::Paused {
            resume: Node::MergeGate,
        },
    );
    ma_ok(ma_call(&mut w, &t.id, false), "already_authorized");
    assert_eq!(queue_count(&w, &t, "merge.auto"), 0);
    ma_ok(ma_call(&mut w, &t.id, true), "revoked");
    assert!(
        crate::merge_queue::authorization(&w.ctx, &t)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        ma_call(&mut w, &t.id, true),
        crate::ipc::ReplyOutcome::Error { .. }
    ));
    ma_ok(ma_call(&mut w, &t.id, false), "authorized");
    ma_ok(ma_call(&mut w, &t.id, false), "already_authorized");
    assert_eq!(queue_count(&w, &t, "merge.auto"), 2);
    assert_eq!(
        crate::merge_queue::authorization(&w.ctx, &t).unwrap(),
        Some(autome_domain::merge_queue::AuthSource::User)
    );
}
#[test]
fn merge_auth_task_merge_rejects_failed_and_finished_tasks() {
    let mut w = mq_world("ma-terminal");
    let t = mq_task(&mut w, "T-1");
    for params in [
        json!({}),
        json!({"task_id":""}),
        json!({"task_id":"missing"}),
        json!({"task_id":t.id,"cancel":"true"}),
        json!({"task_id":t.id,"cancel":null}),
    ] {
        assert!(matches!(
            ma_params(&mut w, params),
            crate::ipc::ReplyOutcome::Error { .. }
        ));
        assert_eq!(queue_count(&w, &t, "merge.auto"), 0);
    }
    for state in [
        TaskState::Failed {
            at: Node::MergeGate,
            reason: autome_domain::task::FailureReason::ImplBudget { limit: 1 },
        },
        TaskState::Done,
        TaskState::Cancelled,
        TaskState::Active {
            node: Node::MainCheck,
        },
        TaskState::Active {
            node: Node::Cleanup,
        },
    ] {
        w.set_state(&t.id, state);
        assert!(matches!(
            ma_call(&mut w, &t.id, false),
            crate::ipc::ReplyOutcome::Error { .. }
        ));
        assert_eq!(queue_count(&w, &t, "merge.auto"), 0);
    }
}
#[test]
fn merge_auth_leaving_the_merge_tail_writes_no_revocation() {
    for trigger in [
        Trigger::Pause,
        Trigger::Stop,
        Trigger::Cancel,
        Trigger::GateSameFailure {
            node: Node::MergeGate,
            count: 3,
            summary: "same".into(),
            log: "test".into(),
        },
        Trigger::GateDone {
            status: None,
            node: Node::MergeGate,
            outcome: GateOutcome::Failed {
                head: "head".into(),
                command: "false".into(),
                tail: "test".into(),
            },
        },
        Trigger::SetupFailed {
            command: "sleep 60".into(),
            exit_code: 124,
            log: "timeout".into(),
        },
    ] {
        let mut w = mq_world("ma-lifetime");
        let t = mq_task(&mut w, "T-1");
        w.ctx
            .store
            .append_event("merge.auto", &t.id, json!({"active":true,"by":"user"}))
            .unwrap();
        advance(&mut w.ctx, &t.id, &trigger).unwrap();
        assert_eq!(queue_count(&w, &t, "merge.auto"), 1);
        assert_eq!(
            w.ctx
                .store
                .last_event(&t.id, "merge.auto")
                .unwrap()
                .unwrap()["active"],
            true
        );
    }
}

#[test]
fn merge_conflict_a_dequeue_conflict_yields_the_queue_and_counts_the_streak() {
    let mut w = mq_world("mc-yield");
    let a = queue_task(&mut w, "A");
    let b = mq_task(&mut w, "B");
    let c = mq_task(&mut w, "C");
    let wt = w.repo.join(".worktree/B");
    std::fs::write(wt.join("README.md"), "branch\n").unwrap();
    git::commit_paths(&wt, &["README.md"], "branch conflict").unwrap();
    let blocker = queue_job(&w, &a, "gate", "block", None);
    queue_run(&mut w, &b);
    queue_run(&mut w, &c);
    assert_eq!(queue_count(&w, &b, "gate.queued"), 1);
    std::fs::write(w.repo.join("README.md"), "main\n").unwrap();
    git::commit_paths(&w.repo, &["README.md"], "main conflict").unwrap();
    let main = git::head_sha(&w.repo).unwrap();
    blocker.clear();
    queue_run(&mut w, &b);
    assert_eq!(
        w.state(&b.id),
        TaskState::Active {
            node: Node::Implement
        }
    );
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    assert_eq!(queue_count(&w, &b, "merge.unlocked"), 1);
    assert!(merge_requested(&w.ctx, &b.id).unwrap());
    assert_eq!(git::head_sha(&w.repo).unwrap(), main);
    assert_eq!(
        std::fs::read_to_string(wt.join("README.md")).unwrap(),
        "branch\n"
    );
    queue_run(&mut w, &c);
    assert_eq!(queue_count(&w, &c, "gate.started"), 1);
    assert!(mq_seq(&w, &b, "merge.unlocked") < mq_seq(&w, &c, "gate.started"));
    // Replay the same conflict after repair rounds; the third observation trips
    // the breaker, while a zero budget prevents even the first repair launch.
    for (budget, expected_count) in [(25, 2), (25, 3), (0, 4)] {
        w.ctx.store.set_task_budget(&b.id, budget).unwrap();
        w.set_state(
            &b.id,
            TaskState::Active {
                node: Node::MergeGate,
            },
        );
        advance(
            &mut w.ctx,
            &b.id,
            &Trigger::CoreStepDone {
                node: Node::MergeGate,
                result: CoreStepResult::Conflict {
                    files: vec!["README.md".into()],
                    detail: "same conflict".into(),
                },
            },
        )
        .unwrap();
        assert_eq!(
            w.ctx
                .store
                .last_event(&b.id, "rebase.conflicts")
                .unwrap()
                .unwrap()["count"],
            expected_count
        );
        match expected_count {
            2 => assert_eq!(
                w.state(&b.id),
                TaskState::Active {
                    node: Node::Implement
                }
            ),
            3 => assert!(matches!(
                w.state(&b.id),
                TaskState::Failed {
                    at: Node::MergeGate,
                    reason: FailureReason::RebaseConflict { .. }
                }
            )),
            _ => assert!(matches!(
                w.state(&b.id),
                TaskState::Failed {
                    at: Node::MergeGate,
                    reason: FailureReason::ImplBudget { limit: 0 }
                }
            )),
        }
        assert!(merge_requested(&w.ctx, &b.id).unwrap());
    }
}

#[test]
fn merge_conflict_bug_streak_and_budget_stop_before_launching_another_fix() {
    for (count, budget, breaker) in [(1, 25, true), (0, 0, false)] {
        let (mut w, task, _, _, _, _) = repro_gate_world("before_merge", 0);
        w.ctx.store.set_task_budget(&task.id, budget).unwrap();
        w.ctx
            .store
            .append_event(
                "rebase.conflicts",
                &task.id,
                json!({"files":["a","b"],"count":count}),
            )
            .unwrap();
        for expected in (count + 1)..=3 {
            w.set_state(
                &task.id,
                TaskState::Active {
                    node: Node::MergeGate,
                },
            );
            advance(
                &mut w.ctx,
                &task.id,
                &Trigger::CoreStepDone {
                    node: Node::MergeGate,
                    result: CoreStepResult::Conflict {
                        files: vec!["b".into(), "a".into(), "a".into()],
                        detail: "conflict".into(),
                    },
                },
            )
            .unwrap();
            assert_eq!(
                w.ctx
                    .store
                    .last_event(&task.id, "rebase.conflicts")
                    .unwrap()
                    .unwrap()["count"],
                expected
            );
            if !breaker {
                assert!(matches!(
                    w.state(&task.id),
                    TaskState::Failed {
                        at: Node::MergeGate,
                        reason: FailureReason::FixBudget { limit: 0 }
                    }
                ));
                assert_eq!(queue_count(&w, &task, "bug.fix_round_started"), 0);
                break;
            }
            if expected == 2 {
                assert_eq!(w.state(&task.id), TaskState::Active { node: Node::Fix });
            } else {
                assert!(matches!(
                    w.state(&task.id),
                    TaskState::Failed {
                        at: Node::MergeGate,
                        reason: FailureReason::RebaseConflict { .. }
                    }
                ));
                assert_eq!(queue_count(&w, &task, "bug.fix_round_started"), 1);
            }
        }
    }
}

fn mv_read(w: &mut World, method: &str, params: serde_json::Value) -> serde_json::Value {
    let result = crate::dispatch::handle_command(
        &mut w.ctx,
        &crate::ipc::Command {
            request_id: "view".into(),
            command_id: crate::store::new_id("view"),
            expected_revision: None,
            protocol_version: crate::ipc::PROTOCOL_VERSION,
            method: method.into(),
            params,
        },
    );
    match result.reply.outcome {
        crate::ipc::ReplyOutcome::Ok { payload, .. } => payload,
        other => panic!("{other:?}"),
    }
}
#[test]
fn merge_view_behind_count_counts_commits_only_on_the_base() {
    let mut w = mq_world("view-count");
    let t = mq_task(&mut w, "A");
    assert_eq!(git::behind_count(&w.repo, &t.branch(), "main").unwrap(), 0);
    mq_move(&w);
    assert_eq!(git::behind_count(&w.repo, &t.branch(), "main").unwrap(), 1);
    assert!(git::behind_count(&w.repo, "missing", "main").is_err());
    assert!(git::behind_count(&w.repo, "", "missing").is_err());
}
#[test]
fn merge_view_task_get_reports_a_stale_await_merge() {
    let mut w = mq_world("view-stale");
    let t = mq_task(&mut w, "A");
    mq_move(&w);
    for node in [
        Node::MergeGate,
        Node::AwaitMerge,
        Node::Implement,
        Node::Merging,
    ] {
        w.set_state(&t.id, TaskState::Active { node });
        let got = mv_read(&mut w, "task.get", json!({"task_id":t.id}));
        let stale = &got["task"]["merge"]["stale"];
        if node == Node::AwaitMerge {
            assert_eq!(stale["behind"], 1);
            assert_eq!(stale["gated"], true);
            assert_eq!(stale["repos"][0]["base"], "main");
        } else {
            assert!(stale.is_null());
        }
    }
    w.set_state(
        &t.id,
        TaskState::Active {
            node: Node::AwaitMerge,
        },
    );
    git::rebase(&w.repo.join(".worktree/A"), "main").unwrap();
    assert!(
        mv_read(&mut w, "task.get", json!({"task_id":t.id}))["task"]["merge"]["stale"].is_null()
    );
    std::fs::write(w.repo.join("another-main"), "another\n").unwrap();
    git::commit_paths(&w.repo, &["another-main"], "move again").unwrap();
    std::fs::write(
        config_io::project_config_path(&w.repo),
        "[merge]\nauto = true\n",
    )
    .unwrap();
    let mut snapshot = w.ctx.store.get_task(&t.id).unwrap();
    let plain = crate::merge_queue::view(&w.ctx, &snapshot).unwrap();
    assert_eq!(plain["stale"]["gated"], false);
    assert_eq!(plain["stale"]["behind"], 1);
    snapshot.kind = TaskKind::Bug;
    assert_eq!(
        crate::merge_queue::view(&w.ctx, &snapshot).unwrap()["stale"]["gated"],
        true
    );
    for (active, by, expected) in [
        (true, "user", json!("user")),
        (true, "behind", json!("behind")),
        (false, "user", serde_json::Value::Null),
    ] {
        w.ctx
            .store
            .append_event("merge.auto", &t.id, json!({"active":active,"by":by}))
            .unwrap();
        assert_eq!(
            mv_read(&mut w, "task.get", json!({"task_id":t.id}))["task"]["merge"]["authorized_by"],
            expected
        );
    }
    let listed = mv_read(&mut w, "project.get", json!({"project_id":t.project_id}));
    assert_eq!(listed["tasks"][0]["merge"]["stale"]["behind"], 1);
    // Missing branch: read path degrades, rather than failing task.get.

    let mut broken = w.ctx.store.get_task(&t.id).unwrap();
    broken.slug = "missing".into();
    assert!(crate::merge_queue::view(&w.ctx, &broken).unwrap()["stale"].is_null());
    w.set_state(&t.id, TaskState::Cancelled);
    assert!(mv_read(&mut w, "task.get", json!({"task_id":t.id}))["task"]["merge"].is_null());
    // Workspace totals include every member, even with different default branches.
    w.set_state(
        &t.id,
        TaskState::Active {
            node: Node::AwaitMerge,
        },
    );
    let extra = w.root.join("extra");
    std::fs::create_dir_all(&extra).unwrap();
    git::init(&extra, "trunk").unwrap();
    std::fs::write(extra.join("file"), "one").unwrap();
    git::commit_paths(&extra, &["file"], "initial").unwrap();
    let checkout = w.root.join(".worktree/A/extra");
    git::worktree_add(&extra, checkout.to_str().unwrap(), &t.branch(), "trunk").unwrap();
    for content in ["two", "three"] {
        std::fs::write(extra.join("file"), content).unwrap();
        git::commit_paths(&extra, &["file"], "advance").unwrap();
    }
    let mut project = w.ctx.store.get_project(&t.project_id).unwrap();
    project.path = w.root.to_string_lossy().into_owned();
    project.kind = autome_domain::project::ProjectKind::Workspace;
    project.docs_repo = Some("repo".into());
    project.members = [("repo", "main"), ("extra", "trunk")]
        .into_iter()
        .map(|(name, base)| autome_domain::project::Member {
            name: name.into(),
            default_branch: base.into(),
        })
        .collect();
    w.ctx.store.replace_project(&project).unwrap();
    let got = mv_read(&mut w, "task.get", json!({"task_id":t.id}));
    assert_eq!(got["task"]["merge"]["stale"]["behind"], 3);
    assert_eq!(
        got["task"]["merge"]["stale"]["repos"][1],
        json!({"repo":"extra","base":"trunk","behind":2})
    );
    std::fs::rename(&extra, w.root.join("missing-extra")).unwrap();
    assert!(
        mv_read(&mut w, "task.get", json!({"task_id":t.id}))["task"]["merge"]["stale"].is_null()
    );
}
#[test]
fn merge_view_dashboard_reports_authorization_waiting_and_warnings() {
    let mut w = mq_world("view-dashboard");
    let a = mq_task(&mut w, "A");
    let b = mq_task(&mut w, "B");
    crate::merge_queue::enter(&mut w.ctx, &a).unwrap();
    let got = mv_read(&mut w, "dashboard.get", json!({}));
    let entries = got["running"].as_array().unwrap();
    let first = entries.iter().find(|e| e["task"]["id"] == a.id).unwrap();
    let second = entries.iter().find(|e| e["task"]["id"] == b.id).unwrap();
    assert_eq!(first["task"]["merge"]["holds_section"], true);
    assert_eq!(first["task"]["merge"]["authorized_by"], "project");
    assert_eq!(second["task"]["merge"]["waiting_for"], a.id);
    assert!(second["gate_queue_position"].is_null());
    w.set_state(
        &b.id,
        TaskState::Active {
            node: Node::Implement,
        },
    );
    assert!(
        mv_read(&mut w, "task.get", json!({"task_id":b.id}))["task"]["merge"]["waiting_for"]
            .is_null()
    );
    std::fs::write(
        config_io::project_config_path(&w.repo),
        "[merge]\nauto = true\n",
    )
    .unwrap();
    let got = mv_read(&mut w, "dashboard.get", json!({}));
    assert!(
        got["config_warnings"][0]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("没有配置 [gate]")
    );
}
