package bootstrap

import (
	"bytes"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"internal/shared/settings"
)

func TestNewLoggerUsesConfiguredFormat(t *testing.T) {
	buffer := &bytes.Buffer{}
	logger := newLogger(settings.LogConfig{
		Level:  settings.LogLevelInfo,
		Format: settings.LogFormatJSON,
	}, buffer)

	logger.Info("hello", slog.String("component", "test"))

	output := buffer.String()
	if !strings.Contains(output, `"msg":"hello"`) {
		t.Fatalf("expected json logger output, got %q", output)
	}
}

func TestInitializeBuildsRuntime(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.json")

	content := `{
		"log": {"format": "json"},
		"http": {"addr": ":9090"}
	}`

	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	runtime, prob := Initialize(path)
	if prob != nil {
		t.Fatalf("expected runtime initialization to succeed, got %v", prob)
	}
	if runtime == nil || runtime.Logger == nil {
		t.Fatalf("expected runtime logger to be built")
	}
	if runtime.Config.HTTP.Addr != ":9090" {
		t.Fatalf("expected config to be loaded, got %q", runtime.Config.HTTP.Addr)
	}
}
