//! The IPC surface for curation: rule proposals, removal experiments, and the
//! backfill that tells a past change whether it was right.

use autome_domain::lesson::{AggregatedLesson, Lesson, LessonDomain};
use serde_json::{Value, json};

use crate::curation::{self, ExperimentVerdict, ProposalState};
use crate::dispatch::{Ctx, DispatchResult, rejected};

/// Every task's lessons, aggregated, plus the order the tasks ran in.
///
/// The two halves are only useful together. The aggregation says which tasks
/// a lesson key was seen in; the ordered list says how many tasks have run
/// since — which is what tells a rule that is still earning its place from one
/// that has gone quiet.
struct Lessons {
    /// Slugs of the tasks that wrote lessons, oldest first.
    ///
    /// Tasks that wrote none are left out on purpose. A task that never
    /// reached the retro round had no opportunity to mention a rule, so
    /// counting it as a task that failed to mention one would retire rules
    /// faster than they earned.
    task_slugs: Vec<String>,
    aggregated: Vec<AggregatedLesson>,
}

/// One pass over every task's lessons.
///
/// Cheap and idempotent, so it runs whenever the panel asks rather than being
/// scheduled: the answer changes only when a task ends, and asking then costs
/// one pass over lessons the core already has in the event stream.
fn walk_lessons(
    ctx: &mut Ctx,
    project_id: &str,
) -> Result<Lessons, crate::dispatch::DispatchError> {
    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let mut per_task: Vec<(String, Vec<Lesson>)> = Vec::new();
    for task in ctx.store.list_tasks(project_id)? {
        let lessons = ctx
            .store
            .last_event(&task.id, "task.lessons")?
            .and_then(|p| p.get("lessons").cloned())
            .and_then(|v| serde_json::from_value::<Vec<Lesson>>(v).ok())
            .or_else(|| {
                std::fs::read_to_string(repo.join(task.doc_dir()).join("lessons.md"))
                    .ok()
                    .and_then(|t| autome_domain::lesson::parse(&t).ok())
            })
            .unwrap_or_default();
        if !lessons.is_empty() {
            per_task.push((task.slug.clone(), lessons));
        }
    }
    let aggregated = autome_domain::lesson::aggregate(
        per_task
            .iter()
            .map(|(slug, l)| (slug.as_str(), l.as_slice()))
            .collect::<Vec<_>>(),
    );
    Ok(Lessons {
        task_slugs: per_task.into_iter().map(|(slug, _)| slug).collect(),
        aggregated,
    })
}

/// Recomputes a project's rule proposals and records them.
fn refresh(
    ctx: &mut Ctx,
    project_id: &str,
    lessons: &Lessons,
) -> Result<Vec<curation::Proposal>, crate::dispatch::DispatchError> {
    let answered = ctx.store.answered_rule_proposals(project_id)?;
    let proposals = curation::proposals(&lessons.aggregated, &answered);
    for p in &proposals {
        ctx.store.upsert_rule_proposal(
            project_id,
            &p.key,
            p.domain.as_str(),
            &p.proposal,
            &p.evidence,
        )?;
    }
    Ok(proposals)
}

/// The rules Autome wrote, with how long each has gone unmentioned.
///
/// Nothing new is recorded to produce this. An approved proposal *is* a rule
/// in the file: the row carries the lesson key it was written for and the
/// exact sentence that was written, and the aggregation already knows every
/// task whose lessons named that key. A rule is idle for each task that wrote
/// lessons after the last one that named it.
///
/// Three exclusions, all deliberate:
///
/// - **Rules that have ever been the subject of an experiment.** A running one
///   is not in the file to remove. A restored one was removed, something got
///   worse, and it went back — which is this mechanism's strongest possible
///   evidence that the rule is carrying its weight, and offering to remove it
///   again on the next quiet stretch would be asking the user to re-run an
///   experiment that already answered. Quietness is why it is asked about at
///   all, so "it has been quiet again since" does not reopen the question.
/// - **Rules whose key no longer appears in any lesson at all.** That means
///   the lessons they came from are gone — deleted, or the task archived — and
///   a rule whose origin cannot be shown is one this mechanism has nothing to
///   say about. The panel's whole argument is "here is what it was written
///   for, and here is how long since anything needed it".
/// - **Rules a person wrote by hand.** They have no proposal row, so Autome
///   has no provenance for them and cannot say how long they have been idle.
///   Offering to remove one would be the cleanup this mechanism exists to
///   avoid.
fn retirement_rules(
    ctx: &mut Ctx,
    project_id: &str,
    lessons: &Lessons,
) -> Result<Vec<curation::Rule>, crate::dispatch::DispatchError> {
    // Every state, not just `running` — see the note above on restored rules.
    let already_experimented: Vec<String> = ctx
        .store
        .list_rule_experiments(project_id)?
        .into_iter()
        .map(|row| row.2)
        .collect();

    let mut rules = Vec::new();
    for (key, domain, proposal, _evidence, state) in ctx.store.list_rule_proposals(project_id)? {
        if state != curation::ProposalState::Approved.as_str() {
            continue;
        }
        if already_experimented.contains(&proposal) {
            continue;
        }
        let Some(agg) = lessons
            .aggregated
            .iter()
            .find(|l| curation::proposal_key(l) == key)
        else {
            continue;
        };
        // `tasks` is first-seen order over the same list, so its last entry is
        // the most recent task whose lessons named this rule.
        let Some(newest) = agg.tasks.last() else {
            continue;
        };
        let Some(at) = lessons.task_slugs.iter().position(|s| s == newest) else {
            continue;
        };
        let idle_tasks = (lessons.task_slugs.len() - 1 - at) as u32;

        let domain = LessonDomain::parse(&domain).unwrap_or(LessonDomain::Process);
        rules.push(curation::Rule {
            file: format!(".autome/rules/{}.md", domain.as_str()),
            body: proposal,
            idle_tasks,
        });
    }
    Ok(rules)
}

