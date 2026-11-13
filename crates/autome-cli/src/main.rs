//! `autome` — the Loop from a terminal.
//!
//! The desktop app is one way to drive Autome; this is the other. It opens the
//! same ledger under `~/.autome`, so a task started here shows up on the
//! dashboard and a design approved there can be merged from here. Nothing
//! moves on its own: `autome run` (or the open desktop app) is what ticks the
//! scheduler, and the two take turns through the core's driver lock.

mod args;
mod client;
mod render;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use args::Args;
use client::{CliError, Client, Result};
use serde_json::{Value, json};

// Keep closed output pipes local to CLI output; changing SIGPIPE would also
// affect the embedded driver's writes to child processes.
macro_rules! print {
    ($($arg:tt)*) => {
        print_stdout(format_args!($($arg)*))
    };
}

macro_rules! println {
    () => { print!("\n") };
    ($($arg:tt)*) => { print!("{}\n", format_args!($($arg)*)) };
}

fn print_stdout(args: std::fmt::Arguments<'_>) {
    let mut stdout = std::io::stdout().lock();
    if let Err(error) = stdout.write_fmt(args).and_then(|()| stdout.flush()) {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        panic!("failed printing to stdout: {error}");
    }
}

const USAGE: &str = "autome — 一句话需求，经设计、评审、实现、审计，到合并

用法：autome <命令> [参数]

开始
  doctor                         检查 Git、Claude Code、Codex 与登录状态
  project add <路径> [--workspace] [--docs-repo 仓] [--skip-onboarding]
  project ls | rm <项目> | onboard <项目> [--finish]
  task new [需求] [--kind feature|bug] [--brief <文件|->] [--draft] [--after 任务,…] [-p 项目] [--attach 文件]... [--doc 路径]...

  after <任务> <任务,…> | --none  修改或清空依赖
  task start <任务>              开始待开始任务
  task convert <任务> [--start]  把判定为「不是 Bug」的任务改建成需求（原任务取消）

推进
  run [--once | --until-idle] [--interval 秒] [--json]
                                 驱动 Loop；Ctrl-C 只停驱动，会话照常跑完
  status [任务]                  不带任务：等你决定 / 运行中 / 报警；带任务：详情
  ls                             所有任务（含已完成）
  logs <任务> [--session 会话] [-f] [--json]

卡点与控制
  queue move <任务> --before X | --after X | --top
  approve <任务>                 批准设计
  reject <任务> \"意见\"           驳回设计，意见原样交给下一轮设计
  reply <任务> \"补充信息\"       Bug 任务在等你补充信息/等你决定、失败或已停止时用；需求任务等同 reject
  merge <任务> [--cancel]        合并到默认分支；未到卡点时预先同意，--cancel 撤销
  decide <任务> backlog|dispute <条目> include|ignore|ruled|none
  pause | resume | stop | cancel <任务>
  rerun <任务> <节点>            从某个节点重跑（失败或已停止的任务）
  extend <任务> <轮数>           追加实现轮次（预算用完的任务）
  alerts [dismiss <报警>]        项目级报警，如合并后 main 门禁失败

其他
  core                           以 stdio 提供内核（面板用）
  rpc <方法> ['<JSON 参数>']     直接调用内核的任意 JSON-RPC 方法
  version | help

通用：--json 输出原始 JSON；-p/--project 用 id、名称或路径指定项目，
不写时取当前目录所在的已注册项目；<任务> 可以写 id 或 slug 前缀。

状态在 $AUTOME_HOME（默认 ~/.autome），与桌面端共用。
项目的门禁、重试与预算在仓库的 .autome/config.toml 里配置（[gate] / [retry] / [budget]）。
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    // The session wrapper calls back into whichever binary launched the
    // session, so these two must behave exactly as `automed`'s do — and
    // before anything else, because `render-stream` writes into the log.
    match argv.first().map(String::as_str) {
        Some("core") => std::process::exit(automed::entry::serve_stdio()),
        Some("render-stream") => std::process::exit(automed::entry::render_stream_main(&argv[1..])),
        Some("protocol") if argv.get(1).map(String::as_str) == Some("eval") => {
            std::process::exit(automed::entry::protocol_eval_main(&argv[2..]))
        }
        _ => {}
    }

    let code = match run(&argv) {
        Ok(()) => 0,
        Err(e) => {
            match &e {
                CliError::Usage(m) => eprintln!("autome：{m}\n用 `autome help` 看用法。"),
                CliError::Failed(m) => eprintln!("autome：{m}"),
            }
            e.exit_code()
        }
    };
    std::process::exit(code);
}

