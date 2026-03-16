package runtimebootstrap

import (
	"context"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/requestctx"
)

func TestClientListActiveIngestionBindings(t *testing.T) {
	t.Parallel()

	var correlationID string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		correlationID = r.Header.Get("X-Correlation-ID")
		if got := r.URL.Query().Get("scope_kind"); got != "tenant" {
			t.Fatalf("expected scope_kind query, got %q", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string"}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1"}}}]}`))
	}))
	defer server.Close()

	client := NewClient(server.URL, time.Second)
	reply, prob := client.ListActiveIngestionBindings(requestctx.WithCorrelationID(context.Background(), "corr-123"), configctlcontracts.ListActiveIngestionBindingsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	})
	if prob != nil {
		t.Fatalf("bootstrap request: %v", prob)
	}
	if correlationID != "corr-123" {
		t.Fatalf("expected correlation id to be forwarded, got %q", correlationID)
	}
	if len(reply.Bindings) != 1 || len(reply.Bindings[0].Fields) != 1 {
		t.Fatalf("expected active binding payload, got %+v", reply.Bindings)
	}
}

func TestClientWaitForActiveIngestionBootstrapBuildsIndex(t *testing.T) {
	t.Parallel()

	requests := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests++
		w.Header().Set("Content-Type", "application/json")
		if requests == 1 {
			_, _ = w.Write([]byte(`{"bindings":[]}`))
			return
		}
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1"}}}]}`))
	}))
	defer server.Close()

	client := NewClient(server.URL, time.Second)
	bootstrapState, prob := client.WaitForActiveIngestionBootstrap(context.Background(), slog.Default(), WaitOptions{
		ScopeKind:    "tenant",
		ScopeKey:     "br",
		PollInterval: 10 * time.Millisecond,
	})
	if prob != nil {
		t.Fatalf("wait for active ingestion bootstrap: %v", prob)
	}
	if len(bootstrapState.Index.Topics()) != 1 {
		t.Fatalf("expected indexed topic, got %+v", bootstrapState.Index.Topics())
	}
	if requests < 2 {
		t.Fatalf("expected polling before bindings became available, got %d requests", requests)
	}
}

func TestClientWaitForActiveIngestionBootstrapSetBuildsAggregateIndex(t *testing.T) {
	t.Parallel()

	var gotScopeKind string
	var gotScopeKey string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotScopeKind = r.URL.Query().Get("scope_kind")
		gotScopeKey = r.URL.Query().Get("scope_key")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br"}}},{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us"}}}]}`))
	}))
	defer server.Close()

	client := NewClient(server.URL, time.Second)
	bootstrapState, prob := client.WaitForActiveIngestionBootstrapSet(context.Background(), slog.Default(), AggregateWaitOptions{
		CorrelationID: "aggregate-bootstrap",
		PollInterval:  10 * time.Millisecond,
	})
	if prob != nil {
		t.Fatalf("wait for active ingestion bootstrap set: %v", prob)
	}
	if gotScopeKind != "" || gotScopeKey != "" {
		t.Fatalf("expected aggregate bootstrap to omit scope query, got scope_kind=%q scope_key=%q", gotScopeKind, gotScopeKey)
	}
	if len(bootstrapState.Index.Topics()) != 1 {
		t.Fatalf("expected a shared topic to be indexed once, got %+v", bootstrapState.Index.Topics())
	}
	if len(bootstrapState.Index.All()) != 2 {
		t.Fatalf("expected aggregate bindings from multiple scopes, got %+v", bootstrapState.Index.All())
	}
}

func TestActiveIngestionBootstrapSignatureIsOrderIndependent(t *testing.T) {
	t.Parallel()

	left := ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{
			{
				Binding: configctlcontracts.BindingRecord{Name: "orders-us", Topic: "sales.order.created"},
				Runtime: configctlcontracts.ActiveIngestionBindingRecord{}.Runtime,
			},
		},
	}
	left.Bindings[0].Runtime.Scope.Kind = "tenant"
	left.Bindings[0].Runtime.Scope.Key = "us"
	left.Bindings[0].Runtime.Config.VersionID = "ver-us"
	left.Bindings[0].Runtime.Config.DefinitionChecksum = "sum-us"
	left.Bindings = append(left.Bindings, configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders-br", Topic: "sales.order.created"},
	})
	left.Bindings[1].Runtime.Scope.Kind = "tenant"
	left.Bindings[1].Runtime.Scope.Key = "br"
	left.Bindings[1].Runtime.Config.VersionID = "ver-br"
	left.Bindings[1].Runtime.Config.DefinitionChecksum = "sum-br"

	right := ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{
			left.Bindings[1],
			left.Bindings[0],
		},
	}

	if left.Signature() == "" {
		t.Fatal("expected non-empty signature")
	}
	if left.Signature() != right.Signature() {
		t.Fatalf("expected signature to ignore binding order, got %q vs %q", left.Signature(), right.Signature())
	}
}
