use crate::process_utils::run_command_with_timeout;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Status of a single evidence collection attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", content = "detail")]
pub enum EvidenceStatus {
    Ok,
    Unavailable(String),
    Error(String),
}

/// A single piece of collected evidence.
#[derive(Debug)]
pub struct Evidence {
    pub name: String,
    pub file: String,
    pub content: String,
    pub status: EvidenceStatus,
}

/// Collects diagnostic evidence from compose, API, configs, and logs.
pub struct Collector {
    compose_file: PathBuf,
    base_url: String,
    project_root: PathBuf,
    log_lines: u32,
    results_limit: u32,
}

const CONFIG_FILES: &[&str] = &[
    "server.jsonc",
    "consumer.jsonc",
    "validator.jsonc",
    "emulator.jsonc",
    "configctl.jsonc",
];

const SERVICES: &[&str] = &[
    "nats",
    "kafka",
    "configctl",
    "server",
    "consumer",
    "validator",
    "emulator",
];

const NATS_MONITOR_BASE_URL: &str = "http://127.0.0.1:8222";

impl Collector {
    pub fn new(
        compose_file: &Path,
        base_url: &str,
        project_root: &Path,
        log_lines: u32,
        results_limit: u32,
    ) -> Self {
        Self {
            compose_file: compose_file.to_path_buf(),
            base_url: base_url.trim_end_matches('/').to_string(),
            project_root: project_root.to_path_buf(),
            log_lines,
            results_limit,
        }
    }

    /// Collect all evidence sources. Each collector is independent —
    /// failures in one do not affect others.
    pub fn collect_all(&self) -> Vec<Evidence> {
        let mut evidences = Vec::new();

        evidences.push(self.collect_compose_status());
        evidences.push(self.collect_nats_healthz());
        evidences.push(self.collect_jetstream_state());
        evidences.push(self.collect_healthz());
        evidences.push(self.collect_readyz());
        evidences.push(self.collect_active_config());
        evidences.push(self.collect_configctl_runtime_projections());
        evidences.push(self.collect_ingestion_bindings());
        evidences.push(self.collect_validator_runtime());
        evidences.push(self.collect_validation_results());
        evidences.extend(self.collect_deploy_configs());
        evidences.extend(self.collect_service_logs());

        evidences
    }

    fn collect_compose_status(&self) -> Evidence {
        let name = "Compose status".to_string();
        let file = "compose-status.txt".to_string();

        let compose_dir = match self.compose_file.parent() {
            Some(d) => d,
            None => {
                return Evidence {
                    name,
                    file,
                    content: String::new(),
                    status: EvidenceStatus::Error("cannot determine compose directory".into()),
                };
            }
        };
        let compose_file_arg = self
            .compose_file
            .canonicalize()
            .unwrap_or_else(|_| self.compose_file.clone());

        let mut command = Command::new("docker");
        command
            .args(["compose", "-f"])
            .arg(&compose_file_arg)
            .args(["ps", "--all"])
            .current_dir(compose_dir);

        match run_command_with_timeout(&mut command, Duration::from_secs(5), "docker compose ps") {
            Ok(output) if output.status.success() => {
                let stdout = output.stdout;
                Evidence {
                    name,
                    file,
                    content: stdout,
                    status: EvidenceStatus::Ok,
                }
            }
            Ok(output) => {
                let stderr = output.stderr.trim().to_string();
                Evidence {
                    name,
                    file,
                    content: String::new(),
                    status: EvidenceStatus::Unavailable(format!(
                        "docker compose ps failed: {stderr}"
                    )),
                }
            }
            Err(e) => Evidence {
                name,
                file,
                content: String::new(),
                status: EvidenceStatus::Unavailable(format!("docker not available: {e}")),
            },
        }
    }

    fn collect_healthz(&self) -> Evidence {
        self.collect_endpoint("Health check", "healthz.json", "/healthz")
    }

    fn collect_nats_healthz(&self) -> Evidence {
        self.collect_external_endpoint(
            "NATS monitor health",
            "nats/healthz.json",
            NATS_MONITOR_BASE_URL,
            "/healthz",
        )
    }

    fn collect_jetstream_state(&self) -> Evidence {
        self.collect_external_endpoint(
            "JetStream state",
            "nats/jsz.json",
            NATS_MONITOR_BASE_URL,
            "/jsz?streams=true&consumers=true&config=true",
        )
    }

    fn collect_readyz(&self) -> Evidence {
        self.collect_endpoint("Readiness check", "readyz.json", "/readyz")
    }

    fn collect_active_config(&self) -> Evidence {
        self.collect_endpoint(
            "Active config",
            "active-config.json",
            "/configctl/configs/active",
        )
    }

    fn collect_configctl_runtime_projections(&self) -> Evidence {
        self.collect_endpoint(
            "Configctl runtime projections",
            "configctl-runtime-projections.json",
            "/runtime/configctl/projections?scope_kind=global&scope_key=default",
        )
    }

