//! Best-effort cleanup of processes whose executable or cwd belongs to a task.
//! Never select by name; revalidate identity, paths and controlling terminal
//! before every signal and every liveness check.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

#[derive(Clone)]
struct Process {
    pid: i32,
    start: (u64, u64),
    uid: u32,
    zombie: bool,
    tty: Option<String>,
    exe: Result<PathBuf, String>,
    cwd: Result<PathBuf, String>,
    command: String,
}
trait ProcessTable {
    fn pids(&mut self) -> Result<Vec<i32>, String>;
    fn read(&mut self, pid: i32) -> Result<Option<Process>, String>;
    fn signal(&mut self, pid: i32, signal: i32) -> Result<(), String>;
    fn wait(&mut self);
}
#[derive(Default)]
struct Report {
    processes: Vec<Value>,
    survivors: Vec<Value>,
    unchecked: BTreeMap<i32, Value>,
    released: BTreeSet<i32>,
}
fn roots(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for path in paths {
        if path.as_os_str().is_empty() {
            continue;
        }
        if let Ok(real) = path.canonicalize() {
            roots.push(real);
        }
        roots.push(path);
    }
    roots.sort();
    roots.dedup();
    roots
}
fn matched(p: &Process, roots: &[PathBuf]) -> Option<&'static str> {
    [("exe", &p.exe), ("cwd", &p.cwd)]
        .into_iter()
        .find_map(|(kind, path)| {
            path.as_ref()
                .ok()
                .filter(|path| roots.iter().any(|root| path.starts_with(root)))
                .map(|_| kind)
        })
}
fn warning(report: &mut Report, p: &Process, kind: &str, reason: String) {
    let mut value = json!({"pid":p.pid,"command":p.command.chars().take(120).collect::<String>(),"kind":kind,"reason":reason});
    if let Some(tty) = &p.tty {
        value["tty"] = json!(tty);
    }
    report.unchecked.insert(p.pid, value);
}
fn eligible(p: &Process, roots: &[PathBuf], report: &mut Report) -> Option<&'static str> {
    if let Some(kind) = matched(p, roots) {
        if p.tty.is_some() {
            warning(report, p, "tty", "有控制终端，可能是用户自己打开的".into());
            None
        } else {
            Some(kind)
        }
    } else {
        let errors = [("exe", &p.exe), ("cwd", &p.cwd)]
            .into_iter()
            .filter_map(|(name, path)| path.as_ref().err().map(|error| format!("{name}: {error}")))
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            warning(report, p, "unknown", errors.join("；"));
        }
        None
    }
}
fn recheck(
    table: &mut impl ProcessTable,
    p: &Process,
    roots: &[PathBuf],
    report: &mut Report,
) -> Option<Process> {
    match table.read(p.pid) {
        Ok(Some(now)) if now.start == p.start && now.uid == p.uid && !now.zombie => {
            if eligible(&now, roots, report).is_some() {
                Some(now)
            } else {
                report.released.insert(p.pid);
                if let Some(warning) = report.unchecked.get_mut(&p.pid)
                    && warning["kind"] == "unknown"
                {
                    warning["reason"] = json!(format!(
                        "信号前无法复核身份：{}",
                        warning["reason"].as_str().unwrap_or_default()
                    ));
                }
                None
            }
        }
        Err(error) => {
            report.released.insert(p.pid);
            warning(report, p, "unknown", format!("信号前无法复核身份：{error}"));
            None
        }
        _ => None,
    }
}
fn run(table: &mut impl ProcessTable, roots: &[PathBuf], uid: u32, own: i32) -> Report {
    let mut report = Report::default();
    let pids = match table.pids() {
        Ok(pids) => pids,
        Err(error) => {
            report.unchecked.insert(
                0,
                json!({"pid":0,"command":"","kind":"unknown","reason":error}),
            );
            return report;
        }
    };
    let mut candidates = Vec::new();
    for pid in pids {
        if pid == own || pid <= 0 {
            continue;
        }
        // EPERM and vanished processes cannot be attributed to this user.
        if let Ok(Some(p)) = table.read(pid)
            && p.uid == uid
            && !p.zombie
            && eligible(&p, roots, &mut report).is_some()
        {
            candidates.push(p);
        }
    }
    let mut sent = BTreeMap::new();
    let mut errors = BTreeMap::new();
    for (signal, label, polls) in [(libc::SIGTERM, "TERM", 60), (libc::SIGKILL, "KILL", 20)] {
        candidates.retain_mut(|p| {
            let Some(now) = recheck(table, p, roots, &mut report) else { return false; };
            let kind = matched(&now, roots).expect("rechecked path");
            match table.signal(p.pid, signal) {
                Ok(()) => { sent.insert(p.pid, json!({"pid":p.pid,"command":now.command.chars().take(120).collect::<String>(),"matched":kind,"signal":label})); }
                Err(error) => { errors.insert(p.pid, error); }
            }
            *p = now;
            true
        });
        for _ in 0..polls {
            if candidates.is_empty() {
                break;
            }
            table.wait();
            candidates.retain(|p| recheck(table, p, roots, &mut report).is_some());
        }
    }
    for p in candidates {
        sent.remove(&p.pid);
        report.survivors.push(json!({"pid":p.pid,"command":p.command.chars().take(120).collect::<String>(),"error":errors.remove(&p.pid).unwrap_or_else(|| "SIGKILL 后仍存活".into())}));
    }
    report.processes = sent
        .into_iter()
        .filter_map(|(pid, value)| (!report.released.contains(&pid)).then_some(value))
        .collect();
    report
}

