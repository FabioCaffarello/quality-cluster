mod collect;

use crate::error::{CliError, Result};
use crate::runtime_diagnostics::{
    compact_bootstrap_signature, parse_consumer_bootstrap_diagnostics,
    parse_emulator_bootstrap_diagnostics,
};
use collect::{Collector, Evidence, EvidenceStatus};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
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
                EvidenceStatus::Unavailable(msg) | EvidenceStatus::Error(msg) => Some(msg.clone()),
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

    let summary = build_summary(&timestamp, &evidences, &collected, &failed);
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
        out.push_str(&format!("Collected ({}):\n", report.collected.len()));
        for e in &report.collected {
            out.push_str(&format!("  + {:<30} {}\n", e.name, e.file));
        }
    }

    if !report.failed.is_empty() {
        out.push_str(&format!("\nUnavailable ({}):\n", report.failed.len()));
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
    format!("{year:04}{month:02}{day:02}-{hours:02}{minutes:02}{seconds:02}")
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
    evidences: &[Evidence],
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

    if let Some(refresh_summary) = build_refresh_observability_summary(evidences) {
        s.push_str("\n## Refresh observability\n\n");
        s.push_str(&refresh_summary);
        s.push('\n');
    }

    if let Some(loaded_bootstrap_summary) = build_loaded_bootstrap_summary(evidences) {
        s.push_str("\n## Loaded bootstrap diagnostics\n\n");
        s.push_str(&loaded_bootstrap_summary);
        s.push('\n');
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
    s.push_str("3. Inspect `nats/healthz.json` and `nats/jsz.json` for JetStream and refresh-consumer state\n");
    s.push_str("4. Inspect `active-config.json` for the lifecycle-facing active config record\n");
    s.push_str("5. Inspect `configctl-runtime-projections.json` for the canonical configctl runtime projection\n");
    s.push_str("6. Check `ingestion-bindings.json` and `validator-runtime.json` for operational routing and loaded-state views\n");
    s.push_str("7. Review `validation-results.json` for recent pass/fail outcomes\n");
    s.push_str("8. Examine `logs/` for per-service console output\n");
    s.push_str("9. Compare `configs/` with active runtime to spot drift\n");

    s
}

fn build_refresh_observability_summary(evidences: &[Evidence]) -> Option<String> {
    let by_file: HashMap<&str, &Evidence> =
        evidences.iter().map(|e| (e.file.as_str(), e)).collect();
    let consumer_interval =
        extract_reconcile_interval(by_file.get("configs/consumer.jsonc")?.content.as_str());
    let emulator_interval =
        extract_reconcile_interval(by_file.get("configs/emulator.jsonc")?.content.as_str());
    let refresh_status = classify_refresh_status(
        consumer_interval.as_deref(),
        emulator_interval.as_deref(),
        by_file
            .get("nats/jsz.json")
            .and_then(|e| parse_refresh_durables(e.content.as_str())),
    );

    let mut out = String::new();
    out.push_str(&format!("- refresh status: `{}`\n", refresh_status.label()));
    out.push_str(&format!(
        "- refresh mode: `{}`\n",
        refresh_status.mode_label()
    ));
    out.push_str(&format!(
        "- `bootstrap.reconcile_interval`: consumer=`{}`, emulator=`{}`\n",
        consumer_interval.as_deref().unwrap_or("missing"),
        emulator_interval.as_deref().unwrap_or("missing")
    ));

    match refresh_status.durables.as_ref() {
        Some(durables) if !durables.is_empty() => {
            out.push_str("- refresh durables on `CONFIGCTL_EVENTS`:\n");
            for durable in durables {
                out.push_str(&format!(
                    "  - `{}` pending=`{}` ack_pending=`{}` redelivered=`{}` delivered=`{}` ack_floor=`{}`\n",
                    durable.name,
                    durable.num_pending,
                    durable.num_ack_pending,
                    durable.num_redelivered,
                    durable.delivered_consumer_seq,
                    durable.ack_floor_consumer_seq
                ));
            }
        }
        Some(_) => out.push_str("- refresh durables on `CONFIGCTL_EVENTS`: not found\n"),
        None => out.push_str("- refresh durables on `CONFIGCTL_EVENTS`: unavailable\n"),
    }

    if let Some(reason) = refresh_status.reason.as_deref() {
        out.push_str(&format!("- diagnosis: {}\n", reason));
    }
    if let Some(next_step) = refresh_status.next_step.as_deref() {
        out.push_str(&format!("- next step: {}\n", next_step));
    }

    Some(out)
}

fn build_loaded_bootstrap_summary(evidences: &[Evidence]) -> Option<String> {
    let by_file: HashMap<&str, &Evidence> =
        evidences.iter().map(|e| (e.file.as_str(), e)).collect();

    let consumer = by_file
        .get("logs/consumer.log")
        .and_then(|e| parse_consumer_bootstrap_diagnostics(e.content.as_str()));
    let emulator = by_file
        .get("logs/emulator.log")
        .and_then(|e| parse_emulator_bootstrap_diagnostics(e.content.as_str()));

    if consumer.is_none() && emulator.is_none() {
        return None;
    }

    let mut out = String::new();
    match consumer.as_ref() {
        Some(diag) => {
            out.push_str("- consumer loaded bootstrap:\n");
            out.push_str(&format!("  - event: `{}`\n", diag.event));
            out.push_str(&format!(
                "  - signature: `{}`\n",
                compact_bootstrap_signature(diag.signature.as_str())
            ));
            out.push_str(&format!(
                "  - runtime refs: {}\n",
                format_runtime_refs(diag.runtime_refs.as_slice())
            ));
        }
        None => out.push_str("- consumer loaded bootstrap: unavailable in logs\n"),
    }

    match emulator.as_ref() {
        Some(diag) => {
            out.push_str("- emulator loaded bootstrap:\n");
            out.push_str(&format!("  - event: `{}`\n", diag.event));
            out.push_str(&format!(
                "  - signature: `{}`\n",
                compact_bootstrap_signature(diag.signature.as_str())
            ));
            out.push_str(&format!(
                "  - runtime refs: {}\n",
                format_runtime_refs(diag.runtime_refs.as_slice())
            ));
        }
        None => out.push_str("- emulator loaded bootstrap: unavailable in logs\n"),
    }

    if let (Some(consumer), Some(emulator)) = (consumer.as_ref(), emulator.as_ref()) {
        let aligned = consumer.signature == emulator.signature
            && consumer.runtime_refs == emulator.runtime_refs;
        out.push_str(&format!(
            "- loaded bootstrap alignment: `{}`\n",
            if aligned { "aligned" } else { "mismatch" }
        ));
        if !aligned {
            out.push_str(
                "- diagnosis: consumer and emulator loaded different aggregate bootstrap generations\n",
            );
            out.push_str("- next step: inspect `logs/consumer.log` and `logs/emulator.log`, then rerun `raccoon-cli scenario-smoke happy-path`\n");
        }
    }

    Some(out)
}

fn format_runtime_refs(runtime_refs: &[String]) -> String {
    if runtime_refs.is_empty() {
        return "none".into();
    }
    runtime_refs
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn extract_reconcile_interval(raw: &str) -> Option<String> {
    let cleaned = strip_jsonc_comments(raw);
    let value: Value = serde_json::from_str(&cleaned).ok()?;
    value
        .get("bootstrap")?
        .get("reconcile_interval")?
        .as_str()
        .map(|s| s.to_string())
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if escape_next {
            result.push(chars[i]);
            escape_next = false;
            i += 1;
            continue;
        }
        if chars[i] == '\\' && in_string {
            result.push(chars[i]);
            escape_next = true;
            i += 1;
            continue;
        }
        if chars[i] == '"' {
            in_string = !in_string;
            result.push(chars[i]);
            i += 1;
            continue;
        }
        if !in_string && i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

#[derive(Debug)]
struct RefreshDurableStatus {
    name: String,
    num_pending: u64,
    num_ack_pending: u64,
    num_redelivered: u64,
    delivered_consumer_seq: u64,
    ack_floor_consumer_seq: u64,
    last_active_epoch_seconds: Option<u64>,
    observed_epoch_seconds: Option<u64>,
}

#[derive(Debug)]
struct RefreshHealth {
    state: RefreshHealthState,
    mode: RefreshHealthMode,
    durables: Option<Vec<RefreshDurableStatus>>,
    reason: Option<String>,
    next_step: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum RefreshHealthState {
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Copy)]
enum RefreshHealthMode {
    CaughtUp,
    TelemetryUnavailable,
    DurableMissing,
    CadenceMismatch,
    TransientLag,
    StalledRefresh,
    RedeliveryDetected,
}

impl RefreshHealth {
    fn label(&self) -> &'static str {
        match self.state {
            RefreshHealthState::Healthy => "healthy",
            RefreshHealthState::Degraded => "degraded",
        }
    }

    fn mode_label(&self) -> &'static str {
        match self.mode {
            RefreshHealthMode::CaughtUp => "caught-up",
            RefreshHealthMode::TelemetryUnavailable => "telemetry-unavailable",
            RefreshHealthMode::DurableMissing => "durable-missing",
            RefreshHealthMode::CadenceMismatch => "cadence-mismatch",
            RefreshHealthMode::TransientLag => "transient-lag",
            RefreshHealthMode::StalledRefresh => "stalled-refresh",
            RefreshHealthMode::RedeliveryDetected => "redelivery-detected",
        }
    }
}

fn classify_refresh_status(
    consumer_interval: Option<&str>,
    emulator_interval: Option<&str>,
    durables: Option<Vec<RefreshDurableStatus>>,
) -> RefreshHealth {
    let cadence_window_seconds =
        cadence_window_seconds(consumer_interval, emulator_interval).unwrap_or(60);

    match durables {
        None => RefreshHealth {
            state: RefreshHealthState::Degraded,
            mode: RefreshHealthMode::TelemetryUnavailable,
            durables: None,
            reason: Some("JetStream state for refresh durables is unavailable".into()),
            next_step: Some(
                "rerun `raccoon-cli trace-pack`, then inspect `nats/jsz.json` and service logs"
                    .into(),
            ),
        },
        Some(ref d) if d.is_empty() => RefreshHealth {
            state: RefreshHealthState::Degraded,
            mode: RefreshHealthMode::DurableMissing,
            durables,
            reason: Some("refresh durables were not found under `CONFIGCTL_EVENTS`".into()),
            next_step: Some(
                "run `raccoon-cli contract-audit` and confirm refresh durables are registered in the configctl NATS registry".into(),
            ),
        },
        Some(d) => {
            let mismatched_intervals = consumer_interval != emulator_interval;
            let lagging = d.iter().any(RefreshDurableStatus::has_backlog);
            let redelivering = d.iter().any(|durable| durable.num_redelivered > 0);

            if mismatched_intervals {
                return RefreshHealth {
                    state: RefreshHealthState::Degraded,
                    mode: RefreshHealthMode::CadenceMismatch,
                    durables: Some(d),
                    reason: Some("consumer and emulator use different reconcile intervals".into()),
                    next_step: Some(
                        "align `bootstrap.reconcile_interval` in deploy configs, rerun `raccoon-cli topology-doctor`, then rerun `raccoon-cli trace-pack`".into(),
                    ),
                };
            }

            if redelivering {
                return RefreshHealth {
                    state: RefreshHealthState::Degraded,
                    mode: RefreshHealthMode::RedeliveryDetected,
                    durables: Some(d),
                    reason: Some("refresh durable redelivery is non-zero".into()),
                    next_step: Some(
                        "inspect `consumer` and `emulator` logs, then run `raccoon-cli contract-audit`, `raccoon-cli runtime-bindings`, and `raccoon-cli scenario-smoke happy-path`".into(),
                    ),
                };
            }

            if lagging {
                let transient = d.iter().all(|durable| {
                    !durable.has_backlog()
                        || durable
                            .lag_age_seconds()
                            .is_some_and(|age| age <= cadence_window_seconds)
                });

                return if transient {
                    RefreshHealth {
                        state: RefreshHealthState::Degraded,
                        mode: RefreshHealthMode::TransientLag,
                        durables: Some(d),
                        reason: Some(format!(
                            "refresh lag is non-zero, but durable activity is recent within {}s",
                            cadence_window_seconds
                        )),
                        next_step: Some(
                            "rerun `raccoon-cli trace-pack` after one reconcile window; if lag persists, run `raccoon-cli scenario-smoke happy-path`".into(),
                        ),
                    }
                } else {
                    RefreshHealth {
                        state: RefreshHealthState::Degraded,
                        mode: RefreshHealthMode::StalledRefresh,
                        durables: Some(d),
                        reason: Some(
                            "refresh lag is non-zero and durable activity appears stale".into(),
                        ),
                        next_step: Some(
                            "inspect `consumer` and `emulator` logs, then run `raccoon-cli contract-audit`, `raccoon-cli runtime-bindings`, and rerun `raccoon-cli scenario-smoke happy-path`".into(),
                        ),
                    }
                };
            }

            RefreshHealth {
                state: RefreshHealthState::Healthy,
                mode: RefreshHealthMode::CaughtUp,
                durables: Some(d),
                reason: Some("refresh durables are caught up and reconcile cadence is aligned".into()),
                next_step: None,
            }
        }
    }
}

fn cadence_window_seconds(
    consumer_interval: Option<&str>,
    emulator_interval: Option<&str>,
) -> Option<u64> {
    let consumer = consumer_interval.and_then(parse_simple_duration_seconds)?;
    let emulator = emulator_interval.and_then(parse_simple_duration_seconds)?;
    Some(consumer.max(emulator).saturating_mul(2).max(30))
}

fn parse_simple_duration_seconds(raw: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut value = String::new();

    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            value.push(ch);
            continue;
        }

        let amount = value.parse::<u64>().ok()?;
        value.clear();
        total = total.checked_add(match ch {
            'h' => amount.checked_mul(3600)?,
            'm' => amount.checked_mul(60)?,
            's' => amount,
            _ => return None,
        })?;
    }

    if value.is_empty() {
        Some(total)
    } else {
        None
    }
}

