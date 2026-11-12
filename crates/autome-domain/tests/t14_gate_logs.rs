//! Optional read-only verification against the original host logs.
use autome_domain::gate_failure::fingerprint;
use std::{fs, path::PathBuf};

#[test]
#[ignore = "requires AUTOME_T14_GATE_DIR pointing to the original T-14 logs"]
fn t14_gate_logs() {
    let dir = PathBuf::from(std::env::var_os("AUTOME_T14_GATE_DIR").expect("AUTOME_T14_GATE_DIR"));
    let mut runs = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "started")
            && path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gate-")
        {
            let exit: i32 = fs::read_to_string(path.with_extension("exit"))
                .unwrap()
                .lines()
                .find_map(|line| line.strip_prefix("exit_code="))
                .expect("exit_code marker")
                .parse()
                .unwrap();
            if exit != 0 {
                let started: u64 = fs::read_to_string(&path).unwrap().trim().parse().unwrap();
                runs.push((started, path, exit));
            }
        }
    }
    runs.sort();
    assert_eq!(runs.len(), 21);
    let fingerprints: Vec<_> = runs
        .iter()
        .map(|(_, p, exit)| {
            fingerprint(
                &fs::read_to_string(p.with_extension("log")).unwrap(),
                *exit,
                *exit == 124,
            )
        })
        .collect();
    for fp in &fingerprints[..18] {
        assert_eq!(fp.id, fingerprints[0].id);
        assert!(fp.summary.contains("Cannot find package 'ws'"));
    }
    assert_ne!(fingerprints[18].id, fingerprints[17].id);
    assert_eq!(fingerprints[19].id, fingerprints[20].id);
}
