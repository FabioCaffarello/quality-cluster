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
	validateConfig handlersValidateConfigUseCase,
	compileConfig handlersCompileConfigUseCase,
	activateConfig handlersActivateConfigUseCase,
) []webserver.Route {
	handler := handlers.NewConfigctlWebHandler(
		createDraft,
		getConfig,
		getActive,
		listConfigs,
		validateDraft,
		validateConfig,
		compileConfig,
		activateConfig,
	)

	return []webserver.Route{
		{
			Method:  http.MethodPost,
			Path:    "/configctl/configs",
			Handler: handler.CreateDraft,
		},
		{
			Method:  http.MethodGet,
			Path:    "/configctl/config-versions",
			Handler: handler.ListConfigs,
		},
		{
			Method:  http.MethodGet,
			Path:    "/configctl/config-versions/:id",
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
		{
			Method:  http.MethodPost,
			Path:    "/configctl/config-versions/:id/validate",
			Handler: handler.ValidateConfig,
		},
		{
			Method:  http.MethodPost,
			Path:    "/configctl/config-versions/:id/compile",
			Handler: handler.CompileConfig,
		},
		{
			Method:  http.MethodPost,
			Path:    "/configctl/config-versions/:id/activate",
			Handler: handler.ActivateConfig,
		},
	}
}
