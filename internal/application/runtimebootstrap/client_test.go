package runtimebootstrap

import (
	"context"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	"internal/shared/problem"
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
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string"}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}]}`))
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
	if len(reply.Runtimes) != 1 || reply.Runtimes[0].Artifact.ID != "artifact-1" {
		t.Fatalf("expected compact runtime summary, got %+v", reply.Runtimes)
	}
}

func TestClientWaitForActiveIngestionBootstrapBuildsIndex(t *testing.T) {
	t.Parallel()

	requests := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests++
		w.Header().Set("Content-Type", "application/json")
		if requests == 1 {
			_, _ = w.Write([]byte(`{"bindings":[],"runtimes":[]}`))
			return
		}
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}]}`))
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
	if len(bootstrapState.Runtimes) != 1 {
		t.Fatalf("expected one runtime summary, got %+v", bootstrapState.Runtimes)
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
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders-br","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}}},{"binding":{"name":"orders-us","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-br","definition_checksum":"sum-br"},"artifact":{"id":"artifact-br","checksum":"artifact-sum-br","runtime_loader":"validator:v1"}},{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-us","definition_checksum":"sum-us"},"artifact":{"id":"artifact-us","checksum":"artifact-sum-us","runtime_loader":"validator:v1"}}]}`))
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
	if len(bootstrapState.Runtimes) != 2 {
		t.Fatalf("expected aggregate runtime summaries, got %+v", bootstrapState.Runtimes)
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
	left.Bindings[0].Runtime.Artifact.ID = "artifact-us"
	left.Bindings[0].Runtime.Artifact.Checksum = "artifact-sum-us"
	left.Bindings[0].Runtime.Artifact.RuntimeLoader = "validator:v1"
	left.Bindings = append(left.Bindings, configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders-br", Topic: "sales.order.created"},
	})
	left.Bindings[1].Runtime.Scope.Kind = "tenant"
	left.Bindings[1].Runtime.Scope.Key = "br"
	left.Bindings[1].Runtime.Config.VersionID = "ver-br"
	left.Bindings[1].Runtime.Config.DefinitionChecksum = "sum-br"
	left.Bindings[1].Runtime.Artifact.ID = "artifact-br"
	left.Bindings[1].Runtime.Artifact.Checksum = "artifact-sum-br"
	left.Bindings[1].Runtime.Artifact.RuntimeLoader = "validator:v1"
	left.Runtimes = []sharedruntime.RuntimeRecord{left.Bindings[0].Runtime, left.Bindings[1].Runtime}

	right := ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{
			left.Bindings[1],
			left.Bindings[0],
		},
		Runtimes: []sharedruntime.RuntimeRecord{
			left.Bindings[1].Runtime,
			left.Bindings[0].Runtime,
		},
	}

	if left.Signature() == "" {
		t.Fatal("expected non-empty signature")
	}
	if left.Signature() != right.Signature() {
		t.Fatalf("expected signature to ignore binding order, got %q vs %q", left.Signature(), right.Signature())
	}
}

func TestActiveIngestionBootstrapSignatureChangesWhenArtifactChanges(t *testing.T) {
	t.Parallel()

	left := ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{
			{
				Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
				Runtime: sharedruntime.RuntimeRecord{
					Scope:    sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
					Config:   sharedruntime.ConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
					Artifact: sharedruntime.ArtifactRecord{ID: "artifact-1", Checksum: "artifact-sum-1", RuntimeLoader: "validator:v1"},
				},
			},
		},
		Runtimes: []sharedruntime.RuntimeRecord{
			{
				Scope:    sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
				Config:   sharedruntime.ConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
				Artifact: sharedruntime.ArtifactRecord{ID: "artifact-1", Checksum: "artifact-sum-1", RuntimeLoader: "validator:v1"},
			},
		},
	}
	right := ActiveIngestionBootstrap{
		Bindings: []configctlcontracts.ActiveIngestionBindingRecord{left.Bindings[0]},
		Runtimes: []sharedruntime.RuntimeRecord{left.Runtimes[0]},
	}
	right.Bindings[0].Runtime.Artifact.Checksum = "artifact-sum-2"
	right.Runtimes[0].Artifact.Checksum = "artifact-sum-2"

	if left.Signature() == right.Signature() {
		t.Fatalf("expected signature to change when artifact checksum changes, got %q", left.Signature())
	}
}

func TestActiveIngestionBootstrapRuntimeRefsAreCanonical(t *testing.T) {
	t.Parallel()

	bootstrap := ActiveIngestionBootstrap{
		Runtimes: []sharedruntime.RuntimeRecord{
			{
				Scope:    sharedruntime.ScopeRecord{Kind: "tenant", Key: "us"},
				Config:   sharedruntime.ConfigRecord{VersionID: "ver-us"},
				Artifact: sharedruntime.ArtifactRecord{ID: "artifact-us"},
			},
			{
				Scope:    sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
				Config:   sharedruntime.ConfigRecord{VersionID: "ver-br"},
				Artifact: sharedruntime.ArtifactRecord{ID: "artifact-br"},
			},
		},
	}

	if got, want := bootstrap.RuntimeRefs(), []string{"tenant:br:ver-br:artifact-br", "tenant:us:ver-us:artifact-us"}; len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Fatalf("expected canonical runtime refs %v, got %v", want, got)
	}
}

func TestClientWaitForActiveIngestionBootstrapRequiresRuntimeSummaries(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}}],"runtimes":[]}`))
	}))
	defer server.Close()

	client := NewClient(server.URL, time.Second)
	_, prob := client.WaitForActiveIngestionBootstrap(context.Background(), slog.Default(), WaitOptions{
		ScopeKind:    "tenant",
		ScopeKey:     "br",
		PollInterval: 10 * time.Millisecond,
	})
	if prob == nil || prob.Code != problem.InvalidArgument {
		t.Fatalf("expected invalid bootstrap runtimes problem, got %v", prob)
	}
}

func TestClientWaitForActiveIngestionBootstrapRejectsRuntimeSummaryDrift(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"bindings":[{"binding":{"name":"orders","topic":"sales.order.created"},"fields":[{"name":"order_id","type":"string","required":true}],"runtime":{"scope":{"kind":"tenant","key":"br"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}}],"runtimes":[{"scope":{"kind":"tenant","key":"us"},"config":{"version_id":"ver-1","definition_checksum":"sum-1"},"artifact":{"id":"artifact-1","checksum":"artifact-sum-1","runtime_loader":"validator:v1"}}]}`))
	}))
	defer server.Close()

	client := NewClient(server.URL, time.Second)
	_, prob := client.WaitForActiveIngestionBootstrap(context.Background(), slog.Default(), WaitOptions{
		ScopeKind:    "tenant",
		ScopeKey:     "br",
		PollInterval: 10 * time.Millisecond,
	})
	if prob == nil || prob.Code != problem.Conflict {
		t.Fatalf("expected runtime summary drift conflict, got %v", prob)
	}
}
