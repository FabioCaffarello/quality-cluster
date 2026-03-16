# Trace Pack — 20260316-051232

Diagnostic evidence snapshot for quality-service cluster.

## What is this?

This pack contains runtime state, configuration, logs, and API responses
collected at a single point in time. Use it to diagnose failures without
needing live access to the cluster.

## Collected evidence

| File | Description |
|------|-------------|
| `compose-status.txt` | Compose status |
| `healthz.json` | Health check |
| `readyz.json` | Readiness check |
| `active-config.json` | Active config |
| `ingestion-bindings.json` | Ingestion bindings |
| `validator-runtime.json` | Validator runtime |
| `validation-results.json` | Validation results |
| `configs/server.jsonc` | Deploy config: server.jsonc |
| `configs/consumer.jsonc` | Deploy config: consumer.jsonc |
| `configs/validator.jsonc` | Deploy config: validator.jsonc |
| `configs/emulator.jsonc` | Deploy config: emulator.jsonc |
| `configs/configctl.jsonc` | Deploy config: configctl.jsonc |
| `logs/nats.log` | Logs: nats |
| `logs/kafka.log` | Logs: kafka |
| `logs/configctl.log` | Logs: configctl |
| `logs/server.log` | Logs: server |
| `logs/consumer.log` | Logs: consumer |
| `logs/validator.log` | Logs: validator |
| `logs/emulator.log` | Logs: emulator |

## How to use

1. Check `compose-status.txt` for service health overview
2. Review `healthz.json` and `readyz.json` for API readiness
3. Inspect `active-config.json` for the running configuration
4. Check `ingestion-bindings.json` for active data routing
5. Review `validation-results.json` for recent pass/fail outcomes
6. Examine `logs/` for per-service console output
7. Compare `configs/` with active runtime to spot drift
