package bootstrap

import (
	"io"
	"log/slog"
	"os"
	"strings"

	"internal/shared/settings"
)

// BuildLogger creates a structured *slog.Logger from the given LogConfig.
func BuildLogger(cfg settings.LogConfig) *slog.Logger {
	return newLogger(cfg, os.Stdout)
}

func newLogger(cfg settings.LogConfig, writer io.Writer) *slog.Logger {
	var level slog.Level
	if err := level.UnmarshalText([]byte(strings.ToLower(string(cfg.Level)))); err != nil {
		level = slog.LevelInfo
	}

	options := &slog.HandlerOptions{Level: level}
	if strings.EqualFold(string(cfg.Format), string(settings.LogFormatJSON)) {
		return slog.New(slog.NewJSONHandler(writer, options))
	}

	return slog.New(slog.NewTextHandler(writer, options))
}
