package runtimebootstrap

import (
	"context"
	"log/slog"
	"strings"

	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"
	"internal/shared/settings"
)

func WaitForConfiguredActiveIngestionBootstrap(ctx context.Context, logger *slog.Logger, config settings.AppConfig, source string) (ActiveIngestionBootstrap, *problem.Problem) {
	return WaitForConfiguredActiveIngestionBootstrapWithRegistry(ctx, logger, config, source, dataplaneapp.DefaultRegistry())
}

func WaitForConfiguredActiveIngestionBootstrapWithRegistry(ctx context.Context, logger *slog.Logger, config settings.AppConfig, source string, registry dataplaneapp.Registry) (ActiveIngestionBootstrap, *problem.Problem) {
	baseURL := strings.TrimSpace(config.Bootstrap.BaseURL)
	if baseURL == "" {
		return ActiveIngestionBootstrap{}, problem.New(problem.InvalidArgument, "bootstrap.base_url must not be empty")
	}

	client := NewClientWithRegistry(baseURL, config.Bootstrap.TimeoutDuration(), registry)
	return client.WaitForActiveIngestionBootstrap(ctx, logger, WaitOptions{
		ScopeKind:     config.Bootstrap.ScopeKind,
		ScopeKey:      config.Bootstrap.ScopeKey,
		CorrelationID: strings.TrimSpace(source) + ".bootstrap",
	})
}

func WaitForConfiguredActiveIngestionBootstrapSet(ctx context.Context, logger *slog.Logger, config settings.AppConfig, source string) (ActiveIngestionBootstrap, *problem.Problem) {
	return WaitForConfiguredActiveIngestionBootstrapSetWithRegistry(ctx, logger, config, source, dataplaneapp.DefaultRegistry())
}

func WaitForConfiguredActiveIngestionBootstrapSetWithRegistry(ctx context.Context, logger *slog.Logger, config settings.AppConfig, source string, registry dataplaneapp.Registry) (ActiveIngestionBootstrap, *problem.Problem) {
	baseURL := strings.TrimSpace(config.Bootstrap.BaseURL)
	if baseURL == "" {
		return ActiveIngestionBootstrap{}, problem.New(problem.InvalidArgument, "bootstrap.base_url must not be empty")
	}

	client := NewClientWithRegistry(baseURL, config.Bootstrap.TimeoutDuration(), registry)
	return client.WaitForActiveIngestionBootstrapSet(ctx, logger, AggregateWaitOptions{
		CorrelationID: strings.TrimSpace(source) + ".bootstrap-set",
	})
}