    fn collect_ingestion_bindings(&self) -> Evidence {
        self.collect_endpoint(
            "Ingestion bindings",
            "ingestion-bindings.json",
            "/runtime/ingestion/bindings?scope_kind=global&scope_key=default",
        )
    }

    fn collect_validator_runtime(&self) -> Evidence {
        self.collect_endpoint(
            "Validator runtime",
            "validator-runtime.json",
            "/runtime/validator/active?scope_kind=global&scope_key=default",
        )
    }

    fn collect_validation_results(&self) -> Evidence {
        let path = format!(
            "/runtime/validator/results?scope_kind=global&scope_key=default&limit={}",
            self.results_limit
        );
        self.collect_endpoint("Validation results", "validation-results.json", &path)
    }

    fn collect_endpoint(&self, name: &str, file: &str, path: &str) -> Evidence {
        let url = format!("{}{path}", self.base_url);
        self.collect_url(name, file, &url)
    }

    fn collect_external_endpoint(
        &self,
        name: &str,
        file: &str,
        base_url: &str,
        path: &str,
    ) -> Evidence {
        let url = format!("{}{path}", base_url.trim_end_matches('/'));
        self.collect_url(name, file, &url)
    }

    fn collect_url(&self, name: &str, file: &str, url: &str) -> Evidence {
        match ureq::get(&url)
            .set("Accept", "application/json")
            .set("X-Correlation-ID", "raccoon-trace-pack")
            .timeout(Duration::from_secs(5))
            .call()
        {
            Ok(resp) => {
                let body = resp
                    .into_string()
                    .unwrap_or_else(|e| format!("{{\"error\": \"failed to read body: {e}\"}}"));
                // Try to pretty-print JSON
                let content = match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(val) => serde_json::to_string_pretty(&val).unwrap_or(body),
                    Err(_) => body,
                };
                Evidence {
                    name: name.into(),
                    file: file.into(),
                    content,
                    status: EvidenceStatus::Ok,
                }
            }
            Err(ureq::Error::Status(_code, resp)) => {
                let body = resp.into_string().unwrap_or_else(|_| String::new());
                Evidence {
                    name: name.into(),
                    file: file.into(),
                    content: body,
                    status: EvidenceStatus::Ok, // Non-200 is still useful evidence
                }
            }
            Err(e) => Evidence {
                name: name.into(),
                file: file.into(),
                content: String::new(),
                status: EvidenceStatus::Unavailable(format!("{e}")),
            },
        }
    }

    fn collect_deploy_configs(&self) -> Vec<Evidence> {
        let configs_dir = self.project_root.join("deploy/configs");
        CONFIG_FILES
            .iter()
            .map(|filename| {
                let path = configs_dir.join(filename);
                let file = format!("configs/{filename}");
                match std::fs::read_to_string(&path) {
                    Ok(content) => Evidence {
                        name: format!("Deploy config: {filename}"),
                        file,
                        content,
                        status: EvidenceStatus::Ok,
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Evidence {
                        name: format!("Deploy config: {filename}"),
                        file,
                        content: String::new(),
                        status: EvidenceStatus::Unavailable("file not found".into()),
                    },
                    Err(e) => Evidence {
                        name: format!("Deploy config: {filename}"),
                        file,
                        content: String::new(),
                        status: EvidenceStatus::Error(format!("read error: {e}")),
                    },
                }
            })
            .collect()
    }

    fn collect_service_logs(&self) -> Vec<Evidence> {
        let compose_dir = match self.compose_file.parent() {
            Some(d) => d,
            None => return vec![],
        };

        SERVICES
            .iter()
            .map(|service| {
                let name = format!("Logs: {service}");
                let file = format!("logs/{service}.log");
                let tail = self.log_lines.to_string();
                let compose_file_arg = self
                    .compose_file
                    .canonicalize()
                    .unwrap_or_else(|_| self.compose_file.clone());

                let mut command = Command::new("docker");
                command
                    .args(["compose", "-f"])
                    .arg(&compose_file_arg)
                    .args(["logs", "--no-color", "--tail", &tail, service])
                    .current_dir(compose_dir);

                match run_command_with_timeout(
                    &mut command,
                    Duration::from_secs(5),
                    &format!("docker compose logs {service}"),
                ) {
                    Ok(output) if output.status.success() => {
                        let stdout = output.stdout;
                        if stdout.trim().is_empty() {
                            Evidence {
                                name,
                                file,
                                content: String::new(),
                                status: EvidenceStatus::Unavailable("no log output".into()),
                            }
                        } else {
                            Evidence {
                                name,
                                file,
                                content: stdout,
                                status: EvidenceStatus::Ok,
                            }
                        }
                    }
                    Ok(output) => {
                        let stderr = output.stderr.trim().to_string();
                        Evidence {
                            name,
                            file,
                            content: String::new(),
                            status: EvidenceStatus::Unavailable(format!(
                                "docker logs failed: {stderr}"
                            )),
                        }
                    }
                    Err(e) => Evidence {
                        name,
                        file,
                        content: String::new(),
                        status: EvidenceStatus::Unavailable(format!("docker not available: {e}")),
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_deploy_configs_reads_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let configs_dir = tmp.path().join("deploy/configs");
        std::fs::create_dir_all(&configs_dir).unwrap();
        std::fs::write(configs_dir.join("server.jsonc"), r#"{"log":"info"}"#).unwrap();
        std::fs::write(configs_dir.join("consumer.jsonc"), r#"{"kafka":{}}"#).unwrap();

        let collector = Collector::new(
            &tmp.path().join("deploy/compose/docker-compose.yaml"),
            "http://127.0.0.1:19999",
            tmp.path(),
            50,
            10,
        );

        let evidences = collector.collect_deploy_configs();

        let found: Vec<_> = evidences
            .iter()
            .filter(|e| matches!(e.status, EvidenceStatus::Ok))
            .collect();
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|e| e.file == "configs/server.jsonc"));
        assert!(found.iter().any(|e| e.file == "configs/consumer.jsonc"));

        let missing: Vec<_> = evidences
            .iter()
            .filter(|e| matches!(e.status, EvidenceStatus::Unavailable(_)))
            .collect();
        // validator.jsonc, emulator.jsonc, configctl.jsonc not present
        assert_eq!(missing.len(), 3);
    }

    #[test]
    fn collect_deploy_configs_handles_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let collector = Collector::new(
            &tmp.path().join("deploy/compose/docker-compose.yaml"),
            "http://127.0.0.1:19999",
            tmp.path(),
            50,
            10,
        );

        let evidences = collector.collect_deploy_configs();
        assert_eq!(evidences.len(), CONFIG_FILES.len());
        for ev in &evidences {
            assert!(
                matches!(ev.status, EvidenceStatus::Unavailable(_)),
                "expected unavailable for {}, got {:?}",
                ev.name,
                ev.status
            );
        }
    }

    #[test]
    fn evidence_status_serializes_correctly() {
        let ok = serde_json::to_string(&EvidenceStatus::Ok).unwrap();
        assert!(ok.contains("Ok"));

        let unavail = serde_json::to_string(&EvidenceStatus::Unavailable("gone".into())).unwrap();
        assert!(unavail.contains("Unavailable"));
        assert!(unavail.contains("gone"));
    }

    #[test]
    fn collect_endpoint_unreachable_returns_unavailable() {
        let collector = Collector::new(
            Path::new("/tmp/docker-compose.yaml"),
            "http://127.0.0.1:19999",
            Path::new("/tmp"),
            50,
            10,
        );

        let ev = collector.collect_endpoint("test", "test.json", "/healthz");
        assert!(
            matches!(ev.status, EvidenceStatus::Unavailable(_)),
            "expected unavailable, got {:?}",
            ev.status
        );
    }

    #[test]
    fn collector_new_strips_trailing_slash() {
        let c = Collector::new(
            Path::new("/tmp/dc.yaml"),
            "http://localhost:8080/",
            Path::new("/tmp"),
            50,
            10,
        );
        assert_eq!(c.base_url, "http://localhost:8080");
    }

    #[test]
    fn config_files_are_known_jsonc() {
        for f in CONFIG_FILES {
            assert!(f.ends_with(".jsonc"), "unexpected extension: {f}");
        }
    }

    #[test]
    fn services_match_expected_set() {
        assert!(SERVICES.contains(&"nats"));
        assert!(SERVICES.contains(&"kafka"));
        assert!(SERVICES.contains(&"server"));
        assert!(SERVICES.contains(&"consumer"));
        assert!(SERVICES.contains(&"validator"));
        assert!(SERVICES.contains(&"emulator"));
        assert!(SERVICES.contains(&"configctl"));
    }

    #[test]
    fn collect_all_returns_expected_count() {
        let tmp = tempfile::tempdir().unwrap();
        let configs_dir = tmp.path().join("deploy/configs");
        std::fs::create_dir_all(&configs_dir).unwrap();

        let compose_dir = tmp.path().join("deploy/compose");
        std::fs::create_dir_all(&compose_dir).unwrap();
        std::fs::write(compose_dir.join("docker-compose.yaml"), "").unwrap();

        let collector = Collector::new(
            &compose_dir.join("docker-compose.yaml"),
            "http://127.0.0.1:19999",
            tmp.path(),
            50,
            10,
        );

        let evidences = collector.collect_all();
        // 1 compose + 2 NATS monitor + 2 health/readyz + 5 runtime endpoints + configs + logs
        let expected = 1 + 2 + 2 + 5 + CONFIG_FILES.len() + SERVICES.len();
        assert_eq!(
            evidences.len(),
            expected,
            "expected {expected} evidence items, got {}",
            evidences.len()
        );
    }
}
