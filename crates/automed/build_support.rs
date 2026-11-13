// Shared by the build script and its tests; no third-party dependencies.
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn utc_timestamp(seconds: i64) -> String {
    // Civil date conversion from days since the Unix epoch (Gregorian calendar).
    let days = seconds.div_euclid(86400);
    let time = seconds.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        time / 3600,
        time / 60 % 60,
        time % 60
    )
}

pub fn watch_paths(
    git_dir: &Path,
    common_dir: &Path,
    head: &str,
    exists: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut paths = vec![git_dir.join("HEAD")];
    let log = git_dir.join("logs/HEAD");
    let has_log = exists(&log);
    if has_log {
        paths.push(log);
    }
    let packed = common_dir.join("packed-refs");
    if exists(&packed) {
        paths.push(packed);
    }
    if let Some(reference) = head.trim().strip_prefix("ref: ") {
        let loose = common_dir.join(reference);
        if exists(&loose) || !has_log {
            paths.push(loose);
        }
    }
    paths
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn compute(dir: &Path) -> Option<String> {
    if git(dir, &["rev-parse", "--is-shallow-repository"])? != "false" {
        return None;
    }
    let count: u64 = git(dir, &["rev-list", "--count", "HEAD"])?.parse().ok()?;
    if count > 9_999_999 {
        return None;
    }
    let commit = git(dir, &["show", "-s", "--format=%H%x20%ct", "HEAD"])?;
    let (sha, seconds) = commit.split_once(' ')?;
    if sha.len() < 12 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!(
        "{count:07}-{}-{}",
        utc_timestamp(seconds.parse().ok()?),
        &sha[..12]
    ))
}

pub fn repository_watch_paths(dir: &Path) -> Vec<PathBuf> {
    let Some(git_dir) = git(dir, &["rev-parse", "--git-dir"]) else {
        return vec![];
    };
    let Some(common) = git(dir, &["rev-parse", "--git-common-dir"]) else {
        return vec![];
    };
    let git_dir = dir.join(git_dir);
    let common = dir.join(common);
    let head = std::fs::read_to_string(git_dir.join("HEAD")).unwrap_or_default();
    let mut paths = watch_paths(&git_dir, &common, &head, Path::exists);
    // Fetch/unshallow changes the ancestry count without changing HEAD.
    if common.join("shallow").exists() {
        paths.push(common.join("shallow"));
    }
    paths
}
