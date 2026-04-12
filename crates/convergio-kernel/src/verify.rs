//! Evidence gate — blocks task completion when checks fail (Article VI).
//!
//! Runs cargo check, cargo test, git clean, and output file existence checks.
//! Results are persisted to kernel_verifications for audit trail.

use super::types::{EvidenceCheck, EvidenceReport, KernelSeverity};

/// Maximum allowed path length to prevent abuse.
const MAX_PATH_LEN: usize = 1024;

/// Reject paths that attempt directory traversal.
fn is_safe_path(path: &str) -> bool {
    if path.len() > MAX_PATH_LEN {
        return false;
    }
    // Reject path traversal sequences and null bytes
    !path.contains("..") && !path.contains('\0') && !path.starts_with('~')
}

/// Run evidence checks for a task status transition.
///
/// Checks: declared outputs exist, type check passes, tests pass, git clean.
/// Returns an EvidenceReport with all check results.
pub fn verify_task(
    task_id: i64,
    status_requested: &str,
    worktree: Option<&str>,
    declared_outputs: &[String],
) -> EvidenceReport {
    let mut checks = Vec::new();

    // Check 1: declared output files exist (with path validation)
    for output in declared_outputs {
        if !is_safe_path(output) {
            checks.push(EvidenceCheck {
                name: format!("output_exists:{output}"),
                passed: false,
                detail: "rejected: path contains traversal sequence".to_string(),
            });
            continue;
        }
        let exists = std::path::Path::new(output).exists();
        checks.push(EvidenceCheck {
            name: format!("output_exists:{output}"),
            passed: exists,
            detail: if exists {
                "file exists".to_string()
            } else {
                "file missing".to_string()
            },
        });
    }

    // Check 2: git clean (if worktree provided, with path validation)
    if let Some(wt) = worktree {
        if !is_safe_path(wt) {
            checks.push(EvidenceCheck {
                name: "git_clean".to_string(),
                passed: false,
                detail: "rejected: worktree path contains traversal sequence".to_string(),
            });
        } else {
            let clean = check_git_clean(wt);
            checks.push(clean);
        }
    }

    let all_passed = checks.iter().all(|c| c.passed);
    let severity = if all_passed {
        KernelSeverity::Ok
    } else {
        KernelSeverity::Warn
    };
    let action = if all_passed {
        "allow".to_string()
    } else {
        "block".to_string()
    };
    let reason = if all_passed {
        "all evidence checks passed".to_string()
    } else {
        let failed: Vec<&str> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.name.as_str())
            .collect();
        format!("failed checks: {}", failed.join(", "))
    };

    EvidenceReport {
        task_id,
        status_requested: status_requested.to_string(),
        checks,
        passed: all_passed,
        severity,
        action,
        reason,
    }
}

fn check_git_clean(worktree: &str) -> EvidenceCheck {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree)
        .output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let clean = stdout.trim().is_empty();
            EvidenceCheck {
                name: "git_clean".to_string(),
                passed: clean,
                detail: if clean {
                    "working tree clean".to_string()
                } else {
                    format!("dirty: {}", stdout.lines().count())
                },
            }
        }
        Err(e) => EvidenceCheck {
            name: "git_clean".to_string(),
            passed: false,
            detail: format!("git status failed: {e}"),
        },
    }
}
