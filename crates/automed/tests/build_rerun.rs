#[path = "../build_support.rs"]
mod build_support;
use automed::build_id::{Verdict, verdict};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(dir: &Path, program: &str, args: &[&str]) -> String {
    let out = Command::new(program)
        .current_dir(dir)
        .args(args)
        .env("TZ", "UTC")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn repo() -> Repo {
    let dir = std::env::temp_dir().join(format!("autome-build-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(dir.join("src")).unwrap();
    run(&dir, "git", &["init", "-b", "main"]);
    run(
        &dir,
        "git",
        &["config", "user.email", "test@example.invalid"],
    );
    run(&dir, "git", &["config", "user.name", "Build Test"]);
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname='build-probe'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/main.rs"),
        "fn main() { println!(\"{}\", env!(\"AUTOME_BUILD_ID\")); }",
    )
    .unwrap();
    fs::write(dir.join(".gitignore"), "target/\n").unwrap();
    for file in ["build.rs", "build_support.rs"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(file),
            dir.join(file),
        )
        .unwrap();
    }
    commit(&dir, "2026-11-13T00:00:00Z");
    Repo(dir)
}
fn commit(dir: &Path, date: &str) {
    run(dir, "git", &["add", "."]);
    let out = Command::new("git")
        .current_dir(dir)
        .args(["commit", "--allow-empty", "-m", "test"])
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_AUTHOR_DATE", date)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn build(dir: &Path) -> String {
    run(
        dir,
        "cargo",
        &["run", "--offline", "--quiet", "--target-dir", "target"],
    )
}
fn expected(dir: &Path) -> String {
    let count: u64 = run(dir, "git", &["rev-list", "--count", "HEAD"])
        .parse()
        .unwrap();
    let sha = run(dir, "git", &["rev-parse", "--short=12", "HEAD"]);
    let time = run(
        dir,
        "git",
        &[
            "show",
            "-s",
            "--date=format-local:%Y%m%dT%H%M%SZ",
            "--format=%cd",
            "HEAD",
        ],
    );
    // These tests execute with TZ=UTC, independently of the production formatter.
    format!("{count:07}-{time}-{sha}")
}
#[test]
fn build_rerun_follows_head_in_a_worktree() {
    let r = repo();
    let work = r.0.join("linked");
    run(
        &r.0,
        "git",
        &["worktree", "add", "-b", "linked", work.to_str().unwrap()],
    );
    let a = build(&work);
    assert_eq!(a, expected(&work));
    commit(&work, "2026-11-14T00:00:00Z");
    let b = build(&work);
    assert_eq!(b, expected(&work));
    assert_ne!(a, b);
    assert_eq!(build_support::compute(&work), Some(b));
    assert!(!build_support::repository_watch_paths(&work).is_empty());
}
#[test]
fn build_rerun_follows_a_packed_ref_without_reflog() {
    let r = repo();
    run(&r.0, "git", &["config", "core.logAllRefUpdates", "false"]);
    fs::remove_dir_all(r.0.join(".git/logs")).unwrap();
    run(&r.0, "git", &["pack-refs", "--all"]);
    let a = build(&r.0);
    assert_eq!(a, expected(&r.0));
    commit(&r.0, "2026-11-14T00:00:00Z");
    let b = build(&r.0);
    assert_eq!(b, expected(&r.0));
    assert_ne!(a, b);
}
#[test]
fn build_id_orders_same_second_parent_and_child() {
    let r = repo();
    let a = build(&r.0);
    commit(&r.0, "2026-11-13T00:00:00Z");
    let b = build(&r.0);
    assert_eq!(b, expected(&r.0));
    assert_eq!(verdict(&a, Some(&b)), Verdict::Older);
    commit(&r.0, "2026-11-12T00:00:00Z");
    let c = build(&r.0);
    assert_eq!(c, expected(&r.0));
    assert_eq!(verdict(&b, Some(&c)), Verdict::Older);
}

#[test]
fn build_fallback_is_unknown_without_usable_git() {
    let r = repo();
    let probe = r.0.join("build-script");
    run(
        &r.0,
        "rustc",
        &["--edition=2024", "build.rs", "-o", probe.to_str().unwrap()],
    );
    let check = |dir: &Path, path: Option<&str>| {
        let mut cmd = Command::new(&probe);
        cmd.env("CARGO_MANIFEST_DIR", dir)
            .env("CARGO_PKG_VERSION", "0.1.0");
        if let Some(path) = path {
            cmd.env("PATH", path);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(stdout.contains("cargo:warning=构建标识：拿不到 git 提交，标为未知来源"));
        let id = stdout
            .lines()
            .find_map(|s| s.strip_prefix("cargo:rustc-env=AUTOME_BUILD_ID="))
            .unwrap();
        assert!(matches!(
            id.parse::<automed::build_id::BuildId>(),
            Ok(automed::build_id::BuildId::Unknown(_))
        ));
    };
    check(&r.0, Some("/nonexistent-autome-git")); // No git executable.
    let empty = std::env::temp_dir().join(format!("autome-source-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&empty).unwrap();
    let empty = Repo(empty);
    check(&empty.0, None); // Source archive without a repository.
    let shallow = r.0.join("shallow");
    run(
        &r.0,
        "git",
        &[
            "clone",
            "--depth",
            "1",
            &format!("file://{}", r.0.display()),
            shallow.to_str().unwrap(),
        ],
    );
    check(&shallow, None);
    let fake = r.0.join("fake-bin");
    fs::create_dir(&fake).unwrap();
    let script = fake.join("git");
    fs::write(&script, "#!/bin/sh\ncase \"$*\" in\n*--is-shallow-repository*) echo false;;\n*--count*) echo 10000000;;\n*) exit 1;;\nesac\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    check(&r.0, Some(fake.to_str().unwrap())); // Count exceeds the seven-digit domain.
    fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    check(&r.0, Some(fake.to_str().unwrap())); // Git command failure.
}
