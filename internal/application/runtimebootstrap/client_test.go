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