pub(crate) fn sweep(
    store: &crate::store::Store,
    task_id: &str,
    layout: &crate::layout::TaskLayout,
) {
    let roots = roots(
        layout
            .repos
            .iter()
            .map(|slot| slot.worktree.clone())
            .chain([layout.cwd.clone()]),
    );
    let report = run(
        &mut Native,
        &roots,
        unsafe { libc::getuid() },
        std::process::id() as i32,
    );
    record(store, task_id, roots, report);
}
fn record(store: &crate::store::Store, task_id: &str, roots: Vec<PathBuf>, report: Report) {
    if (!report.processes.is_empty() || !report.survivors.is_empty())
        && let Err(error) = store.append_event(
            "task.processes_stopped",
            task_id,
            json!({"checkouts":roots,"processes":report.processes,"survivors":report.survivors}),
        )
    {
        tracing::warn!(%error, "cannot record process cleanup");
    }
    if !report.unchecked.is_empty()
        && let Err(error) = store.append_event(
            "task.processes_unchecked",
            task_id,
            json!({"processes":report.unchecked.into_values().collect::<Vec<_>>()}),
        )
    {
        tracing::warn!(%error, "cannot record unchecked processes");
    }
}
struct Native;
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::ffi::CStr;
    use std::mem::{size_of, zeroed};
    use std::os::unix::ffi::OsStrExt;
    fn info<T>(pid: i32, flavor: i32) -> std::io::Result<T> {
        let mut value: T = unsafe { zeroed() };
        let count = unsafe {
            libc::proc_pidinfo(
                pid,
                flavor,
                0,
                (&mut value as *mut T).cast(),
                size_of::<T>() as i32,
            )
        };
        if count == size_of::<T>() as i32 {
            Ok(value)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    fn path(bytes: &[u8]) -> PathBuf {
        PathBuf::from(std::ffi::OsStr::from_bytes(
            bytes.split(|b| *b == 0).next().unwrap_or_default(),
        ))
    }
    fn command(pid: i32, exe: &Result<PathBuf, String>) -> String {
        let fallback = || {
            exe.as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
        let mut bytes = vec![0u8; 262144];
        let mut len = bytes.len();
        if unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                bytes.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        } != 0
            || len < 4
        {
            return fallback();
        }
        let argc = i32::from_ne_bytes(bytes[..4].try_into().unwrap()).max(0) as usize;
        let fields = bytes[4..len]
            .split(|b| *b == 0)
            .filter(|part| !part.is_empty())
            .skip(1)
            .take(argc);
        let text = fields
            .map(|part| String::from_utf8_lossy(part))
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() {
            fallback()
        } else {
            format!("{} {text}", fallback())
        }
    }
    impl ProcessTable for Native {
        fn pids(&mut self) -> Result<Vec<i32>, String> {
            let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
            if count <= 0 {
                return Err(format!("无法枚举进程：{}", std::io::Error::last_os_error()));
            }
            let mut pids = vec![0i32; count as usize * 2 + 128];
            let n = unsafe {
                libc::proc_listallpids(
                    pids.as_mut_ptr().cast(),
                    (pids.len() * size_of::<i32>()) as i32,
                )
            };
            if n <= 0 {
                return Err(format!("无法枚举进程：{}", std::io::Error::last_os_error()));
            }
            pids.truncate(n as usize);
            Ok(pids)
        }
        fn read(&mut self, pid: i32) -> Result<Option<Process>, String> {
            let bsd = match info::<libc::proc_bsdinfo>(pid, libc::PROC_PIDTBSDINFO) {
                Ok(bsd) => bsd,
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if bsd.pbi_uid != unsafe { libc::getuid() } || bsd.pbi_status == libc::SZOMB {
                return Ok(None);
            }
            let mut bytes = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
            let n =
                unsafe { libc::proc_pidpath(pid, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
            let exe = if n > 0 {
                Ok(path(&bytes))
            } else {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    return Ok(None);
                }
                Err(error.to_string())
            };
            let cwd = match info::<libc::proc_vnodepathinfo>(pid, libc::PROC_PIDVNODEPATHINFO) {
                Ok(info) => {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            info.pvi_cdir.vip_path.as_ptr().cast::<u8>(),
                            1024,
                        )
                    };
                    if bytes[0] == 0 {
                        Err("cwd 为空".into())
                    } else {
                        Ok(path(bytes))
                    }
                }
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
                Err(e) => Err(e.to_string()),
            };
            let tty = if bsd.e_tdev == u32::MAX {
                None
            } else {
                let name = unsafe {
                    libc::devname(bsd.e_tdev as libc::dev_t, libc::S_IFCHR as libc::mode_t)
                };
                Some(if name.is_null() {
                    bsd.e_tdev.to_string()
                } else {
                    unsafe { CStr::from_ptr(name) }
                        .to_string_lossy()
                        .into_owned()
                })
            };
            Ok(Some(Process {
                pid,
                start: (bsd.pbi_start_tvsec, bsd.pbi_start_tvusec),
                uid: bsd.pbi_uid,
                zombie: false,
                tty,
                command: command(pid, &exe),
                exe,
                cwd,
            }))
        }
        fn signal(&mut self, pid: i32, signal: i32) -> Result<(), String> {
            if unsafe { libc::kill(pid, signal) } == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error().to_string())
            }
        }
        fn wait(&mut self) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
#[cfg(not(target_os = "macos"))]
impl ProcessTable for Native {
    fn pids(&mut self) -> Result<Vec<i32>, String> {
        Err("无法判断：本平台不支持".into())
    }
    fn read(&mut self, _: i32) -> Result<Option<Process>, String> {
        Ok(None)
    }
    fn signal(&mut self, _: i32, _: i32) -> Result<(), String> {
        Err("本平台不支持".into())
    }
    fn wait(&mut self) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        process: Option<Process>,
        signals: Vec<i32>,
        change: Option<fn(&mut Option<Process>)>,
        waits: usize,
        deny: bool,
    }
    impl ProcessTable for Fake {
        fn pids(&mut self) -> Result<Vec<i32>, String> {
            Ok(vec![42])
        }
        fn read(&mut self, _: i32) -> Result<Option<Process>, String> {
            Ok(self.process.clone())
        }
        fn signal(&mut self, _: i32, signal: i32) -> Result<(), String> {
            self.signals.push(signal);
            if self.deny {
                return Err("EPERM".into());
            }
            if signal == libc::SIGKILL {
                self.process = None;
            }
            Ok(())
        }
        fn wait(&mut self) {
            self.waits += 1;
            if let Some(change) = self.change.take() {
                change(&mut self.process);
            }
        }
    }
    fn fake() -> Fake {
        Fake {
            process: Some(Process {
                pid: 42,
                start: (5, 6),
                uid: 1,
                zombie: false,
                tty: None,
                exe: Ok("/bin/sleep".into()),
                cwd: Ok("/tmp/sweep".into()),
                command: "sleep 600".into(),
            }),
            signals: vec![],
            change: None,
            waits: 0,
            deny: false,
        }
    }
    fn sweep(f: &mut Fake) -> Report {
        run(f, &["/tmp/sweep".into()], 1, 99)
    }
    #[test]
    fn residue_components_not_prefixes() {
        let mut f = fake();
        f.process.as_mut().unwrap().cwd = Ok("/tmp/sweep-2".into());
        assert!(sweep(&mut f).processes.is_empty());
        assert!(f.signals.is_empty());
    }
    #[test]
    fn residue_canonical_tmp_alias() {
        let root = PathBuf::from("/tmp").join(format!("residue-alias-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let r = roots([root.clone()]);
        let mut f = fake();
        f.process.as_mut().unwrap().cwd = Ok(root.canonicalize().unwrap());
        assert_eq!(run(&mut f, &r, 1, 99).processes.len(), 1);
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn residue_executable_matches() {
        let mut f = fake();
        let p = f.process.as_mut().unwrap();
        p.exe = Ok("/tmp/sweep/bin/tool".into());
        p.cwd = Ok("/elsewhere".into());
        assert_eq!(sweep(&mut f).processes[0]["matched"], "exe");
    }
    #[test]
    fn residue_cwd_matches_even_if_executable_unknown() {
        let mut f = fake();
        f.process.as_mut().unwrap().exe = Err("ENOENT".into());
        let report = sweep(&mut f);
        assert_eq!(report.processes[0]["matched"], "cwd");
        assert!(report.unchecked.is_empty());
    }
    #[test]
    fn residue_esrch_zombie_and_self_skipped() {
        let mut f = fake();
        f.process = None;
        sweep(&mut f);
        assert!(f.signals.is_empty());
        let mut f = fake();
        f.process.as_mut().unwrap().zombie = true;
        sweep(&mut f);
        assert!(f.signals.is_empty());
        let mut f = fake();
        run(&mut f, &["/tmp/sweep".into()], 1, 42);
        assert!(f.signals.is_empty());
    }
    #[test]
    fn residue_other_uid_skipped_without_warning() {
        let mut f = fake();
        let p = f.process.as_mut().unwrap();
        p.uid = 2;
        p.exe = Err("EPERM".into());
        assert!(sweep(&mut f).unchecked.is_empty());
        assert!(f.signals.is_empty());
    }
    fn unknown(start: u64) {
        let mut f = fake();
        let p = f.process.as_mut().unwrap();
        p.start = (start, 0);
        p.exe = Err("ENOENT".into());
        p.cwd = Ok("/elsewhere".into());
        let report = sweep(&mut f);
        assert_eq!(report.unchecked[&42]["kind"], "unknown");
        assert!(
            report.unchecked[&42]["reason"]
                .as_str()
                .unwrap()
                .contains("ENOENT")
        );
        assert!(f.signals.is_empty());
    }
    #[test]
    fn residue_unknown_new_process_warns() {
        unknown(u64::MAX);
    }
    #[test]
    fn residue_unknown_old_process_also_warns() {
        unknown(0);
    }
    #[test]
    fn residue_term_then_kill() {
        let mut f = fake();
        let report = sweep(&mut f);
        assert_eq!(f.signals, [libc::SIGTERM, libc::SIGKILL]);
        assert_eq!(f.waits, 61);
        assert_eq!(report.processes[0]["signal"], "KILL");
        assert!(report.survivors.is_empty());
    }
    #[test]
    fn residue_pid_reused_no_kill_or_survivor() {
        let mut f = fake();
        f.change = Some(|p| p.as_mut().unwrap().start.1 += 1);
        assert!(sweep(&mut f).survivors.is_empty());
        assert_eq!(f.signals, [libc::SIGTERM]);
    }
    #[test]
    fn residue_cwd_leaves_before_kill() {
        let mut f = fake();
        f.change = Some(|p| p.as_mut().unwrap().cwd = Ok("/elsewhere".into()));
        let report = sweep(&mut f);
        assert!(report.survivors.is_empty());
        assert!(report.processes.is_empty());
        assert_eq!(f.signals, [libc::SIGTERM]);
    }
    #[test]
    fn residue_tty_never_signalled() {
        let mut f = fake();
        f.process.as_mut().unwrap().tty = Some("ttys002".into());
        let report = sweep(&mut f);
        assert!(f.signals.is_empty());
        assert!(report.processes.is_empty());
        assert_eq!(report.unchecked[&42]["kind"], "tty");
        assert_eq!(report.unchecked[&42]["tty"], "ttys002");
    }
    #[test]
    fn residue_tty_acquired_before_kill() {
        let mut f = fake();
        f.change = Some(|p| p.as_mut().unwrap().tty = Some("ttys003".into()));
        let report = sweep(&mut f);
        assert_eq!(f.signals, [libc::SIGTERM]);
        assert_eq!(report.unchecked[&42]["kind"], "tty");
        assert!(report.survivors.is_empty());
        assert!(report.processes.is_empty());
    }
    #[test]
    fn residue_permission_denied_survivor_and_command_boundary() {
        let mut f = fake();
        f.deny = true;
        f.process.as_mut().unwrap().command = "中".repeat(121);
        let report = sweep(&mut f);
        assert!(report.processes.is_empty());
        assert_eq!(report.survivors[0]["error"], "EPERM");
        assert_eq!(
            report.survivors[0]["command"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            120
        );
        assert_eq!(f.waits, 80);
    }
    #[test]
    fn residue_term_exits_and_unknown_recheck() {
        let mut f = fake();
        f.change = Some(|p| *p = None);
        assert_eq!(sweep(&mut f).processes[0]["signal"], "TERM");
        assert_eq!(f.signals, [libc::SIGTERM]);
        let mut f = fake();
        f.change = Some(|p| {
            let p = p.as_mut().unwrap();
            p.cwd = Err("EPERM".into());
        });
        assert_eq!(sweep(&mut f).unchecked[&42]["kind"], "unknown");
        assert_eq!(f.signals, [libc::SIGTERM]);
    }
    #[test]
    fn residue_empty_roots_never_match() {
        let mut f = fake();
        assert!(
            run(&mut f, &roots([PathBuf::new()]), 1, 99)
                .processes
                .is_empty()
        );
        assert!(f.signals.is_empty());
    }

    #[test]
    fn residue_empty_table_writes_no_events() {
        let store = crate::store::Store::open_in_memory().unwrap();
        let mut f = fake();
        f.process = None;
        record(&store, "T-1", vec![], sweep(&mut f));
        for kind in ["task.processes_stopped", "task.processes_unchecked"] {
            assert!(store.last_event("T-1", kind).unwrap().is_none());
        }
    }

    #[test]
    fn residue_query_failures_do_not_signal() {
        struct Denied {
            reads: usize,
            enumeration: bool,
            before: usize,
        }
        impl ProcessTable for Denied {
            fn pids(&mut self) -> Result<Vec<i32>, String> {
                if self.enumeration {
                    Err("EPERM".into())
                } else {
                    Ok(vec![42])
                }
            }
            fn read(&mut self, _: i32) -> Result<Option<Process>, String> {
                self.reads += 1;
                if self.reads > self.before {
                    Err("EPERM".into())
                } else {
                    Ok(fake().process)
                }
            }
            fn signal(&mut self, _: i32, _: i32) -> Result<(), String> {
                panic!("must not signal")
            }
            fn wait(&mut self) {
                panic!("nothing to wait for")
            }
        }
        for (enumeration, before, warnings) in [(true, 0, 1), (false, 0, 0), (false, 1, 1)] {
            let report = run(
                &mut Denied {
                    reads: 0,
                    enumeration,
                    before,
                },
                &["/tmp/sweep".into()],
                1,
                99,
            );
            assert_eq!(report.unchecked.len(), warnings);
            assert!(report.processes.is_empty());
            assert!(report.survivors.is_empty());
        }
    }
}
