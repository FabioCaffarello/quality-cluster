mod collect;

use crate::error::{CliError, Result};
use collect::{Collector, Evidence, EvidenceStatus};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Configuration for the trace-pack command.
pub struct TracePackConfig {
    pub project_root: PathBuf,
    pub base_url: String,
    pub output_dir: PathBuf,
    pub log_lines: u32,
    pub results_limit: u32,
    pub compress: bool,
}

/// Result of a trace-pack run.
#[derive(Debug, Serialize)]
pub struct TracePackReport {
    pub pack_path: String,
    pub collected: Vec<EvidenceEntry>,
    pub failed: Vec<EvidenceEntry>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct EvidenceEntry {
    pub name: String,
    pub file: String,
    pub status: String,
    pub detail: Option<String>,
}

/// Run the trace-pack collector and write evidence to disk.
pub fn run(config: &TracePackConfig) -> Result<TracePackReport> {
    let timestamp = now_timestamp();
    let pack_name = format!("trace-pack-{timestamp}");
    let pack_dir = config.output_dir.join(&pack_name);

    std::fs::create_dir_all(&pack_dir)?;

    let compose_file = config
        .project_root
        .join("deploy/compose/docker-compose.yaml");

    let collector = Collector::new(
        &compose_file,
        &config.base_url,
        &config.project_root,
        config.log_lines,
        config.results_limit,
    );

    let evidences = collector.collect_all();

    let mut collected = Vec::new();
    let mut failed = Vec::new();

    for ev in &evidences {
        let entry = EvidenceEntry {
            name: ev.name.clone(),
            file: ev.file.clone(),
            status: match &ev.status {
                EvidenceStatus::Ok => "ok".into(),
                EvidenceStatus::Unavailable(_) => "unavailable".into(),
                EvidenceStatus::Error(_) => "error".into(),
            },
            detail: match &ev.status {
                EvidenceStatus::Ok => None,
                EvidenceStatus::Unavailable(msg) | EvidenceStatus::Error(msg) => {
                    Some(msg.clone())
                }
            },
        };

        match &ev.status {
            EvidenceStatus::Ok => {
                write_evidence(&pack_dir, ev)?;
                collected.push(entry);
            }
            EvidenceStatus::Unavailable(_) => {
                failed.push(entry);
            }
            EvidenceStatus::Error(_) => {
                failed.push(entry);
            }
        }
    }

    let summary = build_summary(&timestamp, &collected, &failed);
    std::fs::write(pack_dir.join("SUMMARY.md"), &summary)?;

    let final_path = if config.compress {
        let tar_path = compress_pack(&pack_dir, &config.output_dir, &pack_name)?;
        std::fs::remove_dir_all(&pack_dir)?;
        tar_path
    } else {
        pack_dir.display().to_string()
    };

    Ok(TracePackReport {
        pack_path: final_path,
        collected,
        failed,
        summary,
    })
}

/// Render the report as human-readable text.
pub fn render_human(report: &TracePackReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("trace-pack: {}\n\n", report.pack_path));

    if !report.collected.is_empty() {
        out.push_str(&format!(
            "Collected ({}):\n",
            report.collected.len()
        ));
        for e in &report.collected {
            out.push_str(&format!("  + {:<30} {}\n", e.name, e.file));
        }
    }

    if !report.failed.is_empty() {
        out.push_str(&format!(
            "\nUnavailable ({}):\n",
            report.failed.len()
        ));
        for e in &report.failed {
            let detail = e.detail.as_deref().unwrap_or("");
            out.push_str(&format!("  - {:<30} {}\n", e.name, detail));
        }
    }

    out.push('\n');
    out.push_str(&format!(
        "{} collected, {} unavailable\n",
        report.collected.len(),
        report.failed.len()
    ));

    out
}

/// Render the report as JSON.
pub fn render_json(report: &TracePackReport) -> Result<String> {
    serde_json::to_string_pretty(report).map_err(CliError::Json)
}

