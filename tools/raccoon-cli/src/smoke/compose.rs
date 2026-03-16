use crate::process_utils::run_command_with_timeout;
use std::process::Command;
use std::time::Duration;

/// Check if docker compose services in the dataplane profile are running.
/// Returns a list of running service names.
pub fn running_services(compose_file: &std::path::Path) -> Result<Vec<String>, String> {
    let compose_dir = compose_file
        .parent()
        .ok_or_else(|| "cannot determine compose directory".to_string())?;
    let compose_file_arg = compose_file
        .canonicalize()
        .unwrap_or_else(|_| compose_file.to_path_buf());

    let mut command = Command::new("docker");
    command
        .args(["compose", "-f"])
        .arg(&compose_file_arg)
        .args(["ps", "--format", "{{.Service}}:{{.State}}", "--all"])
        .current_dir(compose_dir);

    let output =
        run_command_with_timeout(&mut command, Duration::from_secs(5), "docker compose ps")?;

    if !output.status.success() {
        let stderr = output.stderr.trim();
        if stderr.is_empty() {
            return Err("docker compose ps failed".to_string());
        }
        return Err(format!("docker compose ps failed: {stderr}"));
    }

    let stdout = output.stdout;
    let running: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 && parts[1].contains("running") {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(running)
}

pub fn service_logs(
    compose_file: &std::path::Path,
    service: &str,
    tail_lines: u32,
) -> Result<String, String> {
    let compose_dir = compose_file
        .parent()
        .ok_or_else(|| "cannot determine compose directory".to_string())?;
    let compose_file_arg = compose_file
        .canonicalize()
        .unwrap_or_else(|_| compose_file.to_path_buf());

    let mut command = Command::new("docker");
    command
        .args(["compose", "-f"])
        .arg(&compose_file_arg)
        .args(["logs", "--no-color", "--tail"])
        .arg(tail_lines.to_string())
        .arg(service)
        .current_dir(compose_dir);

    let output =
        run_command_with_timeout(&mut command, Duration::from_secs(5), "docker compose logs")?;

    if !output.status.success() {
        let stderr = output.stderr.trim();
        if stderr.is_empty() {
            return Err(format!(
                "docker compose logs failed for service '{service}'"
            ));
        }
        return Err(format!(
            "docker compose logs failed for service '{service}': {stderr}"
        ));
    }

    Ok(output.stdout)
}

/// Required services for a full dataplane smoke test.
pub const REQUIRED_SERVICES: &[&str] = &[
    "nats",
    "kafka",
    "configctl",
    "server",
    "consumer",
    "validator",
    "emulator",
];

pub fn missing_required_services<'a>(running: &[String], required: &'a [&'a str]) -> Vec<&'a str> {
    required
        .iter()
        .filter(|svc| !running.iter().any(|r| r == **svc))
        .copied()
        .collect()
}

/// Check which required services are missing from the running set.
pub fn missing_services(running: &[String]) -> Vec<&'static str> {
    missing_required_services(running, REQUIRED_SERVICES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_services_all_present() {
        let running: Vec<String> = REQUIRED_SERVICES.iter().map(|s| s.to_string()).collect();
        assert!(missing_services(&running).is_empty());
    }

    #[test]
    fn missing_services_some_absent() {
        let running = vec!["nats".to_string(), "kafka".to_string()];
        let missing = missing_services(&running);
        assert!(missing.contains(&"configctl"));
        assert!(missing.contains(&"server"));
        assert!(missing.contains(&"consumer"));
        assert!(missing.contains(&"validator"));
        assert!(missing.contains(&"emulator"));
        assert!(!missing.contains(&"nats"));
        assert!(!missing.contains(&"kafka"));
    }

    #[test]
    fn missing_services_empty() {
        let missing = missing_services(&[]);
        assert_eq!(missing.len(), REQUIRED_SERVICES.len());
    }

    #[test]
    fn missing_required_services_supports_subset_bootstrap() {
        let running = vec!["nats".to_string(), "server".to_string()];
        let missing = missing_required_services(&running, &["nats", "configctl", "server"]);
        assert_eq!(missing, vec!["configctl"]);
    }
}
