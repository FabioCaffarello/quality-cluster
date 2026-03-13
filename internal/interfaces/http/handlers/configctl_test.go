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
	reply configctlcontracts.GetActiveConfigReply
	prob  *problem.Problem
}

func (s *getActiveConfigUseCaseSpy) Execute(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
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

func TestConfigctlCreateDraft(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(
		&createDraftUseCaseSpy{
			reply: configctlcontracts.CreateDraftReply{
				Config: configctlcontracts.ConfigRecord{
					ID:        "cfg-123",
					Name:      "core",
					Format:    "json",
					Content:   "{}",
					Status:    "draft",
					CreatedAt: time.Unix(10, 0).UTC(),
					UpdatedAt: time.Unix(10, 0).UTC(),
				},
			},
		},
		nil, nil, nil, nil,
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
			Config: configctlcontracts.ConfigRecord{ID: "cfg-123"},
		},
	}
	handler := NewConfigctlWebHandler(nil, getSpy, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/configs/by-id?id=cfg-123", nil)
	rec := httptest.NewRecorder()

	handler.GetConfig(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if getSpy.query.ID != "cfg-123" {
		t.Fatalf("expected query id %q, got %q", "cfg-123", getSpy.query.ID)
	}
}

func TestConfigctlListConfigs(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(nil, nil, nil, &listConfigsUseCaseSpy{
		reply: configctlcontracts.ListConfigsReply{
			Configs: []configctlcontracts.ConfigRecord{{ID: "cfg-123"}},
		},
	}, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/configs", nil)
	rec := httptest.NewRecorder()

	handler.ListConfigs(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
}

func TestConfigctlValidateDraft(t *testing.T) {
	t.Parallel()

	validateSpy := &validateDraftUseCaseSpy{
		reply: configctlcontracts.ValidateDraftReply{Valid: true},
	}
	handler := NewConfigctlWebHandler(nil, nil, nil, nil, validateSpy)

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

func TestConfigctlMapsProblemResponses(t *testing.T) {
	t.Parallel()

	handler := NewConfigctlWebHandler(nil, &getConfigUseCaseSpy{
		prob: problem.New(problem.NotFound, "config not found"),
	}, nil, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/configctl/configs/by-id?id=cfg-404", nil)
	rec := httptest.NewRecorder()

	handler.GetConfig(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Fatalf("expected status %d, got %d", http.StatusNotFound, rec.Code)
	}
}
