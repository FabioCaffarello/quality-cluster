package main

import (
	"context"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	runtimebootstrap "internal/application/runtimebootstrap"
	"internal/shared/settings"
)

func TestRefreshBootstrapStateDetectsChange(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"}}},{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"}}}]}`))
	}))
	defer server.Close()

	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: server.URL,
			Timeout: "1s",
		},
	}

	state, changed, prob := refreshBootstrapState(context.Background(), slog.Default(), config, "tenant|br|ver-br|sum-br|orders-br|sales.order.created")
	if prob != nil {
		t.Fatalf("refresh bootstrap state: %v", prob)
	}
	if !changed {
		t.Fatal("expected changed bootstrap signature")
	}
	if len(state.Index.All()) != 2 {
		t.Fatalf("expected aggregate bootstrap, got %+v", state.Index.All())
	}
}

func TestRefreshBootstrapStateDetectsNoChange(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"}}}]}`))
	}))
	defer server.Close()

	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: server.URL,
			Timeout: "1s",
		},
	}

	state, changed, prob := refreshBootstrapState(context.Background(), slog.Default(), config, "tenant|br|ver-br|sum-br|orders-br|sales.order.created")
	if prob != nil {
		t.Fatalf("refresh bootstrap state: %v", prob)
	}
	if changed {
		t.Fatal("expected unchanged bootstrap signature")
	}
	if len(state.Index.All()) != 1 {
		t.Fatalf("expected bootstrap binding, got %+v", state.Index.All())
	}
}

func TestReconcileBootstrapStateKeepsCurrentStateWhenRefreshFails(t *testing.T) {
	t.Parallel()

	current := runtimebootstrap.ActiveIngestionBootstrap{}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: "http://127.0.0.1:1",
			Timeout: "1ms",
		},
		Kafka: settings.KafkaConfig{
			Brokers: []string{"127.0.0.1:19092"},
		},
	}

	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()

	next, signature := reconcileBootstrapState(ctx, logger, config, current, "current-signature")
	if signature != "current-signature" {
		t.Fatalf("expected signature to stay unchanged, got %q", signature)
	}
	if len(next.Index.All()) != 0 {
		t.Fatalf("expected current bootstrap state to be preserved, got %+v", next.Index.All())
	}
}

func TestReconcileBootstrapStateAdoptsChangedBootstrap(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"}}}]}`))
	}))
	defer server.Close()

	current := runtimebootstrap.ActiveIngestionBootstrap{}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: server.URL,
			Timeout: "1s",
		},
		Kafka: settings.KafkaConfig{
			Brokers: []string{},
		},
	}

	originalEnsureTopics := ensureTopicsForBootstrap
	ensureTopicsForBootstrap = func(context.Context, []string, []string, time.Duration) error { return nil }
	defer func() { ensureTopicsForBootstrap = originalEnsureTopics }()

	next, signature := reconcileBootstrapState(context.Background(), logger, config, current, "current-signature")
	if signature == "current-signature" {
		t.Fatal("expected signature to change")
	}
	if len(next.Index.All()) != 1 {
		t.Fatalf("expected adopted bootstrap binding, got %+v", next.Index.All())
	}
}
