package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type getValidatorRuntimeUseCaseSpy struct {
	query runtimecontracts.GetActiveRuntimeQuery
	reply runtimecontracts.GetActiveRuntimeReply
	prob  *problem.Problem
}

func (s *getValidatorRuntimeUseCaseSpy) Execute(_ context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

type listActiveIngestionBindingsUseCaseSpy struct {
	query configctlcontracts.ListActiveIngestionBindingsQuery
	reply configctlcontracts.ListActiveIngestionBindingsReply
	prob  *problem.Problem
}

func (s *listActiveIngestionBindingsUseCaseSpy) Execute(_ context.Context, query configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

type listValidationResultsUseCaseSpy struct {
	query validatorresultscontracts.ListValidationResultsQuery
	reply validatorresultscontracts.ListValidationResultsReply
	prob  *problem.Problem
}

func (s *listValidationResultsUseCaseSpy) Execute(_ context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

func TestGetActiveValidatorRuntime(t *testing.T) {
	t.Parallel()

	spy := &getValidatorRuntimeUseCaseSpy{
		reply: runtimecontracts.GetActiveRuntimeReply{
			Runtime: runtimecontracts.ActiveRuntimeRecord{
				RuntimeRecord: sharedruntime.RuntimeRecord{
					Config: sharedruntime.ConfigRecord{
						Key:       "core",
						VersionID: "cfg-123",
					},
					Artifact: sharedruntime.ArtifactRecord{
						ID:            "artifact-1",
						RuntimeLoader: "validator:v1",
					},
				},
				LoadedAt: time.Unix(10, 0).UTC(),
			},
		},
	}
	handler := NewRuntimeWebHandler(spy, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/runtime/validator/active?scope_kind=tenant&scope_key=br", nil)
	rec := httptest.NewRecorder()

	handler.GetActiveValidatorRuntime(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if spy.query.ScopeKind != "tenant" || spy.query.ScopeKey != "br" {
		t.Fatalf("unexpected runtime query: %+v", spy.query)
	}

	var body map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	runtimeBody := body["runtime"].(map[string]any)
	artifactBody := runtimeBody["artifact"].(map[string]any)
	if _, found := artifactBody["compiler_version"]; found {
		t.Fatalf("expected compact artifact payload, got %v", artifactBody)
	}
	if _, found := artifactBody["created_at"]; found {
		t.Fatalf("expected compact artifact payload, got %v", artifactBody)
	}
}

func TestGetActiveValidatorRuntimeMapsProblems(t *testing.T) {
	t.Parallel()

	handler := NewRuntimeWebHandler(&getValidatorRuntimeUseCaseSpy{
		prob: problem.New(problem.NotFound, "validator runtime is not loaded"),
	}, nil, nil)

	req := httptest.NewRequest(http.MethodGet, "/runtime/validator/active", nil)
	rec := httptest.NewRecorder()

	handler.GetActiveValidatorRuntime(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Fatalf("expected status %d, got %d", http.StatusNotFound, rec.Code)
	}
}

func TestListActiveIngestionBindings(t *testing.T) {
	t.Parallel()

	spy := &listActiveIngestionBindingsUseCaseSpy{
		reply: configctlcontracts.ListActiveIngestionBindingsReply{
			Bindings: []configctlcontracts.ActiveIngestionBindingRecord{{
				Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "orders.v1"},
				Fields: []configctlcontracts.FieldRecord{{
					Name:     "order_id",
					Type:     "string",
					Required: true,
				}},
				Runtime: sharedruntime.RuntimeRecord{
					Scope: sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
					Config: sharedruntime.ConfigRecord{
						SetID:              "set-1",
						Key:                "core",
						VersionID:          "cfg-123",
						Version:            3,
						DefinitionChecksum: "definition-1",
					},
					Artifact: sharedruntime.ArtifactRecord{
						ID:            "artifact-1",
						SchemaVersion: "runtime/v1",
						Checksum:      "artifact-checksum",
						RuntimeLoader: "validator:v1",
					},
					ActivatedAt: time.Unix(10, 0).UTC(),
				},
			}},
		},
	}
	handler := NewRuntimeWebHandler(nil, spy, nil)

	req := httptest.NewRequest(http.MethodGet, "/runtime/ingestion/bindings?scope_kind=tenant&scope_key=br", nil)
	rec := httptest.NewRecorder()

	handler.ListActiveIngestionBindings(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if spy.query.ScopeKind != "tenant" || spy.query.ScopeKey != "br" {
		t.Fatalf("unexpected ingestion runtime query: %+v", spy.query)
	}

	var body map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	bindings := body["bindings"].([]any)
	if len(bindings) != 1 {
		t.Fatalf("expected one binding, got %v", bindings)
	}
	first := bindings[0].(map[string]any)
	if _, ok := first["runtime"]; !ok {
		t.Fatalf("expected runtime envelope, got %v", first)
	}
	fields := first["fields"].([]any)
	if len(fields) != 1 {
		t.Fatalf("expected bootstrap fields to be exposed, got %v", first)
	}
}

func TestListActiveIngestionBindingsMapsProblems(t *testing.T) {
	t.Parallel()

	handler := NewRuntimeWebHandler(nil, &listActiveIngestionBindingsUseCaseSpy{
		prob: problem.New(problem.InvalidArgument, "ingestion bindings query is invalid"),
	}, nil)

	req := httptest.NewRequest(http.MethodGet, "/runtime/ingestion/bindings?scope_kind=tenant", nil)
	rec := httptest.NewRecorder()

	handler.ListActiveIngestionBindings(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected status %d, got %d", http.StatusBadRequest, rec.Code)
	}
}

func TestListValidationResults(t *testing.T) {
	t.Parallel()

	spy := &listValidationResultsUseCaseSpy{
		reply: validatorresultscontracts.ListValidationResultsReply{
			Results: []validatorresultscontracts.ValidationResultRecord{{
				MessageID: "msg-1",
				Binding: validatorresultscontracts.ValidationBindingRecord{
					Name:  "orders",
					Topic: "sales.order.created",
					Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
				},
				Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1"},
				Status:      validatorresultscontracts.ValidationStatusFailed,
				ProcessedAt: time.Unix(10, 0).UTC(),
				Violations: []validatorresultscontracts.ViolationRecord{{
					Rule:     "order_id_required",
					Field:    "order_id",
					Operator: "required",
					Severity: "error",
					Message:  "field is required",
				}},
			}},
		},
	}
	handler := NewRuntimeWebHandler(nil, nil, spy)

	req := httptest.NewRequest(http.MethodGet, "/runtime/validator/results?binding_name=orders&limit=5", nil)
	rec := httptest.NewRecorder()

	handler.ListValidationResults(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if spy.query.BindingName != "orders" || spy.query.Limit != 5 {
		t.Fatalf("unexpected validation results query %+v", spy.query)
	}

	var body map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	results := body["results"].([]any)
	if len(results) != 1 {
		t.Fatalf("expected one result, got %v", results)
	}
}

func TestListValidationResultsRejectsInvalidLimit(t *testing.T) {
	t.Parallel()

	handler := NewRuntimeWebHandler(nil, nil, &listValidationResultsUseCaseSpy{})
	req := httptest.NewRequest(http.MethodGet, "/runtime/validator/results?limit=abc", nil)
	rec := httptest.NewRecorder()

	handler.ListValidationResults(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected status %d, got %d", http.StatusBadRequest, rec.Code)
	}
}
