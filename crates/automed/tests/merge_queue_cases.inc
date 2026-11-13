// Included by end_to_end.rs: task preparation uses the existing real Loop fixture.
fn mq_e2e_world(tag: &str, slots: u32, restart: bool) -> (World, Vec<String>) {
    let mut w = World::new(tag);
    if restart {
        w.ctx.store = Store::open(w.root.join("merge.sqlite")).unwrap();
        let added = w.call("project.add", json!({"path":w.repo}));
        w.project_id = ok(&added)["project"]["id"].as_str().unwrap().to_owned();
        ok(&w.call(
            "project.onboarding.skip",
            json!({"project_id":w.project_id}),
        ));
    }
    let ids = ["queue first", "queue second", "queue third"]
        .map(|request| wait_for_automatic_merge(&mut w, request))
        .to_vec();
    let script = w.root.join("merge-gate.sh");
    std::fs::write(
        &script,
        format!(
            r#"set -e
if [ "$AUTOME_GATE_NODE" = merge_gate ]; then
  printf '%s %s %s %s\n' "$AUTOME_GATE_NODE" "$AUTOME_TASK" "$(git rev-parse main)" "$(git rev-parse HEAD)" >> '{}'
  git merge-base --is-ancestor main HEAD
  while [ ! -f '{}/release-'"$AUTOME_TASK" ]; do sleep 0.01; done
fi
"#,
            w.root.join("gate-calls").display(),
            w.root.display()
        ),
    )
    .unwrap();
    std::fs::write(
        w.root.join("autome-home/config.toml"),
        format!("[gate]\nconcurrency = {slots}\n"),
    )
    .unwrap();
    w.configure(&format!("[merge]\nauto = true\n[gate]\nrun = 'before_merge'\ncommand = 'sh {}'\nconcurrency = {slots}\n", script.display()));
    for id in &ids {
        w.ctx
            .store
            .set_task_state(
                id,
                &autome_domain::task::TaskState::Active {
                    node: autome_domain::task::Node::MergeGate,
                },
            )
            .unwrap();
    }
    (w, ids)
}
fn mq_e2e_wait(w: &mut World, ids: &[String], mut ready: impl FnMut(&World) -> bool) {
    queue_wait(w, |w| {
        for id in ids {
            assert!(
                setup_events(w, id, "gate.result")
                    .iter()
                    .all(|(_, p)| p["ok"] == true),
                "gate failed for {id}: {:?}",
                setup_events(w, id, "gate.result")
            );
            assert!(
                !setup_events(w, id, "task.updated")
                    .iter()
                    .any(|(_, p)| p["state"]["state"] == "queued"
                        && p["state"]["resume"] == "rebase"),
                "automatic merge fell behind: {id}"
            );
        }
        ready(w)
    });
}
fn mq_e2e_three(slots: u32, restart: bool) {
    mq_e2e_three_with_auth(slots, restart, false);
}
fn mq_e2e_three_with_auth(slots: u32, restart: bool, user: bool) {
    let (mut w, ids) = mq_e2e_world("mq-three", slots, restart);
    if user {
        let path = w.repo.join(".autome/config.toml");
        let config = std::fs::read_to_string(&path).unwrap().replace("auto = true", "auto = false");
        std::fs::write(path, config).unwrap();
        automed::git::commit_paths(&w.repo, &[".autome/config.toml"], "disable auto").unwrap();
        // Authorize all tasks before any gate is allowed to start.
        for id in &ids {
            w.ctx.store.set_task_state(id, &autome_domain::task::TaskState::Paused {
                resume: autome_domain::task::Node::MergeGate,
            }).unwrap();
        }
        for id in &ids {
            assert_eq!(ok(&w.call("task.merge", json!({"task_id":id})))["merge_request"], "authorized");
        }
        for id in &ids {
            w.ctx.store.set_task_state(id, &autome_domain::task::TaskState::Active {
                node: autome_domain::task::Node::MergeGate,
            }).unwrap();
        }
    }

    mq_e2e_wait(&mut w, &ids, |w| {
        !setup_events(w, &ids[0], "gate.started").is_empty()
    });
    for id in &ids[1..] {
        assert!(setup_events(&w, id, "gate.started").is_empty());
        assert_eq!(setup_events(&w, id, "gate.queued").len(), 1);
    }
    if restart {
        let store = Store::open(w.root.join("merge.sqlite")).unwrap();
        w.ctx = Ctx::new(store, w.root.join("autome-home"), w.root.join("home")).headless();
        automed::scheduler::recover(&mut w.ctx);
        queue_tick(&mut w);
        assert_eq!(
            automed::merge_queue::holder(&w.ctx, &w.project_id).unwrap(),
            Some(ids[0].clone())
        );
        assert_eq!(setup_events(&w, &ids[0], "gate.started").len(), 1);
    }
    for (i, id) in ids.iter().enumerate() {
        mq_e2e_wait(&mut w, &ids, |w| {
            !setup_events(w, id, "gate.started").is_empty()
        });
        for later in &ids[i + 1..] {
            assert!(setup_events(&w, later, "gate.started").is_empty());
        }
        std::fs::write(w.root.join(format!("release-{id}")), "go").unwrap();
        mq_e2e_wait(&mut w, &ids, |w| w.state(id) == "done");
    }
    let calls = std::fs::read_to_string(w.root.join("gate-calls")).unwrap();
    assert_eq!(calls.lines().count(), 3, "{calls}");
    for (line, id) in calls.lines().zip(&ids) {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields[0], "merge_gate");
        assert_eq!(fields[1], id);
        assert!(automed::git::is_ancestor(&w.repo, fields[2], fields[3]).unwrap());
        assert_eq!(setup_events(&w, id, "merge.auto").len(), usize::from(user));
        assert!(setup_events(&w, id, "merge.auto").iter().all(|(_, p)| p["by"] == "user"));
        let starts = setup_events(&w, id, "gate.started");
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].1["node"], "merge_gate");
        let reused = setup_events(&w, id, "gate.reused");
        assert_eq!(reused.len(), 1);
        assert_eq!(reused[0].1["node"], "main_check");

        assert_eq!(setup_events(&w, id, "merge.locked").len(), 1);
        assert_eq!(setup_events(&w, id, "merge.unlocked").len(), 1);
        assert!(
            !setup_events(&w, id, "task.updated")
                .iter()
                .any(|(_, p)| p["state"]["state"] == "queued" && p["state"]["resume"] == "rebase")
        );
    }
}
#[test]
fn merge_queue_three_auto_merging_tasks_run_the_gate_exactly_three_times() {
    mq_e2e_three(1, false);
}
#[test]
fn merge_queue_the_section_and_the_order_survive_a_core_restart() {
    mq_e2e_three(1, true);
}
#[test]
fn merge_queue_with_two_gate_slots_one_project_still_merges_one_at_a_time() {
    mq_e2e_three(2, false);
}
#[test]
fn merge_queue_a_press_during_the_section_waits_and_continues_after_a_gate_failure() {
    let mut w = World::new("mq-press");
    let a = wait_for_automatic_merge(&mut w, "gate holder");
    let b = wait_for_automatic_merge(&mut w, "manual merge");
    let script = w.root.join("failed-gate.sh");
    std::fs::write(
        &script,
        format!(
            "while [ ! -f '{}' ]; do sleep 0.01; done\nexit 1\n",
            w.root.join("release").display()
        ),
    )
    .unwrap();
    // Change only the ignored runtime config: B stays current so it can merge
    // immediately once A's failing gate releases the section.
    let path = w.repo.join(".autome/config.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_default()
        + &format!(
            "\n[merge]\nauto = true\n[gate]\ncommand = 'sh {}'\n",
            script.display()
        );
    std::fs::write(&path, text).unwrap();
    // The fixture configuration is tracked; commit it and align both branches
    // before starting the race, just as their preceding Rebase nodes do.
    automed::git::commit_paths(&w.repo, &[".autome/config.toml"], "config").unwrap();
    for id in [&a, &b] {
        let task = w.ctx.store.get_task(id).unwrap();
        automed::git::rebase(&w.repo.join(".worktree").join(task.slug), "main").unwrap();
    }
    w.ctx
        .store
        .set_task_state(
            &a,
            &autome_domain::task::TaskState::Active {
                node: autome_domain::task::Node::MergeGate,
            },
        )
        .unwrap();
    // Avoid launching an unrelated repair CLI after the intended gate failure.
    w.ctx.launch_mode = automed::launcher::LaunchMode::Dry;
    queue_wait(&mut w, |w| !setup_events(w, &a, "gate.started").is_empty());
    let head = automed::git::head_sha(&w.repo).unwrap();
    ok(&w.call("task.merge", json!({"task_id":b})));
    assert_eq!(w.node(&b).as_deref(), Some("merging"));
    assert_eq!(automed::git::head_sha(&w.repo).unwrap(), head);
    assert_eq!(setup_events(&w, &b, "merge.waiting").len(), 1);
    std::fs::write(w.root.join("release"), "go").unwrap();
    queue_wait(&mut w, |w| {
        w.ctx.store.get_task(&b).unwrap().merge_commit.is_some()
    });
    assert_eq!(setup_events(&w, &a, "gate.result")[0].1["ok"], false);
    assert!(setup_events(&w, &a, "merge.unlocked")[0].0 < setup_events(&w, &b, "merge.repo")[0].0);
}

#[test]
fn merge_auth_three_preauthorized_tasks_run_the_gate_exactly_three_times() {
    mq_e2e_three_with_auth(1, true, true);
}

fn ma_design(w: &mut World, request: &str) -> String {
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
        (5, doc("实现中", 2, 1, &[("M-01", "待审")])),
        (6, doc("实现中", 2, 1, &[("M-01", "已完成")])),
    ] {
        w.doc_step_for(&slug, n, &body, "");
    }
    w.retro_step(7, &slug);
    let retro = std::fs::read_to_string(w.repo.join(".autome/fake/7.sh")).unwrap();
    w.step_for(&slug, 7, &retro);
    let id = create_task(w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    id
}
#[test]
fn merge_auth_preauthorizing_before_design_approval_does_not_skip_it() {
    let mut w = World::new("ma-design");
    let id = ma_design(&mut w, "design permission");
    assert_eq!(ok(&w.call("task.merge", json!({"task_id":id})))["merge_request"], "authorized");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    ok(&w.call("task.approve", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
}
#[test]
fn merge_auth_a_preauthorized_task_merges_without_stopping_and_a_cancelled_one_stops() {
    for cancel in [false, true] {
        let mut w = World::new("ma-flow");
        w.configure("[gate]\nrun = 'before_merge'\ncommand = 'true'\n");
        let id = ma_design(&mut w, "authorized flow");
        ok(&w.call("task.merge", json!({"task_id":id})));
        if cancel {
            assert_eq!(ok(&w.call("task.merge", json!({"task_id":id,"cancel":true})))["merge_request"], "revoked");
        }
        ok(&w.call("task.approve", json!({"task_id":id})));
        w.settle();
        if cancel {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
            assert!(setup_events(&w, &id, "merge.direct").is_empty());
        } else {
            assert_eq!(w.state(&id), "done");
            assert_eq!(setup_events(&w, &id, "merge.direct").len(), 1);
        }
    }
}
