//! automed binary entrypoint: the blocking stdio loop described in plan
//! §3.1 — "stdout 仅承载协议帧，stderr 仅承载诊断信息". `tracing` is
//! therefore configured to write to stderr, never stdout, since stdout is
//! reserved exclusively for length-prefixed JSON-RPC frames read by
//! Electron Main.
//!
//! Every `Command` read from stdin produces exactly one `Reply` frame,
//! written before any `Event` frame it also produces — a caller waiting on
//! a response never hangs, even when dispatch fails or the frame is not a
//! valid `Command` at all (in which case `request_id`/`command_id` are
//! unknown, so the `Reply` carries empty strings for both and the failure
//! is also logged to stderr).

const USAGE: &str = "automed — Autome 的内核（桌面端经 stdio 与它通信）

用法：
  automed                         读 stdin 上的 JSON-RPC 帧（桌面端用）
  automed render-stream [runtime] 把 CLI 的 JSONL 渲染成可读日志（会话包装脚本用）
  automed protocol eval [dir] [--changed|--behaviour] [--tag X]
  automed --version | --help

人在终端里用的是 `autome` 命令，见 `autome --help`。
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // A filter, not the daemon: the session wrapper pipes a CLI's
        // `stream-json` output through it so the log a human opens is
        // readable. It must produce no tracing noise on stderr — stderr is
        // merged into the same pipe it is rendering.
        Some("render-stream") => std::process::exit(automed::entry::render_stream_main(&args[1..])),
        Some("protocol") if args.get(1).map(String::as_str) == Some("eval") => {
            std::process::exit(automed::entry::protocol_eval_main(&args[2..]))
        }
        Some("--help" | "-h" | "help") => {
            print!("{USAGE}");
            return;
        }
        Some("--version" | "-V") => {
            println!("{}", automed::entry::version_line("automed"));
            return;
        }
        _ => {}
    }

    std::process::exit(automed::entry::serve_stdio());
}
