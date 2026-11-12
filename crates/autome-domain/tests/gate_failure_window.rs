use autome_domain::gate_failure::{PreviousResult, fingerprint, streak};

const STEP: &str = "Test (unit, end to end, adapter flags)";
const NOISE_ERROR: &str = "Error: 内核在回复前退出 (code=null, signal=null)";

// A failed early Rust step followed by successful steps, as in ci-local.sh.
// All inputs fit within the reader's 256 KiB budget; only line truncation loses them.
fn gate_log(case: &str, later_lines: usize) -> String {
    let mut log = format!(
        "==> {STEP}\nthread '{case}' panicked at crates/autome-cli/tests/cli.rs:1016:5:\nassertion failed: {case}\ntest {case} ... FAILED\nFAILED (1s)\n==> Desktop\n"
    );
    for _ in 0..later_lines {
        log.push_str("test desktop_render ... ok\n");
    }
    log.push_str(&format!(
        "==> Web test\n{NOISE_ERROR}\n ✓ expected kernel exit (1 test)\nPASSED (1s)\nFAILED  {STEP}\n"
    ));
    assert!(log.len() < 256 * 1024);
    log
}

#[test]
fn gate_failure_window_distinguishes_early_failures() {
    let a = fingerprint(&gate_log("protocol_version_assertion", 420), 1, false);
    let b = fingerprint(&gate_log("protocol_eval_budget", 420), 1, false);
    assert_ne!(
        a.id, b.id,
        "different early failures collapsed: {a:?} / {b:?}"
    );
}

#[test]
fn gate_failure_window_summary_belongs_to_failed_step() {
    // The short control also catches cross-step attribution without truncation.
    for later_lines in [0, 420] {
        let fp = fingerprint(
            &gate_log("protocol_version_assertion", later_lines),
            1,
            false,
        );
        assert!(
            fp.summary.contains(STEP)
                && (fp.summary.contains("protocol_version_assertion")
                    || fp
                        .summary
                        .contains("panicked at crates/autome-cli/tests/cli.rs"))
                && !fp.summary.contains(NOISE_ERROR),
            "later_lines={later_lines}: {fp:?}"
        );
    }
}

#[test]
fn gate_failure_window_different_failures_do_not_trip() {
    let mut prev = None;
    let mut observed = Vec::new();
    for case in [
        "protocol_version_assertion",
        "protocol_eval_budget",
        "json_tick_interrupt",
    ] {
        let fp = fingerprint(&gate_log(case, 420), 1, false);
        let decision = streak(prev.as_ref(), &fp.id, 3);
        prev = Some(PreviousResult {
            ok: false,
            fingerprint: Some(fp.id),
            same_failures: decision.count,
            tripped: decision.tripped,
        });
        observed.push((decision.count, decision.tripped));
    }
    assert_eq!(observed, vec![(1, false), (1, false), (1, false)]);
}

#[test]
fn gate_failure_window_same_failure_still_trips() {
    let mut prev = None;
    for count in 1..=3 {
        let fp = fingerprint(&gate_log("protocol_version_assertion", 420), 1, false);
        let decision = streak(prev.as_ref(), &fp.id, 3);
        assert_eq!((decision.count, decision.tripped), (count, count == 3));
        prev = Some(PreviousResult {
            ok: false,
            fingerprint: Some(fp.id),
            same_failures: decision.count,
            tripped: decision.tripped,
        });
    }
}

#[test]
fn gate_failure_window_short_logs_distinguish_failures() {
    assert_ne!(
        fingerprint(&gate_log("protocol_version_assertion", 0), 1, false).id,
        fingerprint(&gate_log("protocol_eval_budget", 0), 1, false).id
    );
}
