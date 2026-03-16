package routes

import (
	"net/http"

	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
)

func Runtime(getActive handlersGetValidatorRuntimeUseCase, listActiveRuntimeProjections handlersListActiveRuntimeProjectionsUseCase, listActiveIngestionBindings handlersListActiveIngestionBindingsUseCase) []webserver.Route {
	return RuntimeWithValidationResults(getActive, listActiveRuntimeProjections, listActiveIngestionBindings, nil)
}

func RuntimeWithValidationResults(getActive handlersGetValidatorRuntimeUseCase, listActiveRuntimeProjections handlersListActiveRuntimeProjectionsUseCase, listActiveIngestionBindings handlersListActiveIngestionBindingsUseCase, listValidationResults handlersListValidationResultsUseCase) []webserver.Route {
	handler := handlers.NewRuntimeWebHandler(getActive, listActiveRuntimeProjections, listActiveIngestionBindings, listValidationResults)

	return []webserver.Route{
		{
			Method:  http.MethodGet,
			Path:    "/runtime/validator/active",
			Handler: handler.GetActiveValidatorRuntime,
		},
		{
			Method:  http.MethodGet,
			Path:    "/runtime/configctl/projections",
			Handler: handler.ListActiveRuntimeProjections,
		},
		{
			Method:  http.MethodGet,
			Path:    "/runtime/ingestion/bindings",
			Handler: handler.ListActiveIngestionBindings,
		},
		{
			Method:  http.MethodGet,
			Path:    "/runtime/validator/results",
			Handler: handler.ListValidationResults,
		},
	}
}
