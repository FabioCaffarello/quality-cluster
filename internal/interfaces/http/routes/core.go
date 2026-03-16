package routes

import (
	"context"
	"net/http"

	configctlcontracts "internal/application/configctl/contracts"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
	"internal/shared/problem"
)

type Dependencies struct {
	Readiness                    handlers.ReadinessChecker
	CreateDraft                  handlersCreateDraftUseCase
	GetConfig                    handlersGetConfigUseCase
	GetActive                    handlersGetActiveConfigUseCase
	ListActiveRuntimeProjections handlersListActiveRuntimeProjectionsUseCase
	ListActiveIngestionBindings  handlersListActiveIngestionBindingsUseCase
	ListConfigs                  handlersListConfigsUseCase
	ValidateDraft                handlersValidateDraftUseCase
	ValidateConfig               handlersValidateConfigUseCase
	CompileConfig                handlersCompileConfigUseCase
	ActivateConfig               handlersActivateConfigUseCase
	GetRuntime                   handlersGetValidatorRuntimeUseCase
	ListValidationResults        handlersListValidationResultsUseCase
	ListValidationIncidents      handlersListValidationIncidentsUseCase
}

type handlersCreateDraftUseCase interface {
	Execute(context.Context, configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem)
}

type handlersGetConfigUseCase interface {
	Execute(context.Context, configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem)
}

type handlersGetActiveConfigUseCase interface {
	Execute(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem)
}

type handlersListConfigsUseCase interface {
	Execute(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem)
}

type handlersListActiveRuntimeProjectionsUseCase interface {
	Execute(context.Context, configctlcontracts.ListActiveRuntimeProjectionsQuery) (configctlcontracts.ListActiveRuntimeProjectionsReply, *problem.Problem)
}

type handlersListActiveIngestionBindingsUseCase interface {
	Execute(context.Context, configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem)
}

type handlersValidateDraftUseCase interface {
	Execute(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem)
}

type handlersValidateConfigUseCase interface {
	Execute(context.Context, configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem)
}

type handlersCompileConfigUseCase interface {
	Execute(context.Context, configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem)
}

type handlersActivateConfigUseCase interface {
	Execute(context.Context, configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem)
}

type handlersGetValidatorRuntimeUseCase interface {
	Execute(context.Context, runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem)
}

type handlersListValidationResultsUseCase interface {
	Execute(context.Context, validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem)
}

type handlersListValidationIncidentsUseCase interface {
	Execute(context.Context, validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem)
}

func DefaultRoutes(deps Dependencies) []webserver.Route {
	readiness := deps.Readiness
	if readiness == nil {
		readiness = handlers.NewAlwaysReadyChecker()
	}

	routes := Core(readiness)
	routes = append(routes, Configctl(
		deps.CreateDraft,
		deps.GetConfig,
		deps.GetActive,
		deps.ListConfigs,
		deps.ValidateDraft,
		deps.ValidateConfig,
		deps.CompileConfig,
		deps.ActivateConfig,
	)...)
	routes = append(routes, RuntimeWithValidationResults(deps.GetRuntime, deps.ListActiveRuntimeProjections, deps.ListActiveIngestionBindings, deps.ListValidationResults, deps.ListValidationIncidents)...)
	return routes
}

func Core(readiness handlers.ReadinessChecker) []webserver.Route {
	healthzHandler := handlers.NewHealthzWebHandler()
	readyzHandler := handlers.NewReadyzWebHandler(readiness)

	return []webserver.Route{
		{
			Method:  http.MethodGet,
			Path:    "/healthz",
			Handler: healthzHandler.Healthz,
		},
		{
			Method:  http.MethodGet,
			Path:    "/readyz",
			Handler: readyzHandler.Readyz,
		},
	}
}
