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
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"global","key":"default"},"config":{"version_id":"ver-1"}}}]}`))
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
