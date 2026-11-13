use super::*;
use build_support::*;
use std::path::Path;
const A: &str = "0000001-20261113T000000Z-aaaaaaaaaaaa";
const B: &str = "0000002-20261112T000000Z-bbbbbbbbbbbb";
const U: &str = "unknown-0.1.0-20261113T000000Z";

#[test]
fn parse_git() {
    assert!(matches!(
        A.parse::<BuildId>(),
        Ok(BuildId::Git { count: 1, .. })
    ));
}
#[test]
fn parse_unknown() {
    assert!(matches!(U.parse::<BuildId>(), Ok(BuildId::Unknown(_))));
}
#[test]
fn malformed() {
    for s in [
        "",
        "1-20261113T000000Z-aaaaaaaaaaaa",
        "unknown--20261113T000000Z",
        "unknown-é-20261113T000000Z",
        "unknown-x\ny-20261113T000000Z",
        "0000001-20261113T000000Z-AAAAAAAAAAAA",
        "10000000-20261113T000000Z-aaaaaaaaaaaa",
    ] {
        assert!(s.parse::<BuildId>().is_err(), "{s}");
    }
    for s in [
        "0000000-20261113T000000Z-000000000000",
        "9999999-20261113T000000Z-ffffffffffff",
    ] {
        assert!(s.parse::<BuildId>().is_ok());
    }
}
#[test]
fn roundtrip() {
    for s in [A, U] {
        assert_eq!(s.parse::<BuildId>().unwrap().to_string(), s);
        assert_eq!(verdict(s, Some(s)), Verdict::Same);
    }
}
#[test]
fn count_beats_time() {
    assert_eq!(verdict(B, Some(A)), Verdict::Advance);
    assert_eq!(verdict(A, Some(B)), Verdict::Older);
}
#[test]
fn equal_count_different_sha() {
    let other = A.replace('a', "b");
    assert_eq!(verdict(A, Some(&other)), Verdict::Incomparable);
    assert_eq!(verdict(&other, Some(A)), Verdict::Incomparable);
}
#[test]
fn unknown_is_older() {
    assert_eq!(verdict(U, Some(A)), Verdict::Older);
    assert_eq!(verdict(A, Some(U)), Verdict::Advance);
}
#[test]
fn different_unknowns() {
    assert_eq!(
        verdict(U, Some(&U.replace("0.1.0", "0.2.0"))),
        Verdict::Older
    );
}
#[test]
fn corrupt_record() {
    assert_eq!(verdict(A, Some("")), Verdict::Older);
    assert_eq!(verdict("bad", Some("bad")), Verdict::Older);
}
#[test]
fn missing_record() {
    assert_eq!(verdict(A, None), Verdict::Advance);
    assert_eq!(verdict(U, None), Verdict::Advance);
}
#[test]
fn worktree_paths() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(compute(manifest).as_deref(), Some(BUILD_ID));
    assert!(!repository_watch_paths(manifest).is_empty());
    let g = Path::new("/repo/.git/worktrees/x");
    let c = Path::new("/repo/.git");
    let paths = watch_paths(g, c, "ref: refs/heads/x", |_| true);
    for p in [
        g.join("HEAD"),
        g.join("logs/HEAD"),
        c.join("packed-refs"),
        c.join("refs/heads/x"),
    ] {
        assert!(paths.contains(&p));
    }
    let paths = watch_paths(g, c, "ref: refs/heads/x", |_| false);
    assert_eq!(paths, vec![g.join("HEAD"), c.join("refs/heads/x")]);
    let paths = watch_paths(g, c, "ref: refs/heads/x", |p| p.ends_with("logs/HEAD"));
    assert_eq!(paths, vec![g.join("HEAD"), g.join("logs/HEAD")]);
}
#[test]
fn detached_paths() {
    let g = Path::new("/git");
    assert_eq!(watch_paths(g, g, "abc123", |_| false), vec![g.join("HEAD")]);
}
#[test]
fn utc_boundaries() {
    for (s, expected) in [
        (0, "19700101T000000Z"),
        (-1, "19691231T235959Z"),
        (1709164800, "20240229T000000Z"),
        (1735689599, "20241231T235959Z"),
        (1735689600, "20250101T000000Z"),
    ] {
        assert_eq!(utc_timestamp(s), expected);
    }
}
#[test]
fn override_only_older() {
    assert_eq!(effective_override(B, A), Some(A));
    assert_eq!(effective_override(A, B), None);
    assert_eq!(effective_override(A, ""), None);
    assert_eq!(effective_override(A, A), Some(A));
    assert_eq!(effective_override(A, U), Some(U));
}
#[test]
fn rejection_message() {
    assert_eq!(
        older_kernel_message("C", "M"),
        "这个内核（C）比推进过账本的内核（M）旧，已停止推进；请重新安装或设置 AUTOMED_BIN"
    );
}
