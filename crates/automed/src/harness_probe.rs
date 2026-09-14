//! Narrow, mechanically-defined slice of §5.10's `HarnessCapabilitySnapshot`.
//!
//! The plan gives `HarnessCapabilitySnapshot` ~15 fields (`protocol/schema
//! hash`, `auth_mode/state`, `lifecycle/tool/sandbox/resume/usage
//! capabilities`, `models[]` with per-model efforts, `valid_until`, ...),
//! but states no derivation rule for almost all of them: there is no
//! documented protocol/schema hashing scheme, no documented auth probe
//! (`claude`/`codex` each have different, undocumented-here auth surfaces),
//! and no documented way to enumerate `models[]` without provider-specific
//! knowledge the plan explicitly says doesn't exist yet for Claude CLI
//! (§5.10: "Claude CLI 当前没有可依赖的等价稳定 model-list 合同"). Modeling
//! those now would be guessing, not modeling — the same reasoning
//! `step_role.rs` already applied to §10.1's successor routing.
//!
//! What *does* have a mechanical, unambiguous derivation rule: resolve a
//! binary path, hash its bytes, and read its `--version` output. This module
//! covers exactly that — `canonical_binary`/`digest`/`version` — and nothing
//! else. `HarnessBinaryProbe` is deliberately not named or shaped as
//! `HarnessCapabilitySnapshot`; it is a strict subset that a future,
//! fully-specified snapshot builder can embed once the remaining fields have
//! an actual rule to implement.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessBinaryProbe {
    pub canonical_binary: PathBuf,
    pub digest_sha256_hex: String,
    pub raw_version_output: String,
}

#[derive(Debug, Error)]
pub enum HarnessProbeError {
    #[error("binary not found at {0}")]
    BinaryNotFound(PathBuf),
    #[error("failed to read binary at {path}: {source}")]
    ReadBinary {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to spawn {path}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("probe of {path} did not exit within {timeout_ms}ms and was killed")]
    Timeout { path: PathBuf, timeout_ms: u64 },
    #[error("{path} --version exited with {status}; stderr={stderr:?}")]
    NonZeroExit {
        path: PathBuf,
        status: String,
        stderr: String,
    },
    #[error("{path} --version produced non-UTF-8 output")]
    NonUtf8Output { path: PathBuf },
}

/// Resolves `binary_path`, hashes it, and runs `<binary_path> --version`
/// under `timeout`. On timeout the child is killed and reaped before
/// returning — a hung probe must never leak a process, the same discipline
/// `apps/desktop`'s `AutomedSidecar.stop()` already applies to the Core
/// sidecar.
pub fn probe_harness_binary(
    binary_path: &Path,
    timeout: Duration,
) -> Result<HarnessBinaryProbe, HarnessProbeError> {
    let canonical_binary = binary_path
        .canonicalize()
        .map_err(|_| HarnessProbeError::BinaryNotFound(binary_path.to_path_buf()))?;

    let bytes =
        std::fs::read(&canonical_binary).map_err(|source| HarnessProbeError::ReadBinary {
            path: canonical_binary.clone(),
            source,
        })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest_sha256_hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread tokio runtime for a single subprocess probe should always build");
    let raw_version_output = runtime.block_on(run_version_probe(&canonical_binary, timeout))?;

    Ok(HarnessBinaryProbe {
        canonical_binary,
        digest_sha256_hex,
        raw_version_output,
    })
}

async fn run_version_probe(
    canonical_binary: &Path,
    timeout: Duration,
) -> Result<String, HarnessProbeError> {
    let mut child = tokio::process::Command::new(canonical_binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| HarnessProbeError::Spawn {
            path: canonical_binary.to_path_buf(),
            source,
        })?;

    let mut stdout = child.stdout.take().expect("stdout was piped at spawn");
    let mut stderr = child.stderr.take().expect("stderr was piped at spawn");

    let collect = async {
        let mut out_buf = Vec::new();
        let mut err_buf = Vec::new();
        let (out_res, err_res, status_res) = tokio::join!(
            stdout.read_to_end(&mut out_buf),
            stderr.read_to_end(&mut err_buf),
            child.wait(),
        );
        out_res.map_err(|source| HarnessProbeError::Spawn {
            path: canonical_binary.to_path_buf(),
            source,
        })?;
        err_res.map_err(|source| HarnessProbeError::Spawn {
            path: canonical_binary.to_path_buf(),
            source,
        })?;
        let status = status_res.map_err(|source| HarnessProbeError::Spawn {
            path: canonical_binary.to_path_buf(),
            source,
        })?;
        Ok::<_, HarnessProbeError>((status, out_buf, err_buf))
    };

    match tokio::time::timeout(timeout, collect).await {
        Ok(result) => {
            let (status, out_buf, err_buf) = result?;
            if !status.success() {
                return Err(HarnessProbeError::NonZeroExit {
                    path: canonical_binary.to_path_buf(),
                    status: status.to_string(),
                    stderr: String::from_utf8_lossy(&err_buf).trim().to_string(),
                });
            }
            String::from_utf8(out_buf)
                .map(|text| text.trim().to_string())
                .map_err(|_| HarnessProbeError::NonUtf8Output {
                    path: canonical_binary.to_path_buf(),
                })
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(HarnessProbeError::Timeout {
                path: canonical_binary.to_path_buf(),
                timeout_ms: timeout.as_millis() as u64,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn probes_a_well_behaved_binary_and_reports_its_version_and_digest() {
        let probe =
            probe_harness_binary(&fixture("fake_harness_ok.sh"), Duration::from_secs(2)).unwrap();

        assert_eq!(probe.raw_version_output, "9.9.9 (Fake Harness)");
        // Independent oracle: `shasum -a 256 fake_harness_ok.sh`, not this
        // module's own hashing code — recomputing with Sha256 here would
        // only prove determinism, not correctness of the hex encoding.
        assert_eq!(
            probe.digest_sha256_hex,
            "fba7a6d8f3f0cb19a610bffbed3c7899d3012478866ce45117ceecb9c80ea1e8"
        );
    }

    #[test]
    fn missing_binary_is_reported_and_nothing_is_spawned() {
        let err = probe_harness_binary(&fixture("does_not_exist.sh"), Duration::from_secs(2))
            .unwrap_err();
        assert!(matches!(err, HarnessProbeError::BinaryNotFound(_)));
    }

    #[test]
    fn nonzero_exit_surfaces_stderr() {
        let err = probe_harness_binary(&fixture("fake_harness_fails.sh"), Duration::from_secs(2))
            .unwrap_err();
        match err {
            HarnessProbeError::NonZeroExit { stderr, .. } => {
                assert_eq!(stderr, "boom");
            }
            other => panic!("expected NonZeroExit, got {other:?}"),
        }
    }

    #[test]
    fn a_hung_binary_times_out_and_is_killed_not_leaked() {
        let started = std::time::Instant::now();
        let err = probe_harness_binary(
            &fixture("fake_harness_hangs.sh"),
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert!(matches!(err, HarnessProbeError::Timeout { .. }));
        // Must return close to the timeout, not wait out the fixture's own
        // (much longer) sleep — proves the child was actually killed rather
        // than the probe just giving up on reading while the process lives.
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
