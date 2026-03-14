package routes

import (
	"net/http"

	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
)

func Runtime(getActive handlersGetValidatorRuntimeUseCase, listActiveIngestionBindings handlersListActiveIngestionBindingsUseCase) []webserver.Route {
	return RuntimeWithValidationResults(getActive, listActiveIngestionBindings, nil)
}

func RuntimeWithValidationResults(getActive handlersGetValidatorRuntimeUseCase, listActiveIngestionBindings handlersListActiveIngestionBindingsUseCase, listValidationResults handlersListValidationResultsUseCase) []webserver.Route {
	handler := handlers.NewRuntimeWebHandler(getActive, listActiveIngestionBindings, listValidationResults)

	return []webserver.Route{
		{
			Method:  http.MethodGet,
			Path:    "/runtime/validator/active",
			Handler: handler.GetActiveValidatorRuntime,
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
