package routes

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"

	"github.com/julienschmidt/httprouter"
)

type createDraftUseCaseStub struct{}

func (createDraftUseCaseStub) Execute(_ context.Context, _ configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem) {
	return configctlcontracts.CreateDraftReply{
		Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123"},
	}, nil
}

type getConfigUseCaseStub struct{}

func (getConfigUseCaseStub) Execute(_ context.Context, _ configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem) {
	return configctlcontracts.GetConfigReply{
		Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123"},
	}, nil
}

type getActiveUseCaseStub struct{}

func (getActiveUseCaseStub) Execute(_ context.Context, _ configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
	return configctlcontracts.GetActiveConfigReply{
		Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123"},
	}, nil
}

type listConfigsUseCaseStub struct{}

func (listConfigsUseCaseStub) Execute(_ context.Context, _ configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem) {
	return configctlcontracts.ListConfigsReply{
		Configs: []configctlcontracts.ConfigVersionSummary{{ID: "cfg-123"}},
	}, nil
}

type validateDraftUseCaseStub struct{}

func (validateDraftUseCaseStub) Execute(_ context.Context, _ configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem) {
	return configctlcontracts.ValidateDraftReply{Valid: true}, nil
}

type validateConfigUseCaseStub struct{}

func (validateConfigUseCaseStub) Execute(_ context.Context, _ configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem) {
	return configctlcontracts.ValidateConfigReply{Valid: true}, nil
}

type compileConfigUseCaseStub struct{}

func (compileConfigUseCaseStub) Execute(_ context.Context, _ configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem) {
	return configctlcontracts.CompileConfigReply{
		Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "compiled"},
	}, nil
}

type activateConfigUseCaseStub struct{}

func (activateConfigUseCaseStub) Execute(_ context.Context, _ configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem) {
	return configctlcontracts.ActivateConfigReply{
		Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "active"},
	}, nil
}

func TestConfigctlRoutesRegisterHandlers(t *testing.T) {
	t.Parallel()

	routes := Configctl(
		createDraftUseCaseStub{},
		getConfigUseCaseStub{},
		getActiveUseCaseStub{},
		listConfigsUseCaseStub{},
		validateDraftUseCaseStub{},
		validateConfigUseCaseStub{},
		compileConfigUseCaseStub{},
		activateConfigUseCaseStub{},
	)
	router := httprouter.New()
	for _, route := range routes {
		router.HandlerFunc(route.Method, route.Path, route.Handler)
	}

	tests := []struct {
		method string
		path   string
		body   string
		code   int
	}{
		{method: http.MethodPost, path: "/configctl/configs", body: `{"name":"core","format":"json","content":"{}"}`, code: http.StatusCreated},
		{method: http.MethodGet, path: "/configctl/configs", code: http.StatusOK},
		{method: http.MethodGet, path: "/configctl/config-versions", code: http.StatusOK},
		{method: http.MethodGet, path: "/configctl/configs/by-id?id=cfg-123", code: http.StatusOK},
		{method: http.MethodGet, path: "/configctl/config-versions/cfg-123", code: http.StatusOK},
		{method: http.MethodGet, path: "/configctl/configs/active", code: http.StatusOK},
		{method: http.MethodGet, path: "/configctl/active-config", code: http.StatusOK},
		{method: http.MethodPost, path: "/configctl/configs/validate", body: `{"format":"json","content":"{}"}`, code: http.StatusOK},
		{method: http.MethodPost, path: "/configctl/config-versions/cfg-123/validate", code: http.StatusOK},
		{method: http.MethodPost, path: "/configctl/config-versions/cfg-123/compile", body: `{}`, code: http.StatusOK},
		{method: http.MethodPost, path: "/configctl/config-versions/cfg-123/activate", body: `{"scope_kind":"global","scope_key":"default"}`, code: http.StatusOK},
	}

	for _, tt := range tests {
		req := httptest.NewRequest(tt.method, tt.path, strings.NewReader(tt.body))
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, req)
		if rec.Code != tt.code {
			t.Fatalf("%s %s: expected status %d, got %d", tt.method, tt.path, tt.code, rec.Code)
		}
	}
}