fn parse_rfc3339_epoch_seconds(raw: &str) -> Option<u64> {
    let raw = raw.strip_suffix('Z')?;
    let (date, time) = raw.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<u64>().ok()?;
    let month = date_parts.next()?.parse::<u64>().ok()?;
    let day = date_parts.next()?.parse::<u64>().ok()?;

    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u64>().ok()?;
    let minute = time_parts.next()?.parse::<u64>().ok()?;
    let second = time_parts.next()?.parse::<u64>().ok()?;

    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let adjust = if month <= 2 { 1 } else { 0 };
    let y = year.checked_sub(adjust)?;
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe;
    days.checked_sub(719_468)
}

impl RefreshDurableStatus {
    fn has_backlog(&self) -> bool {
        self.num_pending > 0 || self.num_ack_pending > 0
    }

    fn lag_age_seconds(&self) -> Option<u64> {
        let observed = self.observed_epoch_seconds?;
        let last_active = self.last_active_epoch_seconds?;
        observed.checked_sub(last_active)
    }
}

fn parse_refresh_durables(raw: &str) -> Option<Vec<RefreshDurableStatus>> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let account_details = value.get("account_details")?.as_array()?;
    let mut durables = Vec::new();

    for account in account_details {
        let streams = account.get("stream_detail")?.as_array()?;
        for stream in streams {
            if stream.get("name")?.as_str()? != "CONFIGCTL_EVENTS" {
                continue;
            }
            let consumers = stream.get("consumer_detail")?.as_array()?;
            for consumer in consumers {
                let name = consumer.get("name")?.as_str()?.to_string();
                if name != "consumer-runtime-refresh-v1" && name != "emulator-runtime-refresh-v1" {
                    continue;
                }
                let delivered = consumer.get("delivered");
                let ack_floor = consumer.get("ack_floor");
                let delivered_consumer_seq = delivered
                    .and_then(|value| value.get("consumer_seq"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let ack_floor_consumer_seq = ack_floor
                    .and_then(|value| value.get("consumer_seq"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let last_active_epoch_seconds = delivered
                    .and_then(|value| value.get("last_active"))
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339_epoch_seconds)
                    .or_else(|| {
                        ack_floor
                            .and_then(|value| value.get("last_active"))
                            .and_then(Value::as_str)
                            .and_then(parse_rfc3339_epoch_seconds)
                    });
                let observed_epoch_seconds = consumer
                    .get("ts")
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339_epoch_seconds);

                durables.push(RefreshDurableStatus {
                    name,
                    num_pending: consumer
                        .get("num_pending")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    num_ack_pending: consumer
                        .get("num_ack_pending")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    num_redelivered: consumer
                        .get("num_redelivered")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    delivered_consumer_seq,
                    ack_floor_consumer_seq,
                    last_active_epoch_seconds,
                    observed_epoch_seconds,
                });
            }
        }
    }

    durables.sort_by(|a, b| a.name.cmp(&b.name));
    Some(durables)
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
        let summary = build_summary("20260314-120000", &[], &collected, &failed);
        assert!(summary.contains("Compose status"));
        assert!(summary.contains("compose-status.txt"));
        assert!(summary.contains("configctl-runtime-projections.json"));
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
        let summary = build_summary("20260314-120000", &[], &collected, &failed);
        assert!(summary.contains("Unavailable evidence"));
        assert!(summary.contains("connection refused"));
    }

    #[test]
    fn build_summary_contains_refresh_observability() {
        let evidences = vec![
            Evidence {
                name: "Deploy config: consumer.jsonc".into(),
                file: "configs/consumer.jsonc".into(),
                content: "{\n\"bootstrap\":{\"reconcile_interval\":\"30s\"}\n}".into(),
                status: EvidenceStatus::Ok,
            },
            Evidence {
                name: "Deploy config: emulator.jsonc".into(),
                file: "configs/emulator.jsonc".into(),
                content: "{\n\"bootstrap\":{\"reconcile_interval\":\"45s\"}\n}".into(),
                status: EvidenceStatus::Ok,
            },
            Evidence {
                name: "JetStream state".into(),
                file: "nats/jsz.json".into(),
                content: r#"{"account_details":[{"stream_detail":[{"name":"CONFIGCTL_EVENTS","consumer_detail":[{"name":"consumer-runtime-refresh-v1","num_pending":2,"num_ack_pending":1,"num_redelivered":0,"ts":"2026-03-16T16:12:37Z","delivered":{"consumer_seq":15,"last_active":"2026-03-16T16:12:31Z"},"ack_floor":{"consumer_seq":14,"last_active":"2026-03-16T16:12:30Z"}},{"name":"emulator-runtime-refresh-v1","num_pending":0,"num_ack_pending":0,"num_redelivered":0,"ts":"2026-03-16T16:12:37Z","delivered":{"consumer_seq":12,"last_active":"2026-03-16T16:12:31Z"},"ack_floor":{"consumer_seq":12,"last_active":"2026-03-16T16:12:31Z"}}]}]}]}"#.into(),
                status: EvidenceStatus::Ok,
            },
        ];

        let summary = build_summary("20260314-120000", &evidences, &[], &[]);
        assert!(summary.contains("Refresh observability"));
        assert!(summary.contains("refresh status: `degraded`"));
        assert!(summary.contains("refresh mode: `cadence-mismatch`"));
        assert!(summary.contains("consumer=`30s`, emulator=`45s`"));
        assert!(summary.contains("consumer-runtime-refresh-v1"));
        assert!(summary.contains("pending=`2`"));
        assert!(summary.contains("delivered=`15`"));
    }

    #[test]
    fn build_summary_contains_loaded_bootstrap_diagnostics() {
        let evidences = vec![
            Evidence {
                name: "consumer log".into(),
                file: "logs/consumer.log".into(),
                content: r#"time=2026-03-16T18:00:00Z level=INFO msg="consumer runtime ready" generation=2 topics="[sales.order.created]" bindings=1 bootstrap_signature="binding|tenant|br|||ver-br|0|sum-br|artifact-br||artifact-sum-br||validator:v1|orders|sales.order.created\nruntime|tenant|br|||ver-br|0|sum-br|artifact-br||artifact-sum-br||validator:v1" runtime_refs="[tenant:br:ver-br:artifact-br]""#.into(),
                status: EvidenceStatus::Ok,
            },
            Evidence {
                name: "emulator log".into(),
                file: "logs/emulator.log".into(),
                content: r#"time=2026-03-16T18:00:01Z level=INFO msg="emulator started" topics="[sales.order.created]" bindings=1 bootstrap_signature="binding|tenant|br|||ver-br|0|sum-br|artifact-br||artifact-sum-br||validator:v1|orders|sales.order.created\nruntime|tenant|br|||ver-br|0|sum-br|artifact-br||artifact-sum-br||validator:v1" runtime_refs="[tenant:br:ver-br:artifact-br]""#.into(),
                status: EvidenceStatus::Ok,
            },
        ];

        let summary = build_summary("20260314-120000", &evidences, &[], &[]);
        assert!(summary.contains("Loaded bootstrap diagnostics"));
        assert!(summary.contains("consumer loaded bootstrap"));
        assert!(summary.contains("emulator loaded bootstrap"));
        assert!(summary.contains("loaded bootstrap alignment: `aligned`"));
        assert!(summary.contains("tenant:br:ver-br:artifact-br"));
    }

    #[test]
    fn build_summary_reports_loaded_bootstrap_mismatch() {
        let evidences = vec![
            Evidence {
                name: "consumer log".into(),
                file: "logs/consumer.log".into(),
                content: r#"time=2026-03-16T18:00:00Z level=INFO msg="consumer runtime ready" generation=2 topics="[sales.order.created]" bindings=1 bootstrap_signature="consumer-signature" runtime_refs="[tenant:br:ver-br:artifact-br]""#.into(),
                status: EvidenceStatus::Ok,
            },
            Evidence {
                name: "emulator log".into(),
                file: "logs/emulator.log".into(),
                content: r#"time=2026-03-16T18:00:01Z level=INFO msg="emulator bootstrap refreshed" topics="[sales.order.created]" bindings=1 bootstrap_signature="emulator-signature" runtime_refs="[tenant:us:ver-us:artifact-us]""#.into(),
                status: EvidenceStatus::Ok,
            },
        ];

        let summary = build_summary("20260314-120000", &evidences, &[], &[]);
        assert!(summary.contains("loaded bootstrap alignment: `mismatch`"));
        assert!(summary.contains("different aggregate bootstrap generations"));
    }

    #[test]
    fn parse_loaded_bootstrap_diagnostics_uses_latest_matching_line() {
        let parsed = parse_consumer_bootstrap_diagnostics(
            r#"time=2026-03-16T18:00:00Z level=INFO msg="consumer runtime ready" bootstrap_signature="old" runtime_refs="[tenant:br:old:artifact-old]"
time=2026-03-16T18:00:01Z level=INFO msg="consumer runtime ready" bootstrap_signature="new" runtime_refs="[tenant:br:new:artifact-new]""#,
        )
        .expect("consumer diagnostics");

        assert_eq!(parsed.source, "consumer");
        assert_eq!(parsed.signature, "new");
        assert_eq!(parsed.runtime_refs, vec!["tenant:br:new:artifact-new"]);
    }

    #[test]
    fn classify_refresh_status_is_healthy_when_caught_up_and_aligned() {
        let status = classify_refresh_status(
            Some("30s"),
            Some("30s"),
            Some(vec![
                RefreshDurableStatus {
                    name: "consumer-runtime-refresh-v1".into(),
                    num_pending: 0,
                    num_ack_pending: 0,
                    num_redelivered: 0,
                    delivered_consumer_seq: 4,
                    ack_floor_consumer_seq: 4,
                    last_active_epoch_seconds: Some(120),
                    observed_epoch_seconds: Some(120),
                },
                RefreshDurableStatus {
                    name: "emulator-runtime-refresh-v1".into(),
                    num_pending: 0,
                    num_ack_pending: 0,
                    num_redelivered: 0,
                    delivered_consumer_seq: 4,
                    ack_floor_consumer_seq: 4,
                    last_active_epoch_seconds: Some(120),
                    observed_epoch_seconds: Some(120),
                },
            ]),
        );

        assert_eq!(status.label(), "healthy");
        assert_eq!(status.mode_label(), "caught-up");
        assert!(status.next_step.is_none());
    }

    #[test]
    fn classify_refresh_status_is_degraded_when_lagging_is_recent() {
        let status = classify_refresh_status(
            Some("30s"),
            Some("30s"),
            Some(vec![RefreshDurableStatus {
                name: "consumer-runtime-refresh-v1".into(),
                num_pending: 1,
                num_ack_pending: 0,
                num_redelivered: 0,
                delivered_consumer_seq: 9,
                ack_floor_consumer_seq: 8,
                last_active_epoch_seconds: Some(100),
                observed_epoch_seconds: Some(120),
            }]),
        );

        assert_eq!(status.label(), "degraded");
        assert_eq!(status.mode_label(), "transient-lag");
        assert!(status.reason.as_deref().unwrap_or("").contains("recent"));
        assert!(status.next_step.is_some());
    }

    #[test]
    fn classify_refresh_status_is_degraded_when_lagging_is_stale() {
        let status = classify_refresh_status(
            Some("30s"),
            Some("30s"),
            Some(vec![RefreshDurableStatus {
                name: "consumer-runtime-refresh-v1".into(),
                num_pending: 3,
                num_ack_pending: 0,
                num_redelivered: 0,
                delivered_consumer_seq: 10,
                ack_floor_consumer_seq: 7,
                last_active_epoch_seconds: Some(100),
                observed_epoch_seconds: Some(220),
            }]),
        );

        assert_eq!(status.label(), "degraded");
        assert_eq!(status.mode_label(), "stalled-refresh");
        assert!(status.reason.as_deref().unwrap_or("").contains("stale"));
    }

    #[test]
    fn parse_simple_duration_seconds_supports_compound_values() {
        assert_eq!(parse_simple_duration_seconds("1m30s"), Some(90));
        assert_eq!(parse_simple_duration_seconds("2h"), Some(7_200));
        assert_eq!(parse_simple_duration_seconds("30s"), Some(30));
    }

    #[test]
    fn parse_rfc3339_epoch_seconds_parses_utc_timestamp() {
        let ts = parse_rfc3339_epoch_seconds("1970-01-01T00:01:30Z");
        assert_eq!(ts, Some(90));
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
