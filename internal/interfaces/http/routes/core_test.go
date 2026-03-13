package routes

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"internal/interfaces/http/handlers"

	"github.com/julienschmidt/httprouter"
)

func TestCoreRoutes(t *testing.T) {
	t.Parallel()

	router := httprouter.New()
	for _, route := range Core(handlers.ReadinessCheckerFunc(func(context.Context) error {
		return errors.New("not ready")
	})) {
		router.HandlerFunc(route.Method, route.Path, route.Handler)
	}

	healthReq := httptest.NewRequest(http.MethodGet, "/healthz", nil)
	healthRec := httptest.NewRecorder()
	router.ServeHTTP(healthRec, healthReq)

	if healthRec.Code != http.StatusOK {
		t.Fatalf("expected /healthz to return %d, got %d", http.StatusOK, healthRec.Code)
	}

	readyReq := httptest.NewRequest(http.MethodGet, "/readyz", nil)
	readyRec := httptest.NewRecorder()
	router.ServeHTTP(readyRec, readyReq)

	if readyRec.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected /readyz to return %d, got %d", http.StatusServiceUnavailable, readyRec.Code)
	}
}
