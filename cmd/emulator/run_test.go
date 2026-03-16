package main

import (
	"context"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	sharedruntime "internal/application/runtimecontracts"
	"internal/shared/settings"
)

func TestRefreshBootstrapStateDetectsChange(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}},{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}},{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}]}`))
	}))
	defer server.Close()

	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: server.URL,
			Timeout: "1s",
		},
	}

	state, changed, prob := refreshBootstrapState(context.Background(), slog.Default(), config, dataplaneapp.DefaultRegistry(), mustBootstrapSignature("orders-br", "sales.order.created", "ver-br", "tenant", "br", "sum-br", "artifact-br", "artifact-sum-br"))
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
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}]}`))
	}))
	defer server.Close()

	config := settings.AppConfig{
		Bootstrap: settings.BootstrapConfig{
			BaseURL: server.URL,
			Timeout: "1s",
		},
	}

	state, changed, prob := refreshBootstrapState(context.Background(), slog.Default(), config, dataplaneapp.DefaultRegistry(), mustBootstrapSignature("orders-br", "sales.order.created", "ver-br", "tenant", "br", "sum-br", "artifact-br", "artifact-sum-br"))
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

	next, signature := reconcileBootstrapState(ctx, logger, config, dataplaneapp.DefaultRegistry(), current, "current-signature")
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
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}]}`))
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

	next, signature := reconcileBootstrapState(context.Background(), logger, config, dataplaneapp.DefaultRegistry(), current, "current-signature")
	if signature == "current-signature" {
		t.Fatal("expected signature to change")
	}
	if len(next.Index.All()) != 1 {
		t.Fatalf("expected adopted bootstrap binding, got %+v", next.Index.All())
	}
}

func TestDefaultEmulatorRuntimeDependenciesStayAligned(t *testing.T) {
	t.Parallel()

	deps := defaultEmulatorRuntimeDependencies()
	if deps.dataPlaneRegistry.JetStream.Ingested.SubjectPrefix == "" {
		t.Fatal("expected dataplane registry to expose ingested subject prefix")
	}
	if deps.configctlRegistry.EmulatorRuntimeChanged.Durable == "" {
		t.Fatal("expected configctl registry to expose emulator runtime refresh durable")
	}
}

func mustBootstrapSignature(name, topic, versionID, scopeKind, scopeKey, definitionChecksum, artifactID, artifactChecksum string) string {
	bootstrap := runtimebootstrap.ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{
			{
				Binding: configctlcontracts.BindingRecord{Name: name, Topic: topic},
				Runtime: sharedruntime.RuntimeRecord{
					Scope:    sharedruntime.ScopeRecord{Kind: scopeKind, Key: scopeKey},
					Config:   sharedruntime.ConfigRecord{VersionID: versionID, DefinitionChecksum: definitionChecksum},
					Artifact: sharedruntime.ArtifactRecord{ID: artifactID, Checksum: artifactChecksum, RuntimeLoader: "validator:v1"},
				},
			},
		},
		Runtimes: []sharedruntime.RuntimeRecord{
			{
				Scope:    sharedruntime.ScopeRecord{Kind: scopeKind, Key: scopeKey},
				Config:   sharedruntime.ConfigRecord{VersionID: versionID, DefinitionChecksum: definitionChecksum},
				Artifact: sharedruntime.ArtifactRecord{ID: artifactID, Checksum: artifactChecksum, RuntimeLoader: "validator:v1"},
			},
		},
	}
	return bootstrap.Signature()
}
