package main

import (
	actorcommon "internal/actors/common"
	actorserver "internal/actors/scopes/server"
	configctlclient "internal/application/configctlclient"
	"internal/interfaces/http/routes"
	"internal/shared/bootstrap"
	"internal/shared/settings"
	"log/slog"
	"os"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	logger.Info("server starting", "addr", config.HTTP.Addr)
	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		logger.Error("create actor engine", "error", err)
		os.Exit(1)
	}

	gateway, closeFn, prob := newConfigctlGateway(config)
	if prob != nil {
		logger.Error("create configctl gateway", "error", prob)
		os.Exit(1)
	}
	if closeFn != nil {
		defer func() {
			if err := closeFn(); err != nil {
				logger.Error("close configctl gateway", "error", err)
			}
		}()
	}

	createDraftUseCase := configctlclient.NewCreateDraftUseCase(gateway)
	getConfigUseCase := configctlclient.NewGetConfigUseCase(gateway)
	getActiveUseCase := configctlclient.NewGetActiveConfigUseCase(gateway)
	listConfigsUseCase := configctlclient.NewListConfigsUseCase(gateway)
	validateDraftUseCase := configctlclient.NewValidateDraftUseCase(gateway)
	serverRoutes := routes.DefaultRoutes(routes.Dependencies{
		Readiness:     newConfigctlReadinessChecker(gateway),
		CreateDraft:   createDraftUseCase,
		GetConfig:     getConfigUseCase,
		GetActive:     getActiveUseCase,
		ListConfigs:   listConfigsUseCase,
		ValidateDraft: validateDraftUseCase,
	})

	pid := engine.Spawn(actorserver.NewServer(config.HTTP, serverRoutes), "server")
	actorcommon.WaitTillShutdown(engine, pid)
}