fn run(argv: &[String]) -> Result<()> {
    let a = Args::parse(argv)?;
    let verb = a.positional.first().map(String::as_str);
    if a.has("--help") || verb == Some("help") {
        print!("{USAGE}");
        return Ok(());
    }
    if a.has("--version") || verb == Some("version") {
        println!("{}", automed::entry::version_line("autome"));
        return Ok(());
    }
    if verb.is_none() {
        print!("{USAGE}");
        return Ok(());
    }

    let verb = verb.unwrap_or_default();
    let allowed: &[&str] = match verb {
        "doctor" | "ls" | "alerts" | "rpc" | "approve" | "pause" | "resume" | "stop" | "cancel"
        | "reject" | "reply" | "rerun" | "extend" | "decide" => &["--json"],
        "after" => &["--json", "--none"],
        "queue" => &["--json", "--before", "--after", "--top"],
        "status" => &["--json"],
        "project" => &[
            "--json",
            "--workspace",
            "--docs-repo",
            "--skip-onboarding",
            "--finish",
        ],
        "merge" => &["--json", "--cancel"],
        "task" => &[
            "--json",
            "--project",
            "--attach",
            "--doc",
            "--kind",
            "--brief",
            "--draft",
            "--after",
            "--start",
        ],
        "run" => &[
            "--once",
            "--until-idle",
            "--interval",
            "--verbose",
            "--json",
        ],
        "logs" => &["--session", "--follow", "--json"],
        other => return Err(CliError::Usage(format!("未知命令 `{other}`"))),
    };
    if let Some(flag) = a.unknown(allowed) {
        return Err(CliError::Usage(format!("`{verb}` 不认识 {flag}")));
    }

    let mut c = Client::open()?;
    if !a.has("--json") {
        c.ctx.driver_lock_notice = Some(|| eprintln!("等待驱动释放状态库…"));
    }
    match verb {
        "doctor" => doctor(&mut c, &a),
        "project" => project(&mut c, &a),
        "task" => task(&mut c, &a),
        "after" => set_after(&mut c, &a),
        "queue" => queue_move(&mut c, &a),
        "status" => status(&mut c, &a),
        "ls" => list(&mut c, &a),
        "run" => drive(&mut c, &a),
        "logs" => logs(&mut c, &a),
        "approve" => trigger(&mut c, &a, "task.approve", json!({})),
        "merge" => trigger(
            &mut c,
            &a,
            "task.merge",
            json!({"cancel": a.has("--cancel")}),
        ),
        "pause" => trigger(&mut c, &a, "task.pause", json!({})),
        "resume" => trigger(&mut c, &a, "task.resume", json!({})),
        "stop" => trigger(&mut c, &a, "task.stop", json!({})),
        "cancel" => trigger(&mut c, &a, "task.cancel", json!({})),
        "reject" => {
            let feedback = a.need(2, "驳回意见")?.to_string();
            trigger(&mut c, &a, "task.reject", json!({ "feedback": feedback }))
        }
        "reply" => {
            let feedback = a.need(2, "补充信息")?;
            trigger(&mut c, &a, "task.reply", json!({ "info": feedback }))
        }
        "rerun" => {
            let node = a
                .need(2, "节点（如 design、implement、audit）")?
                .to_string();
            trigger(&mut c, &a, "task.rerun_from", json!({ "node": node }))
        }
        "extend" => {
            let n: u32 = a
                .need(2, "追加的轮数")?
                .parse()
                .map_err(|_| CliError::Usage("轮数必须是正整数".into()))?;
            trigger(
                &mut c,
                &a,
                "task.extend_budget",
                json!({ "extra_rounds": n }),
            )
        }
        "decide" => decide(&mut c, &a),
        "alerts" => alerts(&mut c, &a),
        "rpc" => rpc(&mut c, &a),
        _ => unreachable!("checked above"),
    }
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

// ---------------------------------------------------------------------------
// doctor
// ---------------------------------------------------------------------------

fn doctor(c: &mut Client, a: &Args) -> Result<()> {
    use autome_domain::environment::{Login, Severity};
    let kernel = c.call("env.get", json!({}))?["kernel"].clone();
    let env = automed::env_probe::probe_all();
    if a.has("--json") {
        print_json(&json!({
            "home": c.home,
            "ledger": c.db,
            "driver_lock": automed::driver_lock::lock_path(&c.db),
            "binary": std::env::current_exe().ok(),
            "kernel": kernel,
            "environment": env,
            "severity": env.severity(),
        }));
    } else {
        println!("autome {}", automed::entry::VERSION);
        println!(
            "内核：{}（账本记录：{}）",
            kernel["build_id"].as_str().unwrap_or_default(),
            kernel["ledger_build_id"].as_str().unwrap_or("还没有")
        );
        if let Some(message) = kernel["message"].as_str() {
            println!("{message}");
        }
        println!("  家目录    {}", c.home.display());
        println!("  状态库    {}", c.db.display());
        if let Ok(exe) = std::env::current_exe() {
            println!("  本程序    {}", exe.display());
        }
        println!();
        for comp in &env.components {
            let name = comp.component.display_name();
            if !comp.present {
                println!("  ✗ {name:<10} 没有找到");
                continue;
            }
            let login = match &comp.login {
                Login::NotApplicable => String::new(),
                Login::Ok { account_hint } => match account_hint {
                    Some(h) => format!("  已登录（{h}）"),
                    None => "  已登录".into(),
                },
                Login::Expired => "  登录已失效".into(),
                Login::Unknown { detail } => format!("  登录状态未知：{detail}"),
            };
            let mark = if comp.login.is_blocking() {
                "✗"
            } else {
                "✓"
            };
            println!(
                "  {mark} {name:<10} {}{login}    {}",
                comp.version.as_deref().unwrap_or(""),
                comp.path.as_deref().unwrap_or("")
            );
        }
        let in_path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
            .is_some_and(|dir| {
                std::env::var("PATH")
                    .unwrap_or_default()
                    .split(':')
                    .any(|p| std::path::Path::new(p) == dir)
            });
        if !in_path {
            println!("\n  提示：本程序所在目录不在 PATH 里，新开的终端可能找不到 `autome`。");
        }
    }
    match env.severity() {
        Severity::Blocking => Err(CliError::Failed(
            "有阻塞问题：缺少 Git，或两个 CLI 都不可用 / 登录失效".into(),
        )),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// projects and tasks
// ---------------------------------------------------------------------------

fn project(c: &mut Client, a: &Args) -> Result<()> {
    match a.positional.get(1).map(String::as_str) {
        Some("add") => {
            let raw = a.need(2, "项目路径")?;
            let path =
                std::fs::canonicalize(raw).map_err(|e| CliError::Failed(format!("{raw}：{e}")))?;
            let mut params = json!({
                "path": path.to_string_lossy(),
                "workspace": a.has("--workspace"),
            });
            if let Some(repo) = a.value("--docs-repo") {
                params["docs_repo"] = json!(repo);
            }
            let added = c.call("project.add", params)?;
            let id = added["project"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            if a.has("--skip-onboarding") {
                c.call("project.onboarding.skip", json!({ "project_id": id }))?;
            }
            if a.has("--json") {
                print_json(&added);
            } else {
                println!(
                    "已注册 {}  {}",
                    id,
                    added["project"]["display_name"]
                        .as_str()
                        .unwrap_or_default()
                );
                if !a.has("--skip-onboarding") {
                    println!(
                        "可选：`autome project onboard {id}` 让 Agent 读一遍仓库，写项目画像与 AGENTS.md；\n不做就 `autome project onboard {id} --finish` 直接完成。"
                    );
                }
            }
            Ok(())
        }
        Some("ls") | None => {
            let list = c.call("project.list", json!({}))?;
            if a.has("--json") {
                print_json(&list);
                return Ok(());
            }
            for e in list["projects"].as_array().cloned().unwrap_or_default() {
                let p = &e["project"];
                if !p["removed_at"].is_null() {
                    continue;
                }
                println!(
                    "{}  {:<20} {}",
                    p["id"].as_str().unwrap_or_default(),
                    p["display_name"].as_str().unwrap_or_default(),
                    p["path"].as_str().unwrap_or_default()
                );
            }
            Ok(())
        }
        Some("rm") => {
            let p = c.resolve_project(Some(a.need(2, "项目")?))?;
            c.call("project.remove", json!({ "project_id": p["id"] }))?;
            println!(
                "已移除 {}（目录本身没有动）",
                p["display_name"].as_str().unwrap_or_default()
            );
            Ok(())
        }
        Some("onboard") => {
            let p = c.resolve_project(a.positional.get(2).map(String::as_str))?;
            let id = p["id"].as_str().unwrap_or_default().to_string();
            if a.has("--finish") {
                // The desktop wizard's remaining steps are screens to look at;
                // here they are the files themselves, which the user has
                // already read and edited. Walk the wizard to its end, which
                // is where the core makes the init commit.
                let mut last = Value::Null;
                for _ in 0..8 {
                    match c.call("project.onboarding.advance", json!({ "project_id": id })) {
                        Ok(out) => {
                            let done = out["onboarding"]["onboarding"] == json!("completed");
                            last = out;
                            if done {
                                break;
                            }
                        }
                        Err(_) if !last.is_null() => break,
                        Err(e) => return Err(e),
                    }
                }
                if a.has("--json") {
                    print_json(&last);
                } else {
                    println!(
                        "Onboarding 已完成{}",
                        last["init_commit"]
                            .as_str()
                            .map(|c| format!("，init commit {}", render::short(c)))
                            .unwrap_or_default()
                    );
                }
                return Ok(());
            }
            let out = c.call("project.onboarding.run", json!({ "project_id": id }))?;
            if a.has("--json") {
                print_json(&out);
            } else {
                println!("已启动 Onboarding 会话：读一遍仓库，起草项目画像与 AGENTS.md。");
                println!("它和任务一样由 `autome run` 收尾。跑完后看一眼、按需修改：");
                println!("    docs/agent-project-profile.md    AGENTS.md");
                println!("然后 `autome project onboard {id} --finish` 完成并提交 init commit。");
            }
            Ok(())
        }
        Some(other) => Err(CliError::Usage(format!("project 没有子命令 `{other}`"))),
    }
}

fn resolve_after(c: &mut Client, lists: &[String]) -> Result<Vec<String>> {
    lists
        .iter()
        .flat_map(|s| s.split(','))
        .map(|name| {
            if name.trim().is_empty() {
                return Err(CliError::Usage("依赖列表不能包含空任务名".into()));
            }
            c.resolve_task(name.trim())
        })
        .collect()
}

fn set_after(c: &mut Client, a: &Args) -> Result<()> {
    let list = a.positional.get(2);
    if a.positional.len() > 3 || list.is_some() == a.has("--none") {
        return Err(CliError::Usage("依赖列表和 --none 必须且只能给一个".into()));
    }
    let id = c.resolve_task(a.need(1, "任务")?)?;
    let after = match list {
        Some(list) => resolve_after(c, std::slice::from_ref(list))?,
        None => vec![],
    };
    let out = c.call("task.set_after", json!({"task_id":id,"after":after}))?;
    if a.has("--json") {
        print_json(&out);
    } else {
        println!(
            "{}",
            render::event_line("task.after_set", &id, &json!({"after":after})).unwrap()
        );
    }
    Ok(())
}

fn queue_move(c: &mut Client, a: &Args) -> Result<()> {
    let count =
        a.values("--before").len() + a.values("--after").len() + usize::from(a.has("--top"));
    if a.positional.get(1).map(String::as_str) != Some("move")
        || a.positional.len() != 3
        || count != 1
    {
        return Err(CliError::Usage(
            "用法：queue move <任务> --before X | --after X | --top".into(),
        ));
    }
    let id = c.resolve_task(a.need(2, "任务")?)?;
    let mut params = json!({"task_id":id});
    for flag in ["--before", "--after"] {
        if let Some(name) = a.value(flag) {
            if name.trim().is_empty() {
                return Err(CliError::Usage("参照任务不能为空".into()));
            }
            params[&flag[2..]] = json!(c.resolve_task(name)?);
        }
    }
    if a.has("--top") {
        params["top"] = json!(true);
    }
    let out = c.call("task.move", params.clone())?;
    if a.has("--json") {
        print_json(&out);
    } else {
        println!(
            "{}",
            render::event_line("task.moved", &id, &params).unwrap()
        );
    }
    Ok(())
}

fn task(c: &mut Client, a: &Args) -> Result<()> {
    if a.has("--after") && a.positional.get(1).map(String::as_str) != Some("new") {
        return Err(CliError::Usage("--after 只用于 task new".into()));
    }

    match a.positional.get(1).map(String::as_str) {
        Some("new") => {
            let request = a.positional[2..].join(" ");
            if request.trim().is_empty() && !a.has("--brief") {
                return Err(CliError::Usage("缺少参数：一句话需求或 --brief".into()));
            }
            let p = c.resolve_project(a.value("--project"))?;
            let attachments: Vec<String> = a
                .values("--attach")
                .iter()
                .map(|f| {
                    std::fs::canonicalize(f)
                        .map(|p| p.to_string_lossy().into_owned())
                        .map_err(|e| CliError::Failed(format!("附件 {f}：{e}")))
                })
                .collect::<Result<_>>()?;
            let mut params = json!({
                "project_id": p["id"],
                "request": request,
                "attachments": attachments,
                "doc_refs": a.values("--doc"),
            });
            if request.trim().is_empty() && a.has("--brief") {
                params.as_object_mut().unwrap().remove("request");
            }
            if let Some(kind) = a.value("--kind") {
                params["kind"] = json!(kind);
            }
            if let Some(path) = a.value("--brief") {
                let bytes = if path == "-" {
                    let mut bytes = Vec::new();
                    std::io::stdin()
                        .read_to_end(&mut bytes)
                        .map_err(|e| CliError::Failed(format!("读取任务卡正文：{e}")))?;
                    bytes
                } else {
                    std::fs::read(path)
                        .map_err(|e| CliError::Failed(format!("读取任务卡正文 {path}：{e}")))?
                };
                params["brief"] = json!(
                    String::from_utf8(bytes)
                        .map_err(|_| CliError::Failed("任务卡正文不是 UTF-8 文本".into()))?
                );
            }
            if a.has("--draft") {
                params["draft"] = json!(true);
            }
            if a.has("--after") {
                params["after"] = json!(resolve_after(c, &a.values("--after"))?);
            }
            let out = c.call("task.create", params)?;
            if a.has("--json") {
                print_json(&out);
            } else {
                let t = &out["task"];
                println!(
                    "{}  {}  {}",
                    t["id"].as_str().unwrap_or_default(),
                    t["slug"].as_str().unwrap_or_default(),
                    render::task_state_line(t)
                );
                if t["state"]["state"] == "draft" {
                    println!(
                        "已存为待开始。用 `autome task start {}` 开始。",
                        t["id"].as_str().unwrap_or_default()
                    );
                } else {
                    println!("Loop 由 `autome run` 或打开的桌面端推进。");
                }
            }
            Ok(())
        }
        Some("convert") => {
            let id = c.resolve_task(a.need(2, "任务")?)?;
            let mut params = json!({ "task_id": id, "kind": "feature" });
            if a.has("--start") {
                params["start"] = json!(true);
            }
            let out = c.call("task.convert", params)?;
            if a.has("--json") {
                print_json(&out);
            } else {
                let t = &out["task"];
                let new_id = t["id"].as_str().unwrap_or_default();
                println!(
                    "{}  {}  {}",
                    new_id,
                    t["slug"].as_str().unwrap_or_default(),
                    render::task_state_line(t)
                );
                println!("由 {id} 改建，{id} 已取消。");
                if t["state"]["state"] == "draft" {
                    println!("已存为待开始。用 `autome task start {new_id}` 开始。");
                } else {
                    println!("Loop 由 `autome run` 或打开的桌面端推进。");
                }
            }
            Ok(())
        }
        Some("start") => {
            let id = c.resolve_task(a.need(2, "任务")?)?;
            let out = c.call("task.start", json!({ "task_id": id }))?;
            if a.has("--json") {
                print_json(&out);
            } else {
                let t = &out["task"];
                println!("{}  {}", id, render::task_state_line(t));
                if let Some(next) = render::next_step(t) {
                    println!("→ {next}");
                }
            }
            Ok(())
        }
        Some("ls") | None => list(c, a),
        Some(other) => Err(CliError::Usage(format!("task 没有子命令 `{other}`"))),
    }
}

/// Every task, newest last, with where it stands.
fn list(c: &mut Client, a: &Args) -> Result<()> {
    let tasks = c.all_tasks()?;
    if a.has("--json") {
        print_json(&json!(tasks.iter().map(|(_, t)| t).collect::<Vec<_>>()));
        return Ok(());
    }
    if tasks.is_empty() {
        println!("还没有任务。`autome task new \"一句话需求\"` 新建一个。");
    }
    let mut queued: Vec<_> = tasks.iter().filter(|(_, t)| render::is_queued(t)).collect();
    queued.sort_by_key(|(p, t)| {
        (
            p["id"].as_str().unwrap_or_default(),
            t["queue_position"].as_u64().unwrap_or(u64::MAX),
        )
    });
    let ordered = tasks
        .iter()
        .filter(|(_, t)| !render::is_queued(t))
        .chain(queued);
    let mut queue_heading = false;
    for (p, t) in ordered {
        if render::is_queued(t) && !queue_heading {
            println!("\n排队（按出队顺序）");
            queue_heading = true;
        }

        println!(
            "{}  {:<28} {:<16} {}{}  {}{}",
            t["id"].as_str().unwrap_or_default(),
            t["slug"].as_str().unwrap_or_default(),
            p["display_name"].as_str().unwrap_or_default(),
            render::task_state_line(t),
            render::merge_suffix(t, true),
            render::cost_cell(t),
            render::blocked_suffix(t)
        );
    }
    if tasks
        .iter()
        .any(|(_, t)| render::cost_cell(t).ends_with('*'))
    {
        println!("* 有会话没有计费数据（如 Codex），实际花费不低于所示金额");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

fn status(c: &mut Client, a: &Args) -> Result<()> {
    if let Some(named) = a.positional.get(1) {
        let id = c.resolve_task(named)?;
        let got = c.call("task.get", json!({ "task_id": id }))?;
        if a.has("--json") {
            print_json(&got);
        } else {
            task_detail(&got);
        }
        return Ok(());
    }

    let d = c.call("dashboard.get", json!({}))?;
    if a.has("--json") {
        print_json(&d);
        return Ok(());
    }
    for project in d["config_warnings"].as_array().into_iter().flatten() {
        print_config_warnings(
            &project["warnings"],
            project["project_name"].as_str().unwrap_or(""),
        );
    }
    let alerts = d["alerts"].as_array().cloned().unwrap_or_default();
    let waiting = d["waiting"].as_array().cloned().unwrap_or_default();
    let running = d["running"].as_array().cloned().unwrap_or_default();
    let cooldowns = d["cooldowns"].as_array().cloned().unwrap_or_default();

    if !alerts.is_empty() {
        println!("报警");
        for al in &alerts {
            let first = al["detail"]
                .as_str()
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or_default();
            println!("  {}  {first}", al["id"].as_str().unwrap_or_default());
            println!(
                "      → autome alerts dismiss {}",
                al["id"].as_str().unwrap_or_default()
            );
        }
        println!();
    }
    println!("等你决定");
    if waiting.is_empty() {
        println!("  （没有）");
    }
    for e in &waiting {
        let t = &e["task"];
        println!(
            "  {}  {:<28} {}{}  ·  {}",
            t["id"].as_str().unwrap_or_default(),
            t["slug"].as_str().unwrap_or_default(),
            render::task_state_line(t),
            render::merge_suffix(t, false),
            e["project_name"].as_str().unwrap_or_default()
        );
        if let Some(next) = render::next_step(t) {
            println!("      → {next}");
        }
    }
    println!("\n运行中");
    if running.is_empty() {
        println!("  （没有）");
    }
    for e in &running {
        let t = &e["task"];
        let mut line = format!(
            "  {}  {:<28} {}{}",
            t["id"].as_str().unwrap_or_default(),
            t["slug"].as_str().unwrap_or_default(),
            render::task_state_line(t),
            render::merge_suffix(t, false)
        );
        let cost = render::cost_cell(t);
        if !cost.is_empty() {
            line.push_str(&format!("  · {cost}"));
        }
        if let Some(s) = e["session"].as_object() {
            line.push_str(&format!(
                "  · {} #{}",
                s.get("runtime").and_then(Value::as_str).unwrap_or("?"),
                s.get("round").cloned().unwrap_or(Value::Null)
            ));
        }
        if let Some(r) = e["retry"].as_object() {
            line.push_str(&format!(
                "  · {} 再试",
                render::local_time(r.get("not_before").and_then(Value::as_str).unwrap_or(""))
            ));
        }
        if let Some(position) = e["gate_queue_position"].as_u64() {
            line.push_str(&format!("  · {}", render::gate_queue_line(position)));
        }
        println!("{line}");
    }
    if let Some(queued) = d["queued"].as_array().filter(|q| !q.is_empty()) {
        println!("\n排队");
        for e in queued {
            let t = &e["task"];
            println!(
                "  {} {} 排队 · 第 {} 位{}",
                t["id"].as_str().unwrap_or_default(),
                t["slug"].as_str().unwrap_or_default(),
                e["queue_position"],
                render::blocked_suffix(t)
            );
        }
    }
    if !cooldowns.is_empty() {
        println!("\nCLI 冷却");
        for cd in &cooldowns {
            println!(
                "  {:<6} 冷却到 {}  {}",
                cd["runtime"].as_str().unwrap_or_default(),
                render::local_time(cd["until"].as_str().unwrap_or_default()),
                cd["reason"].as_str().unwrap_or_default()
            );
        }
    }
    let env = &d["environment"];
    if env["probed"] == json!(true) && env["severity"] == json!("blocking") {
        println!("\n环境有阻塞问题：`autome doctor` 查看。");
    }
    Ok(())
}

fn task_detail(got: &Value) {
    let t = &got["task"];
    println!(
        "{}  {}",
        t["id"].as_str().unwrap_or_default(),
        t["title"].as_str().unwrap_or_default()
    );
    print_config_warnings(
        &got["config_warnings"],
        got["project"]["name"].as_str().unwrap_or(""),
    );
    println!(
        "  状态      {}{}",
        render::task_state_line(t),
        render::merge_suffix(t, false)
    );

    let dependencies = render::dependencies(t);
    if !dependencies.is_empty() {
        println!("  依赖      {dependencies}");
    }
    if let Some(cost) = got.get("cost") {
        println!(
            "  费用      {}（{} 个会话，其中 {} 个无计费数据）",
            render::usd(cost["usd_total"].as_f64().unwrap_or(0.0)),
            cost["sessions"].as_u64().unwrap_or(0),
            cost["sessions_without_cost"].as_u64().unwrap_or(0)
        );
    }
    println!(
        "  项目      {}  分支 {}",
        got["project"]["name"].as_str().unwrap_or_default(),
        t["branch"].as_str().unwrap_or_default()
    );
    if let Some(position) = got["gate_queue_position"].as_u64() {
        println!("  门禁排队  {}", render::gate_queue_line(position));
    }
    println!("  需求      {}", t["request"].as_str().unwrap_or_default());
    if let Some(sb) = got["status_block"].as_object() {
        let ms: Vec<String> = sb
            .get("milestones")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|m| {
                format!(
                    "{} {}",
                    m["id"].as_str().unwrap_or("?"),
                    render::milestone_label(m["state"].as_str().unwrap_or("?"))
                )
            })
            .collect();
        println!(
            "  轮次      设计 {}/{} · 实现 {}/{}",
            sb.get("design_round").cloned().unwrap_or_default(),
            sb.get("design_round_limit").cloned().unwrap_or_default(),
            sb.get("impl_round").cloned().unwrap_or_default(),
            t["budget_n"]
                .as_u64()
                .map(|n| n.to_string())
                .unwrap_or_else(|| sb
                    .get("impl_round_limit")
                    .map(|v| v.to_string())
                    .unwrap_or_default())
        );
        if !ms.is_empty() {
            println!("  里程碑    {}", ms.join(" · "));
        }
    }
    if got["gate"].is_object() {
        println!("{}", render::gate_status_line(&got["gate"]));
    }
    if let Some(r) = got["retry"].as_object() {
        // Rate limits are not counted, so a round only waiting out a cooldown
        // has crashed zero times — saying so would read as a contradiction.
        let crashed = match r.get("attempts").and_then(Value::as_u64) {
            Some(n) if n > 0 => format!("（已崩溃 {n} 次）"),
            _ => String::new(),
        };
        println!(
            "  重试      {} 在 {} 后再跑{crashed}",
            render::node_label(r.get("node").and_then(Value::as_str).unwrap_or("?")),
            render::local_time(r.get("not_before").and_then(Value::as_str).unwrap_or("")),
        );
    }
    if let Some(line) = render::scope_status_line(&got["scope_restore"]) {
        println!("{line}");
    }
    let decisions = got["decisions"].as_array().cloned().unwrap_or_default();
    let open: Vec<&Value> = decisions
        .iter()
        .filter(|d| d["consumed"] != json!(true))
        .collect();
    if !open.is_empty() {
        println!("  待你决定");
        for d in open {
            let state = if d["carried"] == json!(true) {
                "已交给进行中的一轮".to_string()
            } else {
                d["disposition"].as_str().unwrap_or("none").to_string()
            };
            println!(
                "    {} {}  [{}]",
                d["item_id"].as_str().unwrap_or_default(),
                d["text"].as_str().unwrap_or_default(),
                state
            );
        }
        println!(
            "    → autome decide {} backlog|dispute <条目> include|ignore|ruled",
            t["id"].as_str().unwrap_or_default()
        );
    }
    let sessions = got["sessions"].as_array().cloned().unwrap_or_default();
    if !sessions.is_empty() {
        println!("  会话（最近 5 个）");
        for s in sessions.iter().take(5).rev() {
            let cost = s["metrics"]["cost_usd"]
                .as_f64()
                .map(|c| format!("  ${c:.2}"))
                .unwrap_or_default();
            println!(
                "    {}  {} #{}  {}  {}{}{}",
                s["id"].as_str().unwrap_or_default(),
                s["label"].as_str().unwrap_or_default(),
                s["round"],
                s["runtime"].as_str().unwrap_or_default(),
                render::local_time(s["started_at"].as_str().unwrap_or_default()),
                if s["running"] == json!(true) {
                    format!(
                        "  运行中 · 已运行 {} 分钟 / 上限 {}",
                        s["elapsed_minutes"], s["limit_minutes"]
                    )
                } else if s["timed_out"] == json!(true) {
                    format!("  超时（上限 {} 分钟）", s["limit_minutes"])
                } else {
                    String::new()
                },
                cost
            );
        }
    }
    if let Some(next) = render::next_step(t) {
        println!("  → {next}");
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn catch_sigint() {
    let handler: extern "C" fn(libc::c_int) = on_sigint;
    // SAFETY: installs a handler that only stores to an atomic.
    unsafe {
        libc::signal(libc::SIGINT, handler as *const () as libc::sighandler_t);
    }
}

/// The driver: ticks the scheduler on an interval and prints what changed.
///
/// Ctrl-C finishes the tick in progress and stops driving. Sessions keep
/// running — they are in process groups of their own — and the next `run`,
/// or the desktop app, collects them.
fn drive(c: &mut Client, a: &Args) -> Result<()> {
    let interval: f64 = a
        .value("--interval")
        .map(|v| {
            v.parse()
                .map_err(|_| CliError::Usage("--interval 必须是秒数".into()))
        })
        .transpose()?
        .unwrap_or(3.0);
    let once = a.has("--once");
    let until_idle = a.has("--until-idle");
    let verbose = a.has("--verbose");
    let json = a.has("--json");
    catch_sigint();

    let mut seq = c.call("events.since", json!({ "after_seq": u64::MAX }))?["latest_seq"]
        .as_u64()
        .unwrap_or(0);
    if !once && !json {
        eprintln!("驱动 Loop 中（每 {interval} 秒一轮，Ctrl-C 停止驱动；会话不受影响）");
    }
    let mut told_skipped = false;
    let mut older_message: Option<String> = None;
    loop {
        let report = c.call("scheduler.tick", json!({}))?;
        let older = &report["older_kernel"];
        let blocked = older["message"].as_str();
        if let Some(message) = blocked {
            if once || until_idle {
                if json {
                    println!("{}", json!({"older_kernel": older}));
                    std::process::exit(1);
                }
                return Err(CliError::Failed(message.into()));
            }
            if older_message.as_deref() != Some(message) {
                if json {
                    println!("{}", json!({"older_kernel": older}));
                } else {
                    eprintln!("{message}");
                }
                older_message = Some(message.into());
            }
        } else if older_message.take().is_some() {
            if json {
                println!("{}", json!({"older_kernel": null}));
            } else {
                eprintln!("账本已允许本内核推进，继续驱动");
            }
        }
        if blocked.is_none() && report["skipped"] == json!(true) && !told_skipped && !json {
            eprintln!("另一个驱动（桌面端或另一个 autome run）正在推进，本进程轮流推进。");
            told_skipped = true;
        }
        for e in report["errors"].as_array().cloned().unwrap_or_default() {
            if json {
                println!("{}", json!({"error": e.as_str().unwrap_or_default()}));
            } else {
                eprintln!("  ! {}", e.as_str().unwrap_or_default());
            }
        }
        loop {
            let got = c.call("events.since", json!({ "after_seq": seq, "limit": 500 }))?;
            let events = got["events"].as_array().cloned().unwrap_or_default();
            if events.is_empty() {
                break;
            }
            for e in &events {
                seq = seq.max(e["seq"].as_u64().unwrap_or(seq));
                let kind = e["kind"].as_str().unwrap_or_default();
                let subject = e["subject"].as_str().unwrap_or_default();
                if json {
                    println!("{}", run_event(e, c.ctx.store.get_task(subject).is_ok()));
                    continue;
                }
                match render::event_line(kind, subject, &e["payload"]) {
                    Some(line) => println!("{}  {line}", clock()),
                    None if verbose => println!("{}  {subject} {kind}", clock()),
                    None => {}
                }
            }
            let _ = std::io::stdout().flush();
        }
        if once || INTERRUPTED.load(Ordering::SeqCst) {
            break;
        }
        if until_idle && idle(c) {
            if json {
                let unfinished = c
                    .ctx
                    .store
                    .list_unfinished()
                    .map_err(|e| CliError::Failed(e.to_string()))?;
                let all = c.all_tasks()?;
                let tasks: Vec<Value> = unfinished
                    .iter()
                    .filter_map(|t| {
                        all.iter().find(|(_, v)| v["id"] == t.id).map(
                            |(_, v)| json!({"id": v["id"], "slug": v["slug"], "state": v["state"]}),
                        )
                    })
                    .collect();
                println!("{}", json!({"idle": true, "tasks": tasks}));
            }
            break;
        }
        // Slept in small steps so Ctrl-C is answered promptly.
        let mut left = interval;
        while left > 0.0 && !INTERRUPTED.load(Ordering::SeqCst) {
            let step = left.min(0.2);
            std::thread::sleep(Duration::from_secs_f64(step));
            left -= step;
        }
        if INTERRUPTED.load(Ordering::SeqCst) {
            break;
        }
    }
    if INTERRUPTED.load(Ordering::SeqCst) && !json {
        eprintln!("已停止驱动。还在跑的会话不受影响，下次 `autome run` 或打开桌面端会接着收结果。");
    }
    Ok(())
}

/// Nothing will happen without a person or the clock: no session running, no
/// task queued or on a core step (gates included), no retry due.
fn idle(c: &mut Client) -> bool {
    let store = &c.ctx.store;
    if !store.all_running_sessions().unwrap_or_default().is_empty() {
        return false;
    }
    let now = automed::store::now_iso();
    !store.list_unfinished().unwrap_or_default().iter().any(|t| {
        use autome_domain::task::TaskState;
        match &t.state {
            TaskState::Queued | TaskState::Requeued { .. } => true,
            TaskState::Active { node } if node.is_core_step() => true,
            TaskState::Active { .. } => matches!(
                store.pending_retry(&t.id),
                Ok(Some((_, ref at, _))) if *at <= now
            ),
            _ => false,
        }
    })
}

fn run_event(event: &Value, is_task: bool) -> Value {
    json!({"seq": event["seq"], "time": event["at"],
        "task_id": if is_task { event["subject"].clone() } else { Value::Null },
        "kind": event["kind"], "subject": event["subject"], "payload": event["payload"]})
}

fn clock() -> String {
    render::local_time(&automed::store::now_iso())
}

// ---------------------------------------------------------------------------
// logs
// ---------------------------------------------------------------------------

fn logs_json(c: &mut Client, a: &Args, id: &str, session: &Value, log_path: &str) -> Result<()> {
    let runtime =
        autome_domain::role::Runtime::parse(session["runtime"].as_str().unwrap_or_default())
            .ok_or_else(|| CliError::Failed("不认识的会话 runtime".into()))?;
    let path = std::path::Path::new(log_path).with_extension("jsonl");
    let sid = session["id"].as_str().unwrap_or_default();
    let mut shown = 0;
    let mut line = 0;
    catch_sigint();
    loop {
        // Check lifecycle before reading so an ended session's final write is included.
        let running = c
            .ctx
            .store
            .list_sessions(id)
            .map_err(|e| CliError::Failed(e.to_string()))?
            .iter()
            .any(|s| s.id == sid && s.is_running());
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && running => Vec::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CliError::Failed(format!(
                    "会话 {sid} 没有原始流 {}；不带 --json 的 `autome logs` 可以看渲染日志",
                    path.display()
                )));
            }
            Err(e) => return Err(CliError::Failed(format!("读取 {}：{e}", path.display()))),
        };
        if bytes.len() > shown {
            let end = shown + render::stream_complete_bytes(&bytes[shown..], !running);
            for raw in bytes[shown..end].split_inclusive(|b| *b == b'\n') {
                line += 1;
                if let Some(event) = render::stream_event(raw, line, runtime) {
                    println!("{event}");
                }
            }
            shown = end;
            let _ = std::io::stdout().flush();
        }
        if !a.has("--follow") || !running || INTERRUPTED.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

fn logs(c: &mut Client, a: &Args) -> Result<()> {
    let id = c.resolve_task(a.need(1, "任务")?)?;
    let got = c.call("task.get", json!({ "task_id": id }))?;
    let sessions = got["sessions"].as_array().cloned().unwrap_or_default();
    let session = match a.value("--session") {
        Some(s) => sessions
            .iter()
            .find(|x| x["id"].as_str().is_some_and(|i| i.starts_with(s)))
            .cloned()
            .ok_or_else(|| CliError::Failed(format!("任务 {id} 没有会话 `{s}`")))?,
        None if a.has("--json") => sessions
            .first()
            .cloned()
            .ok_or_else(|| CliError::Failed(format!("任务 {id} 还没有会话")))?,
        None => sessions
            .last()
            .cloned()
            .ok_or_else(|| CliError::Failed(format!("任务 {id} 还没有会话")))?,
    };
    let path = session["log_path"].as_str().unwrap_or_default().to_string();
    if a.has("--json") {
        return logs_json(c, a, &id, &session, &path);
    }
    let mut shown = 0usize;
    catch_sigint();
    loop {
        let text = std::fs::read(&path).unwrap_or_default();
        if text.len() > shown {
            std::io::stdout()
                .write_all(&text[shown..])
                .map_err(|e| CliError::Failed(e.to_string()))?;
            let _ = std::io::stdout().flush();
            shown = text.len();
        }
        if !a.has("--follow") || INTERRUPTED.load(Ordering::SeqCst) {
            break;
        }
        // Follow until the session has ended and its last bytes are out.
        let sid = session["id"].as_str().unwrap_or_default();
        let still = c
            .ctx
            .store
            .list_sessions(&id)
            .unwrap_or_default()
            .iter()
            .any(|s| s.id == sid && s.is_running());
        if !still
            && std::fs::metadata(&path)
                .map(|m| m.len() as usize)
                .unwrap_or(0)
                <= shown
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// stopping points and control
// ---------------------------------------------------------------------------

/// A command about one task: resolves the task, calls the method, prints where
/// the task now stands.
fn trigger(c: &mut Client, a: &Args, method: &str, mut params: Value) -> Result<()> {
    let id = c.resolve_task(a.need(1, "任务")?)?;
    params["task_id"] = json!(id);
    let out = c.call(method, params)?;
    if a.has("--json") {
        print_json(&out);
        return Ok(());
    }
    if let Some(request) = out["merge_request"].as_str() {
        println!(
            "{} {}",
            id,
            match request {
                "authorized" | "already_authorized" => "已预先同意合并",
                "revoked" => "已撤销预先同意合并",
                _ => "开始合并",
            }
        );
    }
    let task = if out["task"].is_object() {
        out["task"].clone()
    } else {
        c.call("task.get", json!({ "task_id": id }))?["task"].clone()
    };
    println!("{}  {}", id, render::task_state_line(&task));
    if let Some(next) = render::next_step(&task) {
        println!("→ {next}");
    }
    Ok(())
}

fn decide(c: &mut Client, a: &Args) -> Result<()> {
    let id = c.resolve_task(a.need(1, "任务")?)?;
    let kind = a.need(2, "backlog 或 dispute")?.to_string();
    let item = a.need(3, "条目编号")?.to_string();
    let disposition = a.need(4, "include / ignore / ruled / none")?.to_string();
    let out = c.call(
        "task.decide",
        json!({ "task_id": id, "kind": kind, "item_id": item, "disposition": disposition }),
    )?;
    if a.has("--json") {
        print_json(&out);
    } else {
        println!("{id} {item} → {disposition}（下一轮会带上它）");
    }
    Ok(())
}

fn alerts(c: &mut Client, a: &Args) -> Result<()> {
    if a.positional.get(1).map(String::as_str) == Some("dismiss") {
        let id = a.need(2, "报警编号")?.to_string();
        c.call("project.alert.dismiss", json!({ "alert_id": id }))?;
        println!("已关闭 {id}");
        return Ok(());
    }
    let d = c.call("dashboard.get", json!({}))?;
    let alerts = d["alerts"].as_array().cloned().unwrap_or_default();
    if a.has("--json") {
        print_json(&json!(alerts));
        return Ok(());
    }
    if alerts.is_empty() {
        println!("没有未处理的报警。");
    }
    for al in &alerts {
        println!(
            "{}  {}  {}",
            al["id"].as_str().unwrap_or_default(),
            al["kind"].as_str().unwrap_or_default(),
            render::local_time(al["created_at"].as_str().unwrap_or_default())
        );
        for line in al["detail"].as_str().unwrap_or_default().lines() {
            println!("    {line}");
        }
    }
    Ok(())
}

fn rpc(c: &mut Client, a: &Args) -> Result<()> {
    let method = a.need(1, "方法名，如 dashboard.get")?.to_string();
    let params: Value = match a.positional.get(2) {
        Some(raw) => serde_json::from_str(raw)
            .map_err(|e| CliError::Usage(format!("参数不是合法 JSON：{e}")))?,
        None => json!({}),
    };
    print_json(&c.call(&method, params)?);
    Ok(())
}

fn print_config_warnings(warnings: &Value, project: &str) {
    for warning in warnings
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        println!("注意：项目 {project}：{warning}");
    }
}

#[cfg(test)]
mod idle_tests {
    use super::*;
    use autome_domain::project::{AddDisposition, Onboarding, Project, ProjectKind};
    use autome_domain::task::{Node, TaskState};
    use automed::dispatch::Ctx;
    use automed::store::{Store, TaskRecord, now_iso};

    fn client() -> Client {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_project(&Project {
                id: "p".into(),
                path: "/unused".into(),
                display_name: "p".into(),
                default_branch: "main".into(),
                parallel_limit: 1,
                onboarding: Onboarding::Skipped,
                disposition: AddDisposition::AdoptedExisting,
                added_at: now_iso(),
                removed_at: None,
                kind: ProjectKind::Repo,
                members: vec![],
                docs_repo: None,
            })
            .unwrap();
        Client {
            ctx: Ctx::new(store, "/unused", "/unused"),
            home: "/unused".into(),
            db: ":memory:".into(),
        }
    }

    fn insert(c: &Client, id: &str, state: TaskState) {
        c.ctx
            .store
            .insert_task(&TaskRecord {
                kind: autome_domain::task::TaskKind::Feature,
                id: id.into(),
                project_id: "p".into(),
                slug: id.into(),
                title: id.into(),
                request: "test".into(),
                attachments: vec![],
                doc_refs: vec![],
                state,
                budget_n: None,
                created_at: now_iso(),
                completed_at: None,
                merge_commit: None,
                archived_at: None,
                protocol_ref: None,
                rules_hash: None,
                metrics: None,
                doc_root: None,
            })
            .unwrap();
    }

    #[test]
    fn run_event_task_identity() {
        let c = client();
        insert(&c, "not-a-prefix", TaskState::Queued);
        let e = json!({"subject":"not-a-prefix", "payload":{"text":"a\nb"}});
        let v = run_event(&e, c.ctx.store.get_task("not-a-prefix").is_ok());
        assert_eq!(v["task_id"], "not-a-prefix");
        assert_eq!(v["payload"], e["payload"]);
        assert_eq!(v.to_string().lines().count(), 1);
    }

    #[test]
    fn run_event_project_preserves_subject() {
        let c = client();
        let e = json!({"subject":"p"});
        let v = run_event(&e, c.ctx.store.get_task("p").is_ok());
        assert!(v["task_id"].is_null());
        assert_eq!(v["subject"], "p");
    }

    #[test]
    fn run_event_time_is_stored_at() {
        let e = json!({"seq":42, "at":"2000-02-29T23:59:59Z"});
        let v = run_event(&e, false);
        assert_eq!(v["time"], e["at"]);
        assert_eq!(v["seq"], 42);
    }

    #[test]
    fn requeued_keeps_until_idle_running_while_slot_holder_cools_down() {
        let mut c = client();
        insert(
            &c,
            "holder",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        c.ctx
            .store
            .schedule_retry(
                "holder",
                Node::Implement,
                "9999-12-31T23:59:59Z",
                None,
                "cooldown",
                false,
            )
            .unwrap();
        assert_eq!(c.ctx.store.slots_in_use("p").unwrap(), 1);
        assert!(
            idle(&mut c),
            "future retry alone retains existing idle behavior"
        );
        insert(
            &c,
            "resumed",
            TaskState::Requeued {
                resume: Node::Audit,
            },
        );
        assert!(!idle(&mut c), "resume queue must keep the driver alive");
        c.ctx
            .store
            .set_task_state("resumed", &TaskState::Cancelled)
            .unwrap();
        assert!(
            idle(&mut c),
            "cancelling the queued task removes pending work"
        );
    }

    #[test]
    fn idle_empty_and_single_task_boundaries() {
        let mut c = client();
        assert!(idle(&mut c));
        insert(&c, "task", TaskState::Queued);
        assert!(!idle(&mut c));
        for state in [
            TaskState::Requeued {
                resume: Node::Implement,
            },
            TaskState::Requeued { resume: Node::Gate },
            TaskState::Active { node: Node::Gate },
        ] {
            c.ctx.store.set_task_state("task", &state).unwrap();
            assert!(!idle(&mut c), "{state:?}");
        }
        for state in [
            TaskState::Paused {
                resume: Node::Implement,
            },
            TaskState::Stopped { at: Node::Audit },
            TaskState::Cancelled,
        ] {
            c.ctx.store.set_task_state("task", &state).unwrap();
            assert!(idle(&mut c), "{state:?}");
        }
    }
}

#[cfg(test)]
mod task_card {
    #[test]
    fn usage_lists_task_start_reply_and_the_new_flags() {
        for text in [
            "task start <任务>",
            "reply <任务>",
            "--kind",
            "--brief",
            "--draft",
        ] {
            assert!(super::USAGE.contains(text), "{text}");
        }
    }
}

#[cfg(test)]
mod bug_usage_tests {
    #[test]
    fn bug_loop_usage_no_longer_says_reply_equals_reject() {
        assert!(!super::USAGE.contains("Bug Loop 上线前"));
        assert!(
            super::USAGE
                .contains("Bug 任务在等你补充信息/等你决定、失败或已停止时用；需求任务等同 reject")
        );
    }
}

#[test]
fn deps_cli_usage() {
    for text in [
        "[--after 任务,…]",
        "after <任务> <任务,…> | --none",
        "queue move <任务> --before X | --after X | --top",
    ] {
        assert!(USAGE.contains(text));
    }
}
