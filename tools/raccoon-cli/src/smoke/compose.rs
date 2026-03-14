use std::process::Command;

/// Check if docker compose services in the dataplane profile are running.
/// Returns a list of running service names.
pub fn running_services(compose_file: &std::path::Path) -> Result<Vec<String>, String> {
    let compose_dir = compose_file
        .parent()
        .ok_or_else(|| "cannot determine compose directory".to_string())?;

    let output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args(["ps", "--format", "{{.Service}}:{{.State}}", "--all"])
        .current_dir(compose_dir)
        .output()
        .map_err(|e| format!("failed to run docker compose ps: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker compose ps failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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

/// Check which required services are missing from the running set.
pub fn missing_services(running: &[String]) -> Vec<&'static str> {
    REQUIRED_SERVICES
        .iter()
        .filter(|svc| !running.iter().any(|r| r == **svc))
        .copied()
        .collect()
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
}
