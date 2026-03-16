package validator

import (
	"testing"
	"time"

	actorcommon "internal/actors/common"
	runtimecontracts "internal/application/validatorruntime/contracts"
	configdomain "internal/domain/configctl"
)

func TestRuntimeCacheActorUpdatesAndServesActiveRuntime(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "runtime-cache-test")
	projection := configdomain.RuntimeProjection{
		Scope:              configdomain.DefaultActivationScope(),
		ConfigSetID:        "set-1",
		ConfigKey:          "core",
		VersionID:          "cfg-123",
		Version:            2,
		Artifact:           configdomain.CompilationArtifact{ID: "artifact-1", Checksum: "checksum-1", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://configctl/artifacts/set-1/cfg-123", CreatedAt: time.Unix(10, 0).UTC()},
		ActivatedAt:        time.Unix(20, 0).UTC(),
		DefinitionChecksum: "definition-1",
	}
	engine.Send(cachePID, applyRuntimeUpdateMessage{
		Event: configdomain.ConfigActivatedEvent{Projection: projection},
	})

	result, err := engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache: %v", err)
	}

	reply, ok := result.(queryActiveRuntimeResult)
	if !ok {
		t.Fatalf("expected queryActiveRuntimeResult, got %T", result)
	}
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if reply.Reply.Runtime.Config.VersionID != "cfg-123" || reply.Reply.Runtime.Artifact.ID != "artifact-1" {
		t.Fatalf("unexpected runtime reply: %+v", reply.Reply.Runtime)
	}
	if reply.Reply.Runtime.LoadedAt.IsZero() {
		t.Fatal("expected loaded_at to be populated")
	}
}

func TestRuntimeCacheActorServesMultipleScopes(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "runtime-cache-multi-scope-test")
	engine.Send(cachePID, applyRuntimeUpdateMessage{
		Event: configdomain.ConfigActivatedEvent{Projection: configdomain.RuntimeProjection{
			Scope:              configdomain.ActivationScope{Kind: "tenant", Key: "br"},
			ConfigSetID:        "set-br",
			ConfigKey:          "orders",
			VersionID:          "cfg-br",
			Version:            1,
			Artifact:           configdomain.CompilationArtifact{ID: "artifact-br", Checksum: "checksum-br", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-br", CreatedAt: time.Unix(10, 0).UTC()},
			ActivatedAt:        time.Unix(20, 0).UTC(),
			DefinitionChecksum: "definition-br",
		}},
	})
	engine.Send(cachePID, applyRuntimeUpdateMessage{
		Event: configdomain.ConfigActivatedEvent{Projection: configdomain.RuntimeProjection{
			Scope:              configdomain.DefaultActivationScope(),
			ConfigSetID:        "set-global",
			ConfigKey:          "orders",
			VersionID:          "cfg-global",
			Version:            2,
			Artifact:           configdomain.CompilationArtifact{ID: "artifact-global", Checksum: "checksum-global", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-global", CreatedAt: time.Unix(10, 0).UTC()},
			ActivatedAt:        time.Unix(20, 0).UTC(),
			DefinitionChecksum: "definition-global",
		}},
	})

	result, err := engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{ScopeKind: "tenant", ScopeKey: "br"},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache: %v", err)
	}

	reply := result.(queryActiveRuntimeResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if reply.Reply.Runtime.Config.VersionID != "cfg-br" {
		t.Fatalf("expected tenant runtime, got %+v", reply.Reply.Runtime)
	}
}

func TestRuntimeCacheActorReturnsNotFoundBeforeActivation(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "runtime-cache-empty-test")
	result, err := engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache: %v", err)
	}

	reply, ok := result.(queryActiveRuntimeResult)
	if !ok {
		t.Fatalf("expected queryActiveRuntimeResult, got %T", result)
	}
	if reply.Prob == nil || reply.Prob.Code == "" {
		t.Fatal("expected not found problem")
	}
}

