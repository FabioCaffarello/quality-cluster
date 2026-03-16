package routes

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"

	configctlcontracts "internal/application/configctl/contracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"

	"github.com/julienschmidt/httprouter"
)

type runtimeUseCaseStub struct{}

func (runtimeUseCaseStub) Execute(context.Context, runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	return runtimecontracts.GetActiveRuntimeReply{}, nil
}

type ingestionBindingsUseCaseStub struct{}

func (ingestionBindingsUseCaseStub) Execute(context.Context, configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	return configctlcontracts.ListActiveIngestionBindingsReply{}, nil
}

type runtimeProjectionsUseCaseStub struct{}

func (runtimeProjectionsUseCaseStub) Execute(context.Context, configctlcontracts.ListActiveRuntimeProjectionsQuery) (configctlcontracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	return configctlcontracts.ListActiveRuntimeProjectionsReply{}, nil
}

type validationResultsUseCaseStub struct{}

func (validationResultsUseCaseStub) Execute(context.Context, validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	return validatorresultscontracts.ListValidationResultsReply{}, nil
}

func TestRuntimeRoutesRegisterHandlers(t *testing.T) {
	t.Parallel()

	router := httprouter.New()
	for _, route := range RuntimeWithValidationResults(runtimeUseCaseStub{}, runtimeProjectionsUseCaseStub{}, ingestionBindingsUseCaseStub{}, validationResultsUseCaseStub{}) {
		router.HandlerFunc(route.Method, route.Path, route.Handler)
	}

	req := httptest.NewRequest(http.MethodGet, "/runtime/validator/active", nil)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/runtime/configctl/projections", nil)
	rec = httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/runtime/ingestion/bindings", nil)
	rec = httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/runtime/validator/results", nil)
	rec = httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
}
