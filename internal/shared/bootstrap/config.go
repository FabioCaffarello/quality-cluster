package bootstrap

import (
	"log/slog"

	"internal/shared/problem"
	"internal/shared/settings"
)

// Runtime groups the shared process-level dependencies built during startup.
type Runtime struct {
	Config settings.AppConfig
	Logger *slog.Logger
}

// LoadAndValidate loads config from disk, applies defaults and validates the result.
func LoadAndValidate(path string) (settings.AppConfig, *problem.Problem) {
	cfg, prob := settings.Load(path)
	if prob != nil {
		return settings.AppConfig{}, prob
	}
	if prob = cfg.Validate(); prob != nil {
		return settings.AppConfig{}, prob
	}
	return cfg, nil
}

// Initialize builds the shared runtime for a process and installs the logger as slog default.
func Initialize(path string) (*Runtime, *problem.Problem) {
	cfg, prob := LoadAndValidate(path)
	if prob != nil {
		return nil, prob
	}

	logger := BuildLogger(cfg.Log)
	slog.SetDefault(logger)

	return &Runtime{
		Config: cfg,
		Logger: logger,
	}, nil
}