/// `rules.proposals` — what the project page's 建议入规 card shows.
pub fn proposals(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let lessons = walk_lessons(ctx, project_id)?;
    let proposals = refresh(ctx, project_id, &lessons)?;
    let retirements = curation::removal_experiments(&retirement_rules(ctx, project_id, &lessons)?);
    let today = crate::store::now_iso();
    let date = today.split('T').next().unwrap_or(&today).to_string();
    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);

    Ok((
        json!({
            "proposals": proposals.iter().map(|p| {
                let file = p.rule_file();
                let existing = std::fs::read_to_string(repo.join(&file)).ok();
                json!({
                    "key": p.key,
                    "domain": p.domain.as_str(),
                    "proposal": p.proposal,
                    "tasks": p.tasks,
                    "evidence": p.evidence,
                    "file": file,
                    // The exact text that will be added, not a description of
                    // it: what the user approves has to be what gets written.
                    "diff": p.rule_text(&date),
                    "file_exists": existing.is_some(),
                })
            }).collect::<Vec<_>>(),
            "experiments": experiments_json(ctx, project_id)?,
            // Rules old enough to be worth an experiment. Offered, never
            // acted on: `RETIREMENT_IDLE_TASKS` says when to ask, and the
            // user says whether to try.
            "retirement_candidates": retirements.iter().map(|e| json!({
                "file": e.rule_file,
                "body": e.body,
                "metric": e.metric,
                "idle_tasks": e.idle_tasks,
                "horizon": e.predicted.horizon,
                "direction": e.predicted.direction.as_str(),
            })).collect::<Vec<_>>(),
        }),
        vec![],
    ))
}

/// `rules.decide` — approve or dismiss a proposal.
pub fn decide(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 project_id"))?;
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 key"))?;
    let approve = params
        .get("approve")
        .and_then(Value::as_bool)
        .ok_or_else(|| rejected("缺少 approve"))?;

    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let rows = ctx.store.list_rule_proposals(project_id)?;
    let Some((_, domain, proposal, evidence, _)) = rows.into_iter().find(|(k, ..)| k == key) else {
        return Err(rejected(format!("没有这条建议：{key}")));
    };
    let domain = LessonDomain::parse(&domain).unwrap_or(LessonDomain::Process);

    if !approve {
        ctx.store
            .set_rule_proposal_state(project_id, key, ProposalState::Dismissed.as_str())?;
        let seq = ctx
            .store
            .append_event("rule.dismissed", project_id, json!({ "key": key }))?;
        return Ok((
            json!({ "state": "dismissed" }),
            vec![crate::dispatch::event(
                seq,
                "rule.dismissed",
                project_id,
                json!({}),
            )],
        ));
    }

    let today = crate::store::now_iso();
    let date = today.split('T').next().unwrap_or(&today).to_string();
    let p = curation::Proposal {
        key: key.to_string(),
        domain,
        proposal,
        evidence,
        tasks: vec![],
    };
    let file = p.rule_file();
    let path = repo.join(&file);
    let existing = std::fs::read_to_string(&path).ok();
    let text = curation::apply(existing.as_deref(), domain, &p.rule_text(&date));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, text).map_err(|e| rejected(format!("无法写入 {file}：{e}")))?;
    let _ = crate::git::commit_paths(
        &repo,
        &[&file],
        &format!("chore(autome): 入规 {}", p.proposal),
    );

    ctx.store
        .set_rule_proposal_state(project_id, key, ProposalState::Approved.as_str())?;
    let seq = ctx.store.append_event(
        "rule.approved",
        project_id,
        json!({ "key": key, "file": file }),
    )?;
    Ok((
        json!({ "state": "approved", "file": file }),
        vec![crate::dispatch::event(
            seq,
            "rule.approved",
            project_id,
            json!({}),
        )],
    ))
}

