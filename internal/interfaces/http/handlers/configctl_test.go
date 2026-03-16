package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/julienschmidt/httprouter"
)

type createDraftUseCaseSpy struct {
	command configctlcontracts.CreateDraftCommand
	reply   configctlcontracts.CreateDraftReply
	prob    *problem.Problem
}

func (s *createDraftUseCaseSpy) Execute(ctx context.Context, command configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem) {
	s.command = command
	if got := requestctx.CorrelationID(ctx); got != "corr-123" {
		return configctlcontracts.CreateDraftReply{}, problem.New(problem.Internal, "missing correlation id")
	}
	return s.reply, s.prob
}

type getConfigUseCaseSpy struct {
	query configctlcontracts.GetConfigQuery
	reply configctlcontracts.GetConfigReply
	prob  *problem.Problem
}

func (s *getConfigUseCaseSpy) Execute(_ context.Context, query configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

type getActiveConfigUseCaseSpy struct {
	query configctlcontracts.GetActiveConfigQuery
	reply configctlcontracts.GetActiveConfigReply
	prob  *problem.Problem
}

func (s *getActiveConfigUseCaseSpy) Execute(_ context.Context, query configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

type listConfigsUseCaseSpy struct {
	reply configctlcontracts.ListConfigsReply
	prob  *problem.Problem
}

func (s *listConfigsUseCaseSpy) Execute(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem) {
	return s.reply, s.prob
}

type validateDraftUseCaseSpy struct {
	command configctlcontracts.ValidateDraftCommand
	reply   configctlcontracts.ValidateDraftReply
	prob    *problem.Problem
}

func (s *validateDraftUseCaseSpy) Execute(_ context.Context, command configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem) {
	s.command = command
	return s.reply, s.prob
}

type validateConfigUseCaseSpy struct {
	command configctlcontracts.ValidateConfigCommand
	reply   configctlcontracts.ValidateConfigReply
	prob    *problem.Problem
}

func (s *validateConfigUseCaseSpy) Execute(_ context.Context, command configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem) {
	s.command = command
	return s.reply, s.prob
}

type compileConfigUseCaseSpy struct {
	command configctlcontracts.CompileConfigCommand
	reply   configctlcontracts.CompileConfigReply
	prob    *problem.Problem
}

func (s *compileConfigUseCaseSpy) Execute(_ context.Context, command configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem) {
	s.command = command
	return s.reply, s.prob
}

type activateConfigUseCaseSpy struct {
	command configctlcontracts.ActivateConfigCommand
	reply   configctlcontracts.ActivateConfigReply
	prob    *problem.Problem
}

func (s *activateConfigUseCaseSpy) Execute(_ context.Context, command configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem) {
	s.command = command
	return s.reply, s.prob
}

func TestConfigctlCreateDraft(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(
		&createDraftUseCaseSpy{
			reply: configctlcontracts.CreateDraftReply{
				Config: configctlcontracts.ConfigVersionDetail{
					ID:          "cfg-123",
					ConfigSetID: "set-123",
					ConfigKey:   "core",
					Format:      "json",
					Source:      "{}",
					Lifecycle:   "draft",
					CreatedAt:   time.Unix(10, 0).UTC(),
					UpdatedAt:   time.Unix(10, 0).UTC(),
				},
			},
		},
		nil, nil, nil, nil, nil, nil, nil,
	)

	req := httptest.NewRequest(http.MethodPost, "/configctl/configs", strings.NewReader(`{"name":"core","format":"json","content":"{}"}`))
	req.Header.Set("X-Correlation-ID", "corr-123")
	rec := httptest.NewRecorder()

	handler.CreateDraft(rec, req)

	if rec.Code != http.StatusCreated {
		t.Fatalf("expected status %d, got %d", http.StatusCreated, rec.Code)
	}

	var response createDraftResponse
	if err := json.NewDecoder(rec.Body).Decode(&response); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	if response.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", response.Config.ID)
	}
}

func TestConfigctlGetConfig(t *testing.T) {
	t.Parallel()

	getSpy := &getConfigUseCaseSpy{
		reply: configctlcontracts.GetConfigReply{
			Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123"},
		},
	}
	handler := NewConfigctlWebHandler(nil, getSpy, nil, nil, nil, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/config-versions/cfg-123", nil)
	req = req.WithContext(context.WithValue(req.Context(), httprouter.ParamsKey, httprouter.Params{{Key: "id", Value: "cfg-123"}}))
	rec := httptest.NewRecorder()

	handler.GetConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if getSpy.query.VersionID != "cfg-123" {
		t.Fatalf("expected query id %q, got %q", "cfg-123", getSpy.query.VersionID)
	}
}

func TestConfigctlGetActiveConfigPassesScopeQuery(t *testing.T) {
	t.Parallel()

	getSpy := &getActiveConfigUseCaseSpy{
		reply: configctlcontracts.GetActiveConfigReply{
			Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123"},
		},
	}
	handler := NewConfigctlWebHandler(nil, nil, getSpy, nil, nil, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/configs/active?scope_kind=tenant&scope_key=br", nil)
	rec := httptest.NewRecorder()

	handler.GetActiveConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if getSpy.query.ScopeKind != "tenant" || getSpy.query.ScopeKey != "br" {
		t.Fatalf("expected scope tenant/br, got %+v", getSpy.query)
	}
}

func TestConfigctlListConfigs(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(nil, nil, nil, &listConfigsUseCaseSpy{
		reply: configctlcontracts.ListConfigsReply{
			Configs: []configctlcontracts.ConfigVersionSummary{{ID: "cfg-123"}},
		},
	}, nil, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/configs", nil)
	rec := httptest.NewRecorder()

	handler.ListConfigs(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if strings.Contains(rec.Body.String(), "\"source\"") {
		t.Fatalf("expected summarized list payload, got %s", rec.Body.String())
	}
}

func TestConfigctlValidateDraft(t *testing.T) {
	t.Parallel()

	validateSpy := &validateDraftUseCaseSpy{
		reply: configctlcontracts.ValidateDraftReply{Valid: true},
	}
	handler := NewConfigctlWebHandler(nil, nil, nil, nil, validateSpy, nil, nil, nil)

	req := httptest.NewRequest(http.MethodPost, "/configctl/configs/validate", strings.NewReader(`{"format":"json","content":"{}"}`))
	rec := httptest.NewRecorder()

	handler.ValidateDraft(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if validateSpy.command.Format != "json" {
		t.Fatalf("expected format %q, got %q", "json", validateSpy.command.Format)
	}
}

func TestConfigctlValidateConfig(t *testing.T) {
	t.Parallel()

	validateSpy := &validateConfigUseCaseSpy{
		reply: configctlcontracts.ValidateConfigReply{Valid: true},
	}
	handler := NewConfigctlWebHandler(nil, nil, nil, nil, nil, validateSpy, nil, nil)

	req := httptest.NewRequest(http.MethodPost, "/configctl/config-versions/cfg-123/validate", nil)
	req = req.WithContext(context.WithValue(req.Context(), httprouter.ParamsKey, httprouter.Params{{Key: "id", Value: "cfg-123"}}))
	rec := httptest.NewRecorder()

	handler.ValidateConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if validateSpy.command.VersionID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", validateSpy.command.VersionID)
	}
}

func TestConfigctlCompileConfig(t *testing.T) {
	t.Parallel()

	compileSpy := &compileConfigUseCaseSpy{
		reply: configctlcontracts.CompileConfigReply{
			Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "compiled"},
		},
	}
	handler := NewConfigctlWebHandler(nil, nil, nil, nil, nil, nil, compileSpy, nil)

	req := httptest.NewRequest(http.MethodPost, "/configctl/config-versions/cfg-123/compile", strings.NewReader(`{"runtime_loader":"validator:v2"}`))
	req = req.WithContext(context.WithValue(req.Context(), httprouter.ParamsKey, httprouter.Params{{Key: "id", Value: "cfg-123"}}))
	rec := httptest.NewRecorder()

	handler.CompileConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if compileSpy.command.VersionID != "cfg-123" || compileSpy.command.RuntimeLoader != "validator:v2" {
		t.Fatalf("unexpected compile command: %+v", compileSpy.command)
	}
}

func TestConfigctlActivateConfig(t *testing.T) {
	t.Parallel()

	activateSpy := &activateConfigUseCaseSpy{
		reply: configctlcontracts.ActivateConfigReply{
			Config: configctlcontracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "active"},
		},
	}
	handler := NewConfigctlWebHandler(nil, nil, nil, nil, nil, nil, nil, activateSpy)

	req := httptest.NewRequest(http.MethodPost, "/configctl/config-versions/cfg-123/activate", strings.NewReader(`{"scope_kind":"tenant","scope_key":"br"}`))
	req = req.WithContext(context.WithValue(req.Context(), httprouter.ParamsKey, httprouter.Params{{Key: "id", Value: "cfg-123"}}))
	rec := httptest.NewRecorder()

	handler.ActivateConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if activateSpy.command.VersionID != "cfg-123" || activateSpy.command.ScopeKind != "tenant" || activateSpy.command.ScopeKey != "br" {
		t.Fatalf("unexpected activate command: %+v", activateSpy.command)
	}
}

func TestConfigctlMapsProblemResponses(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(nil, &getConfigUseCaseSpy{
		prob: problem.New(problem.NotFound, "config not found"),
	}, nil, nil, nil, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/config-versions/cfg-404", nil)
	req = req.WithContext(context.WithValue(req.Context(), httprouter.ParamsKey, httprouter.Params{{Key: "id", Value: "cfg-404"}}))
	rec := httptest.NewRecorder()

	handler.GetConfig(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Fatalf("expected status %d, got %d", http.StatusNotFound, rec.Code)
	}
}
