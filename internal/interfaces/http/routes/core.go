package routes

import (
	"net/http"

	"internal/interfaces/http/handlers"
	"internal/interfaces/http/webserver"
)

func DefaultRoutes() []webserver.Route {
	coreRoutes := Core(handlers.NewAlwaysReadyChecker())
	return coreRoutes
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