fn experiments_json(
    ctx: &mut Ctx,
    project_id: &str,
) -> Result<Value, crate::dispatch::DispatchError> {
    let rows = ctx.store.list_rule_experiments(project_id)?;
    let mut out = Vec::new();
    for (id, file, body, metric, baseline, horizon, removed_at, state, outcome) in rows {
        let after: Vec<f64> = ctx
            .store
            .tasks_completed_after(project_id, &removed_at)?
            .iter()
            .filter_map(|m| m.value(&metric))
            .collect();
        let verdict = curation::judge(baseline, &after, horizon);
        out.push(json!({
            "id": id,
            "file": file,
            "body": body,
            "metric": metric,
            "baseline": baseline,
            "horizon": horizon,
            "samples": after.len(),
            "state": state,
            "outcome": outcome,
            "verdict": match verdict {
                ExperimentVerdict::TooEarly => "还没到期",
                ExperimentVerdict::Held => "没有变差，可以就这样",
                ExperimentVerdict::Regressed => "变差了，建议放回来",
            },
        }));
    }
    Ok(json!(out))
}

/// `rules.retire` — remove a rule as an experiment.
pub fn retire(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 project_id"))?;
    let file = params
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 file"))?;
    let body = params
        .get("body")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 body"))?;

    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let path = repo.join(file);
    let existing =
        std::fs::read_to_string(&path).map_err(|e| rejected(format!("读不到 {file}：{e}")))?;
    if !existing.contains(body) {
        return Err(rejected(format!("{file} 里没有这条规则")));
    }

    let domain = LessonDomain::parse(
        file.rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    )
    .unwrap_or(LessonDomain::Process);
    let metric = curation::metric_for(domain);

    // The baseline is what the metric looked like *with* the rule in place.
    // Without it the later verdict has nothing to compare to, and the
    // experiment is just a deletion with extra steps.
    let history: Vec<f64> = ctx
        .store
        .tasks_with_metrics(project_id)?
        .iter()
        .filter_map(|(_, m)| m.value(metric))
        .collect();
    if history.is_empty() {
        return Err(rejected(
            "这个项目还没有任何带指标的已终结任务，移除实验没有基线可比。",
        ));
    }
    let baseline = history.iter().sum::<f64>() / history.len() as f64;

    std::fs::write(&path, curation::remove(&existing, body))
        .map_err(|e| rejected(format!("无法写入 {file}：{e}")))?;
    let _ = crate::git::commit_paths(&repo, &[file], &format!("chore(autome): 移除实验 · {body}"));

    let id = ctx.store.start_rule_experiment(
        project_id,
        file,
        body,
        metric,
        baseline,
        curation::EXPERIMENT_HORIZON,
    )?;
    let seq = ctx.store.append_event(
        "rule.removal_experiment",
        project_id,
        json!({ "id": id, "file": file, "metric": metric, "baseline": baseline }),
    )?;
    Ok((
        json!({ "id": id, "metric": metric, "baseline": baseline,
                "horizon": curation::EXPERIMENT_HORIZON }),
        vec![crate::dispatch::event(
            seq,
            "rule.removal_experiment",
            project_id,
            json!({}),
        )],
    ))
}

/// `rules.restore` — put a rule back after an experiment went badly.
pub fn restore(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 project_id"))?;
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 id"))?;

    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let rows = ctx.store.list_rule_experiments(project_id)?;
    let Some((_, file, body, ..)) = rows.into_iter().find(|(i, ..)| i == id) else {
        return Err(rejected(format!("没有这个实验：{id}")));
    };

    let domain = LessonDomain::parse(
        file.rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    )
    .unwrap_or(LessonDomain::Process);
    let path = repo.join(&file);
    let existing = std::fs::read_to_string(&path).ok();
    let today = crate::store::now_iso();
    let date = today.split('T').next().unwrap_or(&today);
    let text = curation::apply(
        existing.as_deref(),
        domain,
        &format!("<!-- since: {date} · 移除实验后放回 -->\n- {body}\n"),
    );
    std::fs::write(&path, text).map_err(|e| rejected(format!("无法写入 {file}：{e}")))?;
    let _ = crate::git::commit_paths(&repo, &[&file], &format!("chore(autome): 放回 {body}"));

    ctx.store
        .finish_rule_experiment(id, "restored", "指标变差，规则放回")?;
    let seq = ctx.store.append_event(
        "rule.restored",
        project_id,
        json!({ "id": id, "file": file }),
    )?;
    Ok((
        json!({ "file": file }),
        vec![crate::dispatch::event(
            seq,
            "rule.restored",
            project_id,
            json!({}),
        )],
    ))
}