func TestRuntimeCacheActorBootstrapsProjectionAndIgnoresOlderActivation(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "runtime-cache-bootstrap-test")
	engine.Send(cachePID, bootstrapRuntimeProjectionMessage{
		Projection: configdomain.RuntimeProjection{
			Scope:              configdomain.DefaultActivationScope(),
			ConfigSetID:        "set-1",
			ConfigKey:          "core",
			VersionID:          "cfg-new",
			Version:            2,
			Artifact:           configdomain.CompilationArtifact{ID: "artifact-new", Checksum: "checksum-new", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-new", CreatedAt: time.Unix(20, 0).UTC()},
			ActivatedAt:        time.Unix(30, 0).UTC(),
			DefinitionChecksum: "definition-new",
		},
	})
	engine.Send(cachePID, applyRuntimeUpdateMessage{
		Event: configdomain.ConfigActivatedEvent{
			Projection: configdomain.RuntimeProjection{
				Scope:              configdomain.DefaultActivationScope(),
				ConfigSetID:        "set-1",
				ConfigKey:          "core",
				VersionID:          "cfg-old",
				Version:            1,
				Artifact:           configdomain.CompilationArtifact{ID: "artifact-old", Checksum: "checksum-old", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-old", CreatedAt: time.Unix(10, 0).UTC()},
				ActivatedAt:        time.Unix(15, 0).UTC(),
				DefinitionChecksum: "definition-old",
			},
		},
	})

	result, err := engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache: %v", err)
	}

	reply := result.(queryActiveRuntimeResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if reply.Reply.Runtime.Config.VersionID != "cfg-new" {
		t.Fatalf("expected newest bootstrapped runtime to win, got %+v", reply.Reply.Runtime)
	}
}

func TestRuntimeCacheActorEvictsScopeAndIgnoresStaleDeactivation(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "runtime-cache-evict-test")
	scope := configdomain.ActivationScope{Kind: "tenant", Key: "br"}
	engine.Send(cachePID, bootstrapRuntimeProjectionMessage{
		Projection: configdomain.RuntimeProjection{
			Scope:              scope,
			ConfigSetID:        "set-br",
			ConfigKey:          "orders",
			VersionID:          "cfg-br",
			Version:            1,
			Artifact:           configdomain.CompilationArtifact{ID: "artifact-br", Checksum: "checksum-br", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-br", CreatedAt: time.Unix(10, 0).UTC()},
			ActivatedAt:        time.Unix(20, 0).UTC(),
			DefinitionChecksum: "definition-br",
		},
	})
	engine.Send(cachePID, evictRuntimeScopeMessage{
		Scope:         scope,
		ConfigSetID:   "set-br",
		VersionID:     "cfg-br",
		DeactivatedAt: time.Unix(19, 0).UTC(),
	})

	result, err := engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{ScopeKind: "tenant", ScopeKey: "br"},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache after stale deactivation: %v", err)
	}
	reply := result.(queryActiveRuntimeResult)
	if reply.Prob != nil {
		t.Fatalf("expected stale deactivation to be ignored, got %v", reply.Prob)
	}

	engine.Send(cachePID, evictRuntimeScopeMessage{
		Scope:         scope,
		ConfigSetID:   "set-other",
		VersionID:     "cfg-other",
		DeactivatedAt: time.Unix(20, 0).UTC(),
	})
	result, err = engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{ScopeKind: "tenant", ScopeKey: "br"},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache after mismatched deactivation: %v", err)
	}
	reply = result.(queryActiveRuntimeResult)
	if reply.Prob != nil {
		t.Fatalf("expected mismatched deactivation to be ignored, got %v", reply.Prob)
	}

	engine.Send(cachePID, evictRuntimeScopeMessage{
		Scope:         scope,
		ConfigSetID:   "set-br",
		VersionID:     "cfg-br",
		DeactivatedAt: time.Unix(20, 0).UTC(),
	})
	result, err = engine.Request(cachePID, queryActiveRuntimeMessage{
		Query: runtimecontracts.GetActiveRuntimeQuery{ScopeKind: "tenant", ScopeKey: "br"},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("request runtime cache after eviction: %v", err)
	}
	reply = result.(queryActiveRuntimeResult)
	if reply.Prob == nil {
		t.Fatal("expected runtime to be evicted")
	}
}
