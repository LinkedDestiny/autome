// Included in scheduler::tests so these exercise the real scheduler entry points.
struct QueueStop(PathBuf, String);
impl Drop for QueueStop {
    fn drop(&mut self) {
        crate::gate::stop_all(&self.0, &self.1);
    }
}
fn queue_world(tag: &str) -> World {
    let w = World::new(tag);
    let path = config_io::project_config_path(&w.repo);
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str("\n[gate]\ncommand = 'true'\ntimeout_minutes = 1\n");
    std::fs::write(path, text).unwrap();
    w
}
fn queue_task(w: &mut World, id: &str) -> TaskRecord {
    let t = w.add_task(id, id);
    w.write_design(
        &t,
        &design_doc("实现中", &[("M-01", MilestoneState::Open)], ""),
    );
    w.set_state(id, TaskState::Active { node: Node::Gate });
    w.ctx.store.get_task(id).unwrap()
}
fn queue_run(w: &mut World, t: &TaskRecord) {
    let project = w.ctx.store.get_project(&t.project_id).unwrap();
    let layout = TaskLayout::of(&project, &t.slug, &[]);
    run_gate(&mut w.ctx, t, &project, &layout, t.state.node().unwrap()).unwrap();
}
fn queue_count(w: &World, t: &TaskRecord, kind: &str) -> u32 {
    w.ctx.store.count_events(&t.id, kind).unwrap()
}
fn queue_wait(mut check: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !check() {
        assert!(
            std::time::Instant::now() < deadline,
            "bounded job wait expired"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
// Synthetic startup window and completed markers exercise the on-disk contract;
// tests requiring process lifetime below use the real wrapper instead.
fn queue_job(
    w: &World,
    t: &TaskRecord,
    node: &str,
    key: &str,
    done: Option<i32>,
) -> crate::gate::Job {
    let job = crate::gate::Job::at(&w.repo, &t.id, node, key);
    std::fs::create_dir_all(job.started.parent().unwrap()).unwrap();
    std::fs::write(
        &job.started,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string(),
    )
    .unwrap();
    if let Some(code) = done {
        queue_end(&job, code);
    }
    w.ctx
        .store
        .append_event(
            if node == "setup" {
                "setup.started"
            } else {
                "gate.started"
            },
            &t.id,
            json!({"node":node, "checkout":key,"log":job.log}),
        )
        .unwrap();
    job
}
fn queue_end(job: &crate::gate::Job, code: i32) {
    std::fs::write(
        &job.exit,
        ExitMarker {
            exit_code: code,
            ended_at: now_iso(),
        }
        .render(),
    )
    .unwrap();
}
fn queue_admit(w: &mut World, t: &TaskRecord) -> bool {
    let project = w.ctx.store.get_project(&t.project_id).unwrap();
    crate::gate_queue::admit(&mut w.ctx, t, &project, Node::Gate, "gate", None).unwrap()
}
fn queue_setup(
    w: &mut World,
    t: &TaskRecord,
    kind: autome_domain::setup::Checkout,
    command: &str,
) -> crate::setup::Ready {
    let project = w.ctx.store.get_project(&t.project_id).unwrap();
    let cwd = if kind == autome_domain::setup::Checkout::Main {
        w.repo.clone()
    } else {
        w.repo.join(".worktree").join(&t.slug)
    };
    crate::setup::ensure(
        &mut w.ctx,
        t,
        &project,
        &config::SetupConfig {
            command: command.into(),
            watch: vec![],
            timeout_minutes: 1,
        },
        crate::setup::Checkout {
            kind,
            dir: &cwd,
            record: crate::setup::worktree_record(&w.repo, &t.id),
        },
        t.state.node().unwrap(),
    )
    .unwrap()
}

#[test]
fn gate_queue_second_gate_waits_without_files_or_budget_changes() {
    let mut w = queue_world("queue-second");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let blocker = queue_job(&w, &a, "gate", "held", None);
    let before = (b.state.clone(), b.budget_n);
    for _ in 0..3 {
        queue_run(&mut w, &b);
    }
    assert_eq!(queue_count(&w, &b, "gate.queued"), 1);
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    assert!(!crate::gate::dir_for(&w.repo, &b.id).exists());
    assert_eq!(
        {
            let after = w.ctx.store.get_task(&b.id).unwrap();
            (after.state, after.budget_n)
        },
        before
    );
    assert_eq!(w.ctx.store.retry_attempts(&b.id).unwrap(), 0);
    blocker.clear();
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.started"), 1);
    queue_wait(|| crate::gate_queue::held(&w.ctx).unwrap() == Some(1));
}
#[test]
fn gate_queue_setup_and_gate_keep_one_slot() {
    let mut w = queue_world("queue-chain");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let path = config_io::project_config_path(&w.repo);
    let text = std::fs::read_to_string(&path).unwrap() + "\n[setup]\ncommand = 'true'\n";
    std::fs::write(path, text).unwrap();
    queue_run(&mut w, &a);
    let job = crate::gate::Job::at(&w.repo, &a.id, "setup", "worktree");
    queue_wait(|| job.exit.exists() && !job.pid.exists());
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.queued"), 1);
    queue_run(&mut w, &a);
    assert_eq!(queue_count(&w, &a, "gate.started"), 1);
    assert_eq!(queue_count(&w, &a, "gate.queued"), 0);
    assert_eq!(queue_count(&w, &b, "setup.started"), 0);
}
#[test]
fn gate_queue_finished_uncollected_job_blocks_earlier_task() {
    let mut w = queue_world("queue-uncollected");
    let b = queue_task(&mut w, "B");
    let a = queue_task(&mut w, "A");
    queue_run(&mut w, &a);
    let job = crate::gate::Job::at(
        &w.repo,
        &a.id,
        "gate",
        &git::head_sha(&w.repo.join(".worktree/A")).unwrap()[..12],
    );
    queue_wait(|| job.exit.exists());
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.started"), 0);
    queue_run(&mut w, &a);
    queue_run(&mut w, &b);
    let a_seq = w
        .ctx
        .store
        .last_event_of(&a.id, &["gate.result"])
        .unwrap()
        .unwrap()
        .0;
    let b_seq = w
        .ctx
        .store
        .last_event_of(&b.id, &["gate.started"])
        .unwrap()
        .unwrap()
        .0;
    assert!(a_seq < b_seq);
}
#[test]
fn gate_queue_upgrade_collects_two_old_jobs_without_killing_them() {
    let mut w = queue_world("queue-upgrade");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let c = queue_task(&mut w, "C");
    let mut jobs = Vec::new();
    for t in [&a, &b] {
        let key = git::head_sha(&w.repo.join(".worktree").join(&t.slug)).unwrap();
        jobs.push(queue_job(&w, t, "gate", &key[..12], None));
    }
    queue_run(&mut w, &c);
    assert_eq!(crate::gate_queue::held(&w.ctx).unwrap(), Some(2));
    for (t, j) in [(&a, &jobs[0]), (&b, &jobs[1])] {
        queue_run(&mut w, t);
        assert!(j.exists());
        assert!(!j.exit.exists());
        queue_end(j, 0);
        queue_run(&mut w, t);
        assert_eq!(queue_count(&w, t, "gate.result"), 1);
        if t.id == a.id {
            queue_run(&mut w, &c);
            assert_eq!(queue_count(&w, &c, "gate.started"), 0);
        }
    }
    queue_run(&mut w, &c);
    assert_eq!(queue_count(&w, &c, "gate.started"), 1);
}
#[test]
fn gate_queue_pause_stop_and_resume_rejoin_tail() {
    let mut w = queue_world("queue-rejoin");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let c = queue_task(&mut w, "C");
    let job = queue_job(&w, &a, "gate", "held", None);
    for state in [
        TaskState::Paused { resume: Node::Gate },
        TaskState::Stopped { at: Node::Gate },
    ] {
        assert!(!queue_admit(&mut w, &b));
        assert!(!queue_admit(&mut w, &c));
        w.set_state(&b.id, state);
        w.ctx
            .store
            .append_event("task.updated", &b.id, json!({}))
            .unwrap();
        assert_eq!(crate::gate_queue::waiters(&w.ctx).unwrap()[0].task_id, c.id);
        w.set_state(&b.id, TaskState::Active { node: Node::Gate });
        w.ctx
            .store
            .append_event("task.updated", &b.id, json!({}))
            .unwrap();
        assert!(!queue_admit(&mut w, &b));
        assert_eq!(crate::gate_queue::waiters(&w.ctx).unwrap()[1].task_id, b.id);
        w.ctx
            .store
            .append_event("task.updated", &b.id, json!({}))
            .unwrap();
    }
    job.clear();
}
#[test]
fn gate_queue_stop_discard_and_vanished_release_slots() {
    let mut w = queue_world("queue-release");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let job = queue_job(&w, &a, "gate", "old", None);
    std::fs::write(&job.started, "1").unwrap();
    assert!(queue_admit(&mut w, &b)); // Vanished
    job.clear();
    let _stop = QueueStop(w.repo.clone(), a.id.clone());
    crate::gate::start(&job, &w.repo, "sleep 30", &[]).unwrap();
    queue_wait(|| job.pid.exists());
    assert!(!queue_admit(&mut w, &b));
    crate::gate::stop_all(&w.repo, &a.id);
    queue_wait(|| !crate::gate::running(&job));
    assert!(queue_admit(&mut w, &b));
    crate::gate::discard_node(&w.repo, &a.id, "gate").unwrap();
    assert!(!job.exists());
    assert_eq!(crate::gate_queue::held(&w.ctx).unwrap(), Some(0));
}
#[test]
fn gate_queue_paused_live_job_holds_but_completed_job_does_not() {
    let mut w = queue_world("queue-paused");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let job = queue_job(&w, &a, "gate", "old", None);
    w.set_state(&a.id, TaskState::Paused { resume: Node::Gate });
    assert!(!queue_admit(&mut w, &b));
    queue_end(&job, 0);
    assert!(queue_admit(&mut w, &b));
    assert!(job.exists());
}
#[test]
fn gate_queue_no_process_paths_do_not_enqueue() {
    let mut w = queue_world("queue-bypass");
    let a = queue_task(&mut w, "A");
    let mut b = queue_task(&mut w, "B");
    let job = queue_job(&w, &a, "gate", "held", None);
    b.state = TaskState::Active {
        node: Node::MergeGate,
    };
    w.set_state(&b.id, b.state.clone());
    let head = git::head_sha(&w.repo.join(".worktree/B")).unwrap();
    w.ctx
        .store
        .append_event("gate.result", &b.id, json!({"ok":true,"sha":head}))
        .unwrap();
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.reused"), 1);
    b.state = TaskState::Active {
        node: Node::MainCheck,
    };
    w.set_state(&b.id, b.state.clone());
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.skipped"), 1);
    let record = crate::setup::worktree_record(&w.repo, &b.id);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(record, autome_domain::setup::fingerprint("true", &[])).unwrap();
    assert!(matches!(
        queue_setup(&mut w, &b, autome_domain::setup::Checkout::Worktree, "true"),
        crate::setup::Ready::Ready
    ));
    let path = config_io::project_config_path(&w.repo);
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("command = 'true'", "command = ''");
    std::fs::write(path, text).unwrap();
    b.state = TaskState::Active { node: Node::Gate };
    w.set_state(&b.id, b.state.clone());
    queue_run(&mut w, &b);
    assert_eq!(queue_count(&w, &b, "gate.queued"), 0);
    job.clear();
}
#[test]
fn gate_queue_session_setup_wait_does_not_count_retry() {
    let mut w = queue_world("queue-session");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let job = queue_job(&w, &a, "gate", "held", None);
    let project = w.ctx.store.get_project(&b.project_id).unwrap();
    let mut resolved = resolve_config(&w.ctx, &project).unwrap();
    resolved.setup = Some(config::SetupConfig {
        command: "true".into(),
        watch: vec![],
        timeout_minutes: 1,
    });
    w.set_state(
        &b.id,
        TaskState::Active {
            node: Node::Implement,
        },
    );
    for _ in 0..2 {
        start_session(
            &mut w.ctx,
            &b,
            &project,
            &resolved,
            SessionKind::Role { role: Role::Impl },
            None,
        )
        .unwrap();
    }
    assert_eq!(w.ctx.store.retry_attempts(&b.id).unwrap(), 0);
    assert_eq!(queue_count(&w, &b, "session.started"), 0);
    assert_eq!(queue_count(&w, &b, "setup.started"), 0);
    assert_eq!(queue_count(&w, &b, "gate.queued"), 1);
    job.clear();
}
#[test]
fn gate_queue_main_setup_mutex_precedes_admission() {
    let mut w = queue_world("queue-main");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let job = queue_job(&w, &a, "setup", "main", None);
    assert!(matches!(
        queue_setup(&mut w, &b, autome_domain::setup::Checkout::Main, "true"),
        crate::setup::Ready::Pending
    ));
    assert_eq!(queue_count(&w, &b, "gate.queued"), 0);
    job.clear();
    assert!(matches!(
        queue_setup(&mut w, &b, autome_domain::setup::Checkout::Main, "true"),
        crate::setup::Ready::Pending
    ));
    assert_eq!(queue_count(&w, &b, "setup.started"), 1);
    let job = crate::gate::Job::at(&w.repo, &b.id, "setup", "main");
    queue_wait(|| job.exit.exists());
}
#[test]
fn gate_queue_project_limit_preserves_fifo_and_restart_order() {
    let mut w = queue_world("queue-fifo");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let c = queue_task(&mut w, "C");
    std::fs::write(
        w.ctx.autome_home.join("config.toml"),
        "[gate]\nconcurrency=3\n",
    )
    .unwrap();
    let path = config_io::project_config_path(&w.repo);
    let text = std::fs::read_to_string(&path).unwrap() + "concurrency=1\n";
    std::fs::write(&path, &text).unwrap();
    let job = queue_job(&w, &a, "gate", "held", None);
    assert!(!queue_admit(&mut w, &b));
    let mut other = w.ctx.store.get_project(&a.project_id).unwrap();
    other.id = "larger".into();
    other.path = w.root.join("larger").to_string_lossy().into_owned();
    std::fs::create_dir_all(&other.path).unwrap();
    w.ctx.store.insert_project(&other).unwrap();
    let c = TaskRecord {
        id: "larger-C".into(),
        project_id: other.id,
        ..c
    };
    w.ctx.store.insert_task(&c).unwrap();
    assert!(!queue_admit(&mut w, &b)); // head really has a smaller limit

    assert!(!queue_admit(&mut w, &c));
    assert_eq!(crate::gate_queue::waiters(&w.ctx).unwrap()[0].task_id, b.id);
    // Rebuild the context, preserving only the ledger and paths.
    let replacement = crate::store::Store::open_in_memory().unwrap();
    let store = std::mem::replace(&mut w.ctx.store, replacement);
    w.ctx = Ctx::new(store, &w.ctx.autome_home, &w.ctx.home).dry();

    job.clear();
    assert!(!queue_admit(&mut w, &c));
    assert!(queue_admit(&mut w, &b));
}
#[test]
fn gate_queue_events_do_not_reset_same_failure_count() {
    let store = crate::store::Store::open_in_memory().unwrap();
    let log = Path::new("/missing");
    let first = recorded(&store, Node::Gate, log);
    store
        .append_event("gate.queued", "T", json!({"position":1}))
        .unwrap();
    let mut result = failure_result(Node::Gate, log);
    assert_eq!(
        record_gate_result(&store, "T", Node::Gate, &mut result, 3).unwrap(),
        first
    );
    assert_eq!(store.count_events("T", "gate.result").unwrap(), 1);
    assert_eq!(recorded(&store, Node::Gate, log)["same_failures"], 2);
}
#[test]
fn gate_queue_stale_live_head_is_killed_before_admission() {
    let mut w = queue_world("queue-stale-live");
    let a = queue_task(&mut w, "A");
    let job = queue_job(&w, &a, "gate", "obsolete", None);
    let _stop = QueueStop(w.repo.clone(), a.id.clone());
    crate::gate::start(&job, &w.repo, "sleep 30", &[]).unwrap();
    queue_wait(|| job.pid.exists());
    let pid = std::fs::read_to_string(&job.pid)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    queue_run(&mut w, &a);
    queue_wait(|| !crate::launcher::pid_alive(pid));
    assert!(!job.exists());
    assert!(!job.log.exists());
    assert_eq!(queue_count(&w, &a, "gate.started"), 2);
}
#[test]
fn gate_queue_stale_exited_head_is_deleted_before_admission() {
    let mut w = queue_world("queue-stale-exit");
    let a = queue_task(&mut w, "A");
    let job = queue_job(&w, &a, "gate", "obsolete", Some(9));
    queue_run(&mut w, &a);
    assert!(!job.exists());
    assert!(!job.exit.exists());
    assert_eq!(queue_count(&w, &a, "gate.started"), 2);
    assert_eq!(queue_count(&w, &a, "gate.result"), 0);
}
#[test]
fn gate_queue_resumed_completed_setup_requeues_for_all_checkouts() {
    for kind in [
        autome_domain::setup::Checkout::Worktree,
        autome_domain::setup::Checkout::Verify,
        autome_domain::setup::Checkout::Main,
    ] {
        for expired in [false, true] {
            let mut w = queue_world("queue-resume");
            let a = queue_task(&mut w, "A");
            let b = queue_task(&mut w, "B");
            assert!(matches!(
                queue_setup(&mut w, &a, kind, "true"),
                crate::setup::Ready::Pending
            ));
            let job = crate::gate::Job::at(&w.repo, &a.id, "setup", kind.as_str());
            queue_wait(|| job.exit.exists() && !job.pid.exists());
            if expired {
                w.ctx.gate_prev_tick =
                    Some(std::time::SystemTime::now() + std::time::Duration::from_secs(1));
            } else {
                w.set_state(&a.id, TaskState::Paused { resume: Node::Gate });
                w.ctx
                    .store
                    .append_event("task.updated", &a.id, json!({}))
                    .unwrap();
            }
            let blocker = queue_job(&w, &b, "gate", "held", None);
            w.set_state(&a.id, TaskState::Active { node: Node::Gate });
            if !expired {
                w.ctx
                    .store
                    .append_event("task.updated", &a.id, json!({}))
                    .unwrap();
            }
            assert!(matches!(
                queue_setup(&mut w, &a, kind, "echo changed"),
                crate::setup::Ready::Pending
            ));
            assert_eq!(queue_count(&w, &a, "setup.started"), 1);
            assert_eq!(queue_count(&w, &a, "gate.queued"), 1);
            assert!(!job.exists());
            blocker.clear();
        }
    }
}
#[test]
fn gate_queue_unreachable_head_drops_queue_and_advances_followers() {
    let mut w = queue_world("queue-drop");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let c = queue_task(&mut w, "C");
    let job = queue_job(&w, &a, "gate", "held", None);
    assert!(!queue_admit(&mut w, &b));
    assert!(!queue_admit(&mut w, &c));
    job.clear();
    w.set_state(&a.id, TaskState::Paused { resume: Node::Gate });
    std::fs::rename(w.repo.join(".worktree/B"), w.root.join("B-hidden")).unwrap();
    w.ctx.recovered = true;
    let report = tick(&mut w.ctx);
    assert!(report.errors.iter().any(|e| e.contains(&b.id)));
    assert_eq!(queue_count(&w, &b, "gate.queue_dropped"), 1);
    assert_eq!(queue_count(&w, &c, "gate.started"), 1);
}
#[test]
fn gate_queue_paused_timeout_reclaims_and_preserves_124_for_resume() {
    let mut w = queue_world("queue-paused-timeout");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let head = git::head_sha(&w.repo.join(".worktree/A")).unwrap();
    let job = queue_job(&w, &a, "gate", &head[..12], None);
    let _stop = QueueStop(w.repo.clone(), a.id.clone());
    crate::gate::start(&job, &w.repo, "sleep 30", &[]).unwrap();
    queue_wait(|| job.pid.exists());
    std::fs::write(&job.started, "1").unwrap();
    w.set_state(&a.id, TaskState::Paused { resume: Node::Gate });
    crate::gate_queue::reclaim(&w.ctx).unwrap();
    assert!(queue_admit(&mut w, &b));
    assert_eq!(
        ExitMarker::parse(&std::fs::read_to_string(&job.exit).unwrap())
            .unwrap()
            .exit_code,
        124
    );
    assert!(crate::gate::tail(&job.log, 5).contains("已终止"));
    w.set_state(&a.id, TaskState::Active { node: Node::Gate });
    queue_run(&mut w, &a);
    assert_eq!(
        w.ctx
            .store
            .last_event(&a.id, "gate.result")
            .unwrap()
            .unwrap()["exit_code"],
        124
    );
}
#[test]
fn gate_queue_active_unreachable_jobs_timeout_without_business_poll() {
    for setup in [false, true] {
        let mut w = queue_world("queue-unreachable-timeout");
        let a = queue_task(&mut w, "A");
        let head = git::head_sha(&w.repo.join(".worktree/A")).unwrap();
        let job = queue_job(
            &w,
            &a,
            if setup { "setup" } else { "gate" },
            if setup { "worktree" } else { &head[..12] },
            None,
        );
        if setup {
            let running = w
                .repo
                .join(".autome/output/setup")
                .join(&a.id)
                .join("worktree.running");
            std::fs::create_dir_all(running.parent().unwrap()).unwrap();
            std::fs::write(running, autome_domain::setup::fingerprint("sleep 30", &[])).unwrap();
        }
        let _stop = QueueStop(w.repo.clone(), a.id.clone());
        crate::gate::start(&job, &w.repo, "sleep 30", &[]).unwrap();
        queue_wait(|| job.pid.exists());
        std::fs::write(&job.started, "1").unwrap();
        std::fs::rename(w.repo.join(".worktree/A"), w.root.join("A-hidden")).unwrap();
        w.ctx.recovered = true;
        tick(&mut w.ctx);
        assert_eq!(
            ExitMarker::parse(&std::fs::read_to_string(&job.exit).unwrap())
                .unwrap()
                .exit_code,
            124
        );
        assert!(crate::gate::tail(&job.log, 5).contains("已终止"));
        // One completed tick bounds uncollected occupancy; files remain intact.
        tick(&mut w.ctx);
        assert_eq!(crate::gate_queue::held(&w.ctx).unwrap(), Some(0));
        assert!(job.exists());
        std::fs::rename(w.root.join("A-hidden"), w.repo.join(".worktree/A")).unwrap();
        if setup {
            assert!(matches!(
                queue_setup(
                    &mut w,
                    &a,
                    autome_domain::setup::Checkout::Worktree,
                    "sleep 30"
                ),
                crate::setup::Ready::Failed { exit_code: 124, .. }
            ));
            assert_eq!(
                w.ctx
                    .store
                    .last_event(&a.id, "setup.result")
                    .unwrap()
                    .unwrap()["exit_code"],
                124
            );
        } else {
            queue_run(&mut w, &a);
            assert_eq!(
                w.ctx
                    .store
                    .last_event(&a.id, "gate.result")
                    .unwrap()
                    .unwrap()["exit_code"],
                124
            );
        }
    }
}
#[test]
fn gate_queue_old_uncollected_and_removed_setup_jobs_release_but_keep_results() {
    for setup in [false, true] {
        let mut w = queue_world("queue-old-exit");
        let a = queue_task(&mut w, "A");
        let b = queue_task(&mut w, "B");
        let head = git::head_sha(&w.repo.join(".worktree/A")).unwrap();
        let job = queue_job(
            &w,
            &a,
            if setup { "setup" } else { "gate" },
            if setup { "worktree" } else { &head[..12] },
            Some(9),
        );
        assert!(!queue_admit(&mut w, &b));
        w.ctx.gate_prev_tick =
            Some(std::time::SystemTime::now() + std::time::Duration::from_secs(1));
        assert!(queue_admit(&mut w, &b));
        assert!(job.exists());
        if !setup {
            queue_run(&mut w, &a);
            assert_eq!(
                w.ctx
                    .store
                    .last_event(&a.id, "gate.result")
                    .unwrap()
                    .unwrap()["exit_code"],
                9
            );
        } else {
            assert!(!crate::gate_queue::continuation(&w.ctx, &a, &job, "worktree").unwrap());
        }
    }
}
#[test]
fn gate_queue_unknown_directory_blocks_only_inflight_ledger_jobs() {
    let mut w = queue_world("queue-unknown");
    let a = queue_task(&mut w, "A");
    let b = queue_task(&mut w, "B");
    let mut p = w.ctx.store.get_project(&a.project_id).unwrap();
    p.id = "other".into();
    p.path = w.root.join("other").to_string_lossy().into_owned();
    std::fs::create_dir_all(&p.path).unwrap();
    w.ctx.store.insert_project(&p).unwrap();
    let other = TaskRecord {
        id: "other".into(),
        project_id: p.id.clone(),
        ..a.clone()
    };
    w.ctx.store.insert_task(&other).unwrap();
    std::fs::remove_dir(&p.path).unwrap();
    assert!(queue_admit(&mut w, &b));
    w.ctx
        .store
        .append_event("gate.started", &other.id, json!({"node":"gate"}))
        .unwrap();
    assert!(!queue_admit(&mut w, &b));
    let errors = crate::gate_queue::reclaim(&w.ctx).unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("请恢复该目录") && e.contains("other"))
    );
    assert_eq!(queue_count(&w, &b, "gate.queue_dropped"), 0);
    std::fs::create_dir_all(&p.path).unwrap();
    assert!(queue_admit(&mut w, &b));
}
#[test]
fn gate_queue_fix_round_is_counted_once_after_setup_not_while_waiting() {
    let mut w = queue_world("queue-fix-budget");
    let a = queue_task(&mut w, "A");
    let base = queue_task(&mut w, "B");
    let b = TaskRecord {
        id: "bug".into(),
        slug: "bug".into(),
        kind: TaskKind::Bug,
        state: TaskState::Active {
            node: BugStep::Fix.node(),
        },
        ..base
    };
    w.ctx.store.insert_task(&b).unwrap();
    w.with_worktree(&b);
    w.ctx
        .store
        .append_event("bug.loop_started", &b.id, json!({}))
        .unwrap();
    let blocker = queue_job(&w, &a, "gate", "held", None);
    let project = w.ctx.store.get_project(&b.project_id).unwrap();
    let mut resolved = resolve_config(&w.ctx, &project).unwrap();
    resolved.setup = Some(config::SetupConfig {
        command: "true".into(),
        watch: vec![],
        timeout_minutes: 1,
    });
    let transition = Transition {
        next: b.state.clone(),
        action: Action::StartBugSession {
            step: BugStep::Fix,
            inject: None,
        },
        budget_n: None,
        consumes_decisions: false,
        merge_requested: false,
    };
    perform(&mut w.ctx, &b, &project, &resolved, &transition).unwrap();
    assert_eq!(queue_count(&w, &b, "bug.fix_round_started"), 0);
    assert_eq!(queue_count(&w, &b, "bug.fix_round_requested"), 1);
    for _ in 0..2 {
        start_session(
            &mut w.ctx,
            &b,
            &project,
            &resolved,
            SessionKind::Bug { step: BugStep::Fix },
            None,
        )
        .unwrap();
    }
    assert_eq!(queue_count(&w, &b, "bug.fix_round_started"), 0);
    assert_eq!(w.ctx.store.retry_attempts(&b.id).unwrap(), 0);
    blocker.clear();
    start_session(
        &mut w.ctx,
        &b,
        &project,
        &resolved,
        SessionKind::Bug { step: BugStep::Fix },
        None,
    )
    .unwrap();
    let job = crate::gate::Job::at(&w.repo, &b.id, "setup", "worktree");
    queue_wait(|| job.exit.exists() && !job.pid.exists());
    for _ in 0..3 {
        start_session(
            &mut w.ctx,
            &b,
            &project,
            &resolved,
            SessionKind::Bug { step: BugStep::Fix },
            None,
        )
        .unwrap();
        assert_eq!(queue_count(&w, &b, "bug.fix_round_started"), 1);
        let session = w.ctx.store.running_session(&b.id).unwrap().unwrap();
        w.ctx
            .store
            .finish_session(
                &session.id,
                &SessionLifecycle::Exited { exit_code: 1 },
                &now_iso(),
            )
            .unwrap();
        w.ctx
            .store
            .append_event("task.updated", &b.id, json!({}))
            .unwrap();
    }
}

// Regression: user actions must enter the same slot admission path as resume.
fn user_action_slot_regression(trigger: Trigger, initial: TaskState, resume: Node) {
    assert!(
        git_available(),
        "Git is required; this regression must not skip"
    );
    let mut w = World::new("user-action-slot");
    w.set_parallel(1);
    let task = w.add_task("T-target", "target");
    w.write_design(
        &task,
        &design_doc("实现中", &[("M-01", MilestoneState::Open)], ""),
    );
    w.set_state(&task.id, initial);
    w.add_task("T-busy", "busy");
    w.set_state(
        "T-busy",
        TaskState::Active {
            node: Node::Implement,
        },
    );
    assert_eq!(w.ctx.store.slots_in_use(&w.project_id).unwrap(), 1);
    assert!(fill_slots(&mut w.ctx).unwrap().is_empty());

    // This is the production entry called by dispatch::task_trigger, before
    // its subsequent tick can mask transient oversubscription by doing work.
    apply_trigger(&mut w.ctx, &task.id, &trigger).unwrap();
    assert_eq!(
        w.ctx.store.slots_in_use(&w.project_id).unwrap(),
        1,
        "{trigger:?} bypassed parallel=1; target={:?}, sessions={:?}",
        w.state(&task.id),
        w.ctx.store.list_sessions(&task.id).unwrap()
    );
    assert_eq!(w.state(&task.id), TaskState::Requeued { resume });
    assert!(w.ctx.store.list_sessions(&task.id).unwrap().is_empty());
    assert!(fill_slots(&mut w.ctx).unwrap().is_empty());

    w.set_state(
        "T-busy",
        TaskState::Stopped {
            at: Node::Implement,
        },
    );
    assert_eq!(fill_slots(&mut w.ctx).unwrap(), vec![task.id.clone()]);
    assert_eq!(w.state(&task.id), TaskState::Active { node: resume });
    assert_eq!(w.ctx.store.slots_in_use(&w.project_id).unwrap(), 1);
    if resume == Node::Implement {
        assert_eq!(w.ctx.store.list_sessions(&task.id).unwrap().len(), 1);
    }
}

#[test]
fn user_action_slot_approve_waits_for_capacity() {
    user_action_slot_regression(
        Trigger::Approve,
        TaskState::Active {
            node: Node::AwaitDesignApproval,
        },
        Node::Implement,
    );
}

#[test]
fn user_action_slot_rerun_waits_for_capacity() {
    user_action_slot_regression(
        Trigger::RerunFrom {
            node: Node::Implement,
        },
        TaskState::Stopped {
            at: Node::Implement,
        },
        Node::Implement,
    );
}

#[test]
fn user_action_slot_merge_waits_for_capacity() {
    user_action_slot_regression(
        Trigger::Merge,
        TaskState::Active {
            node: Node::AwaitMerge,
        },
        Node::Merging,
    );
}

#[test]
fn user_action_slot_resume_control_waits_then_starts() {
    user_action_slot_regression(
        Trigger::Resume,
        TaskState::Stopped {
            at: Node::Implement,
        },
        Node::Implement,
    );
}