// ── internals ──────────────────────────────────────────────────

fn now_timestamp() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Format as YYYYMMDD-HHmmss (UTC approximation from epoch)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Simple epoch-to-date calculation
    let (year, month, day) = epoch_days_to_date(days);
    format!(
        "{year:04}{month:02}{day:02}-{hours:02}{minutes:02}{seconds:02}"
    )
}

fn epoch_days_to_date(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn write_evidence(pack_dir: &Path, ev: &Evidence) -> Result<()> {
    let target = pack_dir.join(&ev.file);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, &ev.content)?;
    Ok(())
}

fn compress_pack(_pack_dir: &Path, output_dir: &Path, pack_name: &str) -> Result<String> {
    let tar_name = format!("{pack_name}.tar.gz");
    let tar_path = output_dir.join(&tar_name);

    let output = std::process::Command::new("tar")
        .args(["-czf"])
        .arg(&tar_path)
        .arg("-C")
        .arg(output_dir)
        .arg(pack_name)
        .output()
        .map_err(|e| CliError::Command {
            message: format!("failed to run tar: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Command {
            message: format!("tar failed: {stderr}"),
        });
    }

    Ok(tar_path.display().to_string())
}

fn build_summary(
    timestamp: &str,
    collected: &[EvidenceEntry],
    failed: &[EvidenceEntry],
) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Trace Pack — {timestamp}\n\n"));
    s.push_str("Diagnostic evidence snapshot for quality-service cluster.\n\n");

    s.push_str("## What is this?\n\n");
    s.push_str(
        "This pack contains runtime state, configuration, logs, and API responses\n\
         collected at a single point in time. Use it to diagnose failures without\n\
         needing live access to the cluster.\n\n",
    );

    s.push_str("## Collected evidence\n\n");
    s.push_str("| File | Description |\n");
    s.push_str("|------|-------------|\n");
    for e in collected {
        s.push_str(&format!("| `{}` | {} |\n", e.file, e.name));
    }

    if !failed.is_empty() {
        s.push_str("\n## Unavailable evidence\n\n");
        s.push_str("| Evidence | Reason |\n");
        s.push_str("|----------|--------|\n");
        for e in failed {
            let detail = e.detail.as_deref().unwrap_or("unknown");
            s.push_str(&format!("| {} | {} |\n", e.name, detail));
        }
        s.push_str(
            "\nUnavailable items indicate services that were down or unreachable at collection time.\n",
        );
    }

    s.push_str("\n## How to use\n\n");
    s.push_str("1. Check `compose-status.txt` for service health overview\n");
    s.push_str("2. Review `healthz.json` and `readyz.json` for API readiness\n");
    s.push_str("3. Inspect `active-config.json` for the running configuration\n");
    s.push_str("4. Check `ingestion-bindings.json` for active data routing\n");
    s.push_str("5. Review `validation-results.json` for recent pass/fail outcomes\n");
    s.push_str("6. Examine `logs/` for per-service console output\n");
    s.push_str("7. Compare `configs/` with active runtime to spot drift\n");

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_days_to_date_unix_epoch() {
        let (y, m, d) = epoch_days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn epoch_days_to_date_known_date() {
        // 2026-03-14 is day 20526 from epoch (0-indexed from 1970-01-01)
        let (y, m, d) = epoch_days_to_date(20526);
        assert_eq!((y, m, d), (2026, 3, 14));
    }

    #[test]
    fn build_summary_contains_collected() {
        let collected = vec![EvidenceEntry {
            name: "Compose status".into(),
            file: "compose-status.txt".into(),
            status: "ok".into(),
            detail: None,
        }];
        let failed = vec![];
        let summary = build_summary("20260314-120000", &collected, &failed);
        assert!(summary.contains("Compose status"));
        assert!(summary.contains("compose-status.txt"));
        assert!(!summary.contains("Unavailable evidence"));
    }

    #[test]
    fn build_summary_contains_failed() {
        let collected = vec![];
        let failed = vec![EvidenceEntry {
            name: "Health check".into(),
            file: "healthz.json".into(),
            status: "unavailable".into(),
            detail: Some("connection refused".into()),
        }];
        let summary = build_summary("20260314-120000", &collected, &failed);
        assert!(summary.contains("Unavailable evidence"));
        assert!(summary.contains("connection refused"));
    }

    #[test]
    fn render_human_format() {
        let report = TracePackReport {
            pack_path: "trace-pack-test".into(),
            collected: vec![EvidenceEntry {
                name: "Compose status".into(),
                file: "compose-status.txt".into(),
                status: "ok".into(),
                detail: None,
            }],
            failed: vec![EvidenceEntry {
                name: "Health check".into(),
                file: "healthz.json".into(),
                status: "unavailable".into(),
                detail: Some("connection refused".into()),
            }],
            summary: String::new(),
        };
        let rendered = render_human(&report);
        assert!(rendered.contains("trace-pack: trace-pack-test"));
        assert!(rendered.contains("Collected (1)"));
        assert!(rendered.contains("Unavailable (1)"));
        assert!(rendered.contains("1 collected, 1 unavailable"));
    }

    #[test]
    fn render_json_is_valid() {
        let report = TracePackReport {
            pack_path: "test".into(),
            collected: vec![],
            failed: vec![],
            summary: "test".into(),
        };
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["pack_path"], "test");
    }

    #[test]
    fn write_evidence_creates_nested_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let ev = Evidence {
            name: "test".into(),
            file: "nested/dir/file.txt".into(),
            content: "hello".into(),
            status: EvidenceStatus::Ok,
        };
        write_evidence(tmp.path(), &ev).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("nested/dir/file.txt")).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn run_produces_pack_with_summary() {
        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_output = tempfile::tempdir().unwrap();

        // Create minimal project structure
        let deploy_dir = tmp_project.path().join("deploy/compose");
        std::fs::create_dir_all(&deploy_dir).unwrap();
        std::fs::write(deploy_dir.join("docker-compose.yaml"), "version: '3'").unwrap();

        let configs_dir = tmp_project.path().join("deploy/configs");
        std::fs::create_dir_all(&configs_dir).unwrap();
        std::fs::write(configs_dir.join("server.jsonc"), r#"{"log_level":"info"}"#).unwrap();

        let config = TracePackConfig {
            project_root: tmp_project.path().to_path_buf(),
            base_url: "http://127.0.0.1:19999".into(), // unreachable
            output_dir: tmp_output.path().to_path_buf(),
            log_lines: 50,
            results_limit: 10,
            compress: false,
        };

        let report = run(&config).unwrap();

        // Should have collected config files at minimum
        assert!(!report.pack_path.is_empty());
        // SUMMARY.md should exist
        let pack_path = Path::new(&report.pack_path);
        assert!(pack_path.join("SUMMARY.md").exists());
        // Should have some failures (API unreachable)
        assert!(!report.failed.is_empty());
    }

    #[test]
    fn run_with_compress_produces_tarball() {
        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_output = tempfile::tempdir().unwrap();

        let deploy_dir = tmp_project.path().join("deploy/compose");
        std::fs::create_dir_all(&deploy_dir).unwrap();
        std::fs::write(deploy_dir.join("docker-compose.yaml"), "version: '3'").unwrap();

        let configs_dir = tmp_project.path().join("deploy/configs");
        std::fs::create_dir_all(&configs_dir).unwrap();
        std::fs::write(configs_dir.join("consumer.jsonc"), "{}").unwrap();

        let config = TracePackConfig {
            project_root: tmp_project.path().to_path_buf(),
            base_url: "http://127.0.0.1:19999".into(),
            output_dir: tmp_output.path().to_path_buf(),
            log_lines: 50,
            results_limit: 10,
            compress: true,
        };

        let report = run(&config).unwrap();
        assert!(report.pack_path.ends_with(".tar.gz"));
        assert!(Path::new(&report.pack_path).exists());
    }
}
