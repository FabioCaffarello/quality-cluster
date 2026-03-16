package runtimebootstrap

import (
	"context"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"internal/shared/settings"
)

func TestWaitForConfiguredActiveIngestionBootstrapUsesAppConfig(t *testing.T) {
	t.Parallel()

	var gotCorrelationID string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotCorrelationID = r.Header.Get("X-Correlation-ID")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"global","key":"default"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"global","key":"default"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}]}`))
	}))
	defer server.Close()

	bootstrapState, prob := WaitForConfiguredActiveIngestionBootstrap(context.Background(), slog.Default(), settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL:   server.URL,
			ScopeKind: "global",
			ScopeKey:  "default",
			Timeout:   "1s",
		},
	}, "consumer")
	if prob != nil {
		t.Fatalf("wait for configured bootstrap: %v", prob)
	}

	if gotCorrelationID != "consumer.bootstrap" {
		t.Fatalf("expected correlation id %q, got %q", "consumer.bootstrap", gotCorrelationID)
	}
	if len(bootstrapState.Index.Topics()) != 1 {
		t.Fatalf("expected indexed topic, got %+v", bootstrapState.Index.Topics())
	}
}

func TestWaitForConfiguredActiveIngestionBootstrapRequiresBaseURL(t *testing.T) {
	t.Parallel()

	_, prob := WaitForConfiguredActiveIngestionBootstrap(context.Background(), slog.Default(), settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			Timeout: time.Second.String(),
		},
	}, "consumer")
	if prob == nil {
		t.Fatal("expected missing bootstrap base url to fail")
	}
}

func TestWaitForConfiguredActiveIngestionBootstrapSetIgnoresConfiguredScope(t *testing.T) {
	t.Parallel()

	var gotScopeKind string
	var gotScopeKey string
	var gotCorrelationID string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotScopeKind = r.URL.Query().Get("scope_kind")
		gotScopeKey = r.URL.Query().Get("scope_key")
		gotCorrelationID = r.Header.Get("X-Correlation-ID")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}},{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}},{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}]}`))
	}))
	defer server.Close()

	bootstrapState, prob := WaitForConfiguredActiveIngestionBootstrapSet(context.Background(), slog.Default(), settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL:   server.URL,
			ScopeKind: "global",
			ScopeKey:  "default",
			Timeout:   "1s",
		},
	}, "consumer")
	if prob != nil {
		t.Fatalf("wait for configured bootstrap set: %v", prob)
	}
	if gotScopeKind != "" || gotScopeKey != "" {
		t.Fatalf("expected aggregate bootstrap set to ignore configured scope, got scope_kind=%q scope_key=%q", gotScopeKind, gotScopeKey)
	}
	if gotCorrelationID != "consumer.bootstrap-set" {
		t.Fatalf("expected correlation id %q, got %q", "consumer.bootstrap-set", gotCorrelationID)
	}
	if len(bootstrapState.Index.All()) != 2 {
		t.Fatalf("expected aggregate bootstrap bindings, got %+v", bootstrapState.Index.All())
	}
}
