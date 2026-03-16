package settings

import (
	"os"
	"path/filepath"
	"testing"

	"internal/shared/problem"
)

func TestDefaultsProduceValidConfig(t *testing.T) {
	cfg := Defaults()

	if prob := cfg.Validate(); prob != nil {
		t.Fatalf("expected defaults to be valid, got %v", prob)
	}
}

func TestLoadSupportsJSONCAndDefaults(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	content := `{
		// Override only the log level.
		"log": {
			"level": "debug"
		},
		/* Keep the remaining defaults. */
		"nats": {
			"enabled": true,
			"url": "nats://localhost:4222"
		}
	}`

	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	cfg, prob := Load(path)
	if prob != nil {
		t.Fatalf("expected config to load, got %v", prob)
	}

	if cfg.Log.Level != LogLevelDebug {
		t.Fatalf("expected overridden log level, got %q", cfg.Log.Level)
	}
	if cfg.Log.Format != LogFormatText {
		t.Fatalf("expected missing field to keep default, got %q", cfg.Log.Format)
	}
	if cfg.HTTP.Addr != ":8080" {
		t.Fatalf("expected default http addr, got %q", cfg.HTTP.Addr)
	}
}

func TestLoadSupportsDataPlaneSections(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	content := `{
		"kafka": {
			"enabled": true,
			"brokers": ["kafka:9092"]
		},
		"bootstrap": {
			"base_url": "http://server:8080"
		},
		"emulator": {
			"publish_interval": "7s"
		}
	}`

	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	cfg, prob := Load(path)
	if prob != nil {
		t.Fatalf("expected config to load, got %v", prob)
	}
	if len(cfg.Kafka.Brokers) != 1 || cfg.Bootstrap.ScopeKind != "global" {
		t.Fatalf("expected data plane defaults to apply, got %+v", cfg)
	}
	if cfg.Emulator.PublishInterval != "7s" {
		t.Fatalf("expected emulator config to load, got %q", cfg.Emulator.PublishInterval)
	}
	if cfg.Bootstrap.ReconcileInterval != "30s" {
		t.Fatalf("expected bootstrap reconcile interval default, got %q", cfg.Bootstrap.ReconcileInterval)
	}
}

func TestLoadRejectsUnknownFields(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.json")

	if err := os.WriteFile(path, []byte(`{"unknown": true}`), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	_, prob := Load(path)
	if prob == nil {
		t.Fatalf("expected parse error")
	}
	if prob.Code != cfgParseError {
		t.Fatalf("expected parse error code, got %q", prob.Code)
	}
}

func TestValidateAggregatesIssues(t *testing.T) {
	cfg := Defaults()
	cfg.Log.Level = "verbose"
	cfg.HTTP.ReadTimeout = "nope"
	cfg.NATS.Enabled = true
	cfg.NATS.URL = ""

	prob := cfg.Validate()
	if prob == nil {
		t.Fatalf("expected config validation to fail")
	}
	if prob.Code != cfgInvalid {
		t.Fatalf("expected config invalid code, got %q", prob.Code)
	}

	rawIssues := prob.Details[problem.DetailIssues]
	issues, ok := rawIssues.([]problem.ValidationIssue)
	if !ok {
		t.Fatalf("expected typed validation issues, got %#v", rawIssues)
	}

	if len(issues) != 3 {
		t.Fatalf("expected aggregated issues, got %d", len(issues))
	}
}
