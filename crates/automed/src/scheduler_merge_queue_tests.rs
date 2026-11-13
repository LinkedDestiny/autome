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
