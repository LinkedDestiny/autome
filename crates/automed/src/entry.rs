//! What the two executables share.
//!
//! `automed` is the core the desktop app talks to over stdio; `autome` is the
//! command a person types. Both open the same ledger, and both must answer
//! `render-stream` and `protocol eval` the same way: the session wrapper calls
//! back into whichever binary launched the session (`current_exe()`), so a
//! session started from the terminal renders its log through `autome`.
//!
//! Neither resolves the real home here — `config_io::only_the_entrypoint_…`
//! keeps that in the binaries — so every function takes the home it is given.

use std::path::{Path, PathBuf};

/// Where the ledger is: `AUTOMED_DB_PATH` if set, else under the home.
///
/// It used to default to `automed.sqlite3` in the working directory, which
/// meant running the core from a different directory silently started a
/// second, empty installation. One home, one ledger; the override is for the
/// test suites and for anyone who keeps state elsewhere on purpose.
pub fn ledger_path(autome_home: &Path) -> PathBuf {
    match std::env::var("AUTOMED_DB_PATH") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => crate::config_io::default_db_path(autome_home),
    }
}

/// `render-stream [runtime]`: a filter from a CLI's JSONL to a readable log.
/// Writes nothing else to stderr — stderr is merged into the same pipe.
pub fn render_stream_main(args: &[String]) -> i32 {
    // An absent or unknown name falls back to Claude, which is what every
    // wrapper written before the argument existed passes.
    let runtime = args
        .first()
        .and_then(|s| autome_domain::role::Runtime::parse(s))
        .unwrap_or(autome_domain::role::Runtime::Claude);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    match crate::stream_render::render_stream(runtime, stdin.lock(), &mut stdout) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("render-stream: {e}");
            1
        }
    }
}

/// `protocol eval [<dir>] [--changed | --behaviour] [--tag X]`: the gate a
/// protocol change has to pass before it ships. Prints the report and returns
/// the exit code.
pub fn protocol_eval_main(args: &[String]) -> i32 {
    // Defaults to the working directory: the version being proposed, not the
    // one built into this binary.
    let dir = PathBuf::from(
        args.iter()
            .find(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| ".".into()),
    );
    let (report, mut code) = crate::protocol::eval::run(&dir);
    print!("{report}");

    // The behaviour layer costs money, so it never runs unless asked for by
    // name. `--changed` is the audit round's form: the cases the proposed
    // changes reference, plus the three baselines.
    use crate::protocol::eval::Scope;
    let scope = if args.iter().any(|a| a == "--changed") {
        Some(Scope::Changed)
    } else if args.iter().any(|a| a == "--behaviour" || a == "--behavior") {
        Some(Scope::All)
    } else {
        None
    };
    if let Some(scope) = scope {
        let tag = args
            .iter()
            .position(|a| a == "--tag")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "未发布".into());
        let (behaviour, behaviour_code) = crate::protocol::eval::run_behaviour(&dir, scope, &tag);
        print!("{behaviour}");
        code = code.max(behaviour_code);
    }
    code
}

/// The version both binaries report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The CLI and standalone kernel share one compiled identity.
pub fn version_line(name: &str) -> String {
    format!("{name} {VERSION} ({})", crate::build_id::BUILD_ID)
}
