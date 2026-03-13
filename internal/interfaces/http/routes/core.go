package routes

import (
	"context"
	"net/http"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
	"internal/shared/problem"
)

type Dependencies struct {
	Readiness     handlers.ReadinessChecker
	CreateDraft   handlersCreateDraftUseCase
	GetConfig     handlersGetConfigUseCase
	GetActive     handlersGetActiveConfigUseCase
	ListConfigs   handlersListConfigsUseCase
	ValidateDraft handlersValidateDraftUseCase
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

type handlersValidateDraftUseCase interface {
	Execute(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem)
}

func DefaultRoutes(deps Dependencies) []webserver.Route {
	readiness := deps.Readiness
	if readiness == nil {
		readiness = handlers.NewAlwaysReadyChecker()
	}

	routes := Core(readiness)
	routes = append(routes, Configctl(deps.CreateDraft, deps.GetConfig, deps.GetActive, deps.ListConfigs, deps.ValidateDraft)...)
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
