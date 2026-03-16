package main

import (
	actorcommon "internal/actors/common"
	actorserver "internal/actors/scopes/server"
	configctlclient "internal/application/configctlclient"
	validatorincidentsclient "internal/application/validatorincidentsclient"
	validatorresultsclient "internal/application/validatorresultsclient"
	validatorruntimeclient "internal/application/validatorruntimeclient"
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
	listActiveRuntimeProjectionsUseCase := configctlclient.NewListActiveRuntimeProjectionsUseCase(gateway)
	listActiveIngestionBindingsUseCase := configctlclient.NewListActiveIngestionBindingsUseCase(gateway)
	listConfigsUseCase := configctlclient.NewListConfigsUseCase(gateway)
	validateDraftUseCase := configctlclient.NewValidateDraftUseCase(gateway)
	validateConfigUseCase := configctlclient.NewValidateConfigUseCase(gateway)
	compileConfigUseCase := configctlclient.NewCompileConfigUseCase(gateway)
	activateConfigUseCase := configctlclient.NewActivateConfigUseCase(gateway)

	runtimeGateway, runtimeCloseFn, runtimeProb := newValidatorRuntimeGateway(config)
	if runtimeProb != nil {
		logger.Error("create validator runtime gateway", "error", runtimeProb)
		os.Exit(1)
	}
	if runtimeCloseFn != nil {
		defer func() {
			if err := runtimeCloseFn(); err != nil {
				logger.Error("close validator runtime gateway", "error", err)
			}
		}()
	}
	getValidatorRuntimeUseCase := validatorruntimeclient.NewGetActiveRuntimeUseCase(runtimeGateway)

	resultsGateway, resultsCloseFn, resultsProb := newValidatorResultsGateway(config)
	if resultsProb != nil {
		logger.Error("create validator results gateway", "error", resultsProb)
		os.Exit(1)
	}
	if resultsCloseFn != nil {
		defer func() {
			if err := resultsCloseFn(); err != nil {
				logger.Error("close validator results gateway", "error", err)
			}
		}()
	}
	listValidationResultsUseCase := validatorresultsclient.NewListValidationResultsUseCase(resultsGateway)

	incidentsGateway, incidentsCloseFn, incidentsProb := newValidatorIncidentsGateway(config)
	if incidentsProb != nil {
		logger.Error("create validator incidents gateway", "error", incidentsProb)
		os.Exit(1)
	}
	if incidentsCloseFn != nil {
		defer func() {
			if err := incidentsCloseFn(); err != nil {
				logger.Error("close validator incidents gateway", "error", err)
			}
		}()
	}
	listValidationIncidentsUseCase := validatorincidentsclient.NewListValidationIncidentsUseCase(incidentsGateway)

	serverRoutes := routes.DefaultRoutes(routes.Dependencies{
		Readiness:                    newServerReadinessChecker(config, gateway, runtimeGateway, resultsGateway),
		CreateDraft:                  createDraftUseCase,
		GetConfig:                    getConfigUseCase,
		GetActive:                    getActiveUseCase,
		ListActiveRuntimeProjections: listActiveRuntimeProjectionsUseCase,
		ListActiveIngestionBindings:  listActiveIngestionBindingsUseCase,
		ListConfigs:                  listConfigsUseCase,
		ValidateDraft:                validateDraftUseCase,
		ValidateConfig:               validateConfigUseCase,
		CompileConfig:                compileConfigUseCase,
		ActivateConfig:               activateConfigUseCase,
		GetRuntime:                   getValidatorRuntimeUseCase,
		ListValidationResults:        listValidationResultsUseCase,
		ListValidationIncidents:      listValidationIncidentsUseCase,
	})

	pid := engine.Spawn(actorserver.NewServer(config.HTTP, serverRoutes), "server")
	actorcommon.WaitTillShutdown(engine, pid)
}
