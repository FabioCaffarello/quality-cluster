package routes

import (
	"net/http"

	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
)

func Configctl(
	createDraft handlersCreateDraftUseCase,
	getConfig handlersGetConfigUseCase,
	getActive handlersGetActiveConfigUseCase,
	listConfigs handlersListConfigsUseCase,
	validateDraft handlersValidateDraftUseCase,
) []webserver.Route {
	handler := handlers.NewConfigctlWebHandler(createDraft, getConfig, getActive, listConfigs, validateDraft)

	return []webserver.Route{
		{
			Method:  http.MethodPost,
			Path:    "/configctl/configs",
			Handler: handler.CreateDraft,
		},
		{
			Method:  http.MethodGet,
			Path:    "/configctl/configs",
			Handler: handler.ListConfigs,
		},
		{
			Method:  http.MethodGet,
			Path:    "/configctl/configs/by-id",
			Handler: handler.GetConfig,
		},
		{
			Method:  http.MethodGet,
			Path:    "/configctl/configs/active",
			Handler: handler.GetActiveConfig,
		},
		{
			Method:  http.MethodPost,
			Path:    "/configctl/configs/validate",
			Handler: handler.ValidateDraft,
		},
	}
}
