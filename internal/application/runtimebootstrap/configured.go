package runtimebootstrap

import (
	"context"
	"log/slog"
	"strings"

	"internal/shared/problem"
	"internal/shared/settings"
)

func WaitForConfiguredActiveIngestionBootstrap(ctx context.Context, logger *slog.Logger, config settings.AppConfig, source string) (ActiveIngestionBootstrap, *problem.Problem) {
	baseURL := strings.TrimSpace(config.Bootstrap.BaseURL)
	if baseURL == "" {
		return ActiveIngestionBootstrap{}, problem.New(problem.InvalidArgument, "bootstrap.base_url must not be empty")
	}

	client := NewClient(baseURL, config.Bootstrap.TimeoutDuration())
	return client.WaitForActiveIngestionBootstrap(ctx, logger, WaitOptions{
		ScopeKind:     config.Bootstrap.ScopeKind,
		ScopeKey:      config.Bootstrap.ScopeKey,
		CorrelationID: strings.TrimSpace(source) + ".bootstrap",
	})
}
