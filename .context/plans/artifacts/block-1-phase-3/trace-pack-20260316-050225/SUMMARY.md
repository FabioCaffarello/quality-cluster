# Trace Pack — 20260316-050225

Diagnostic evidence snapshot for quality-service cluster.

## What is this?

This pack contains runtime state, configuration, logs, and API responses
collected at a single point in time. Use it to diagnose failures without
needing live access to the cluster.

## Collected evidence

| File | Description |
|------|-------------|
| `configs/server.jsonc` | Deploy config: server.jsonc |
| `configs/consumer.jsonc` | Deploy config: consumer.jsonc |
| `configs/validator.jsonc` | Deploy config: validator.jsonc |
| `configs/emulator.jsonc` | Deploy config: emulator.jsonc |
| `configs/configctl.jsonc` | Deploy config: configctl.jsonc |

## Unavailable evidence

| Evidence | Reason |
|----------|--------|
| Compose status | docker compose ps failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Health check | http://127.0.0.1:8080/healthz: Connection Failed: Connect error: Connection refused (os error 61) |
| Readiness check | http://127.0.0.1:8080/readyz: Connection Failed: Connect error: Connection refused (os error 61) |
| Active config | http://127.0.0.1:8080/configctl/configs/active: Connection Failed: Connect error: Connection refused (os error 61) |
| Ingestion bindings | http://127.0.0.1:8080/runtime/ingestion/bindings?scope_kind=global&scope_key=default: Connection Failed: Connect error: Connection refused (os error 61) |
| Validator runtime | http://127.0.0.1:8080/runtime/validator/active?scope_kind=global&scope_key=default: Connection Failed: Connect error: Connection refused (os error 61) |
| Validation results | http://127.0.0.1:8080/runtime/validator/results?scope_kind=global&scope_key=default&limit=20: Connection Failed: Connect error: Connection refused (os error 61) |
| Logs: nats | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: kafka | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: configctl | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: server | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: consumer | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: validator | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |
| Logs: emulator | docker logs failed: Cannot connect to the Docker daemon at unix:///Volumes/OWC Express 1M2/.docker/run/docker.sock. Is the docker daemon running? |

Unavailable items indicate services that were down or unreachable at collection time.

## How to use

1. Check `compose-status.txt` for service health overview
2. Review `healthz.json` and `readyz.json` for API readiness
3. Inspect `active-config.json` for the running configuration
4. Check `ingestion-bindings.json` for active data routing
5. Review `validation-results.json` for recent pass/fail outcomes
6. Examine `logs/` for per-service console output
7. Compare `configs/` with active runtime to spot drift
