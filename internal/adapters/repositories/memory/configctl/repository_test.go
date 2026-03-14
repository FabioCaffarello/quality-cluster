package configctl

import (
	"context"
	"sync"
	"testing"
	"time"

	configdomain "internal/domain/configctl"
	"internal/shared/memdb"
)

func TestConfigSetRecordRoundTrip(t *testing.T) {
	t.Parallel()

	set := mustCompiledSet(t, "set-1", "core", "ver-1")
	if prob := set.CreateDraftVersion("ver-2", validSource(), testTime(4)); prob != nil {
		t.Fatalf("create draft version: %v", prob)
	}
	if prob := set.RejectVersion("ver-2", "invalid schema", testTime(5)); prob != nil {
		t.Fatalf("reject version: %v", prob)
	}

	record := newConfigSetRecord(set)
	roundTrip, err := record.toDomain()
	if err != nil {
		t.Fatalf("record to domain: %v", err)
	}

	if roundTrip.ID != set.ID || roundTrip.Key != set.Key || len(roundTrip.Versions) != len(set.Versions) {
		t.Fatalf("unexpected roundtrip set: %+v", roundTrip)
	}
	if roundTrip.Versions[0].Document == nil || roundTrip.Versions[0].Artifact == nil {
		t.Fatalf("expected validated document and artifact to survive roundtrip: %+v", roundTrip.Versions[0])
	}
	if roundTrip.Versions[1].RejectedReason != "invalid schema" {
		t.Fatalf("expected rejected reason to survive roundtrip, got %q", roundTrip.Versions[1].RejectedReason)
	}
}

func TestRepositoryConfigSetQueriesAndDefensiveCopies(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	set := mustCompiledSet(t, "set-1", "core", "ver-1")

	if prob := repository.SaveConfigSet(context.Background(), set); prob != nil {
		t.Fatalf("save config set: %v", prob)
	}

	set.Key = "changed-after-save"
	set.Versions[0].Source.Content = "mutated"

	byID, prob := repository.GetConfigSetByID(context.Background(), "set-1")
	if prob != nil {
		t.Fatalf("get by id: %v", prob)
	}
	if byID.Key != "core" || byID.Versions[0].Source.Content == "mutated" {
		t.Fatalf("expected persisted state to be isolated, got %+v", byID)
	}

	byKey, prob := repository.GetConfigSetByKey(context.Background(), "core")
	if prob != nil {
		t.Fatalf("get by key: %v", prob)
	}
	if byKey.ID != "set-1" {
		t.Fatalf("expected set-1 by key, got %q", byKey.ID)
	}

	byVersion, prob := repository.GetConfigSetByVersionID(context.Background(), "ver-1")
	if prob != nil {
		t.Fatalf("get by version: %v", prob)
	}
	if byVersion.ID != "set-1" {
		t.Fatalf("expected set-1 by version, got %q", byVersion.ID)
	}

	byID.Versions[0].Document.Metadata.Labels["team"] = "mutated"
	again, prob := repository.GetConfigSetByID(context.Background(), "set-1")
	if prob != nil {
		t.Fatalf("get by id again: %v", prob)
	}
	if again.Versions[0].Document.Metadata.Labels["team"] != "quality" {
		t.Fatalf("expected read copy to be isolated, got %+v", again.Versions[0].Document.Metadata.Labels)
	}
	if events := again.PullEvents(); len(events) != 0 {
		t.Fatalf("expected pending events to stay out of persistence, got %d", len(events))
	}
}

func TestRepositoryRejectsCorruptedPersistedConfigSetRecord(t *testing.T) {
	t.Parallel()

	db := memdb.New()
	repository := NewRepository(db)

	if err := db.Update(context.Background(), func(tx memdb.WriteTx) error {
		tx.Put(bucketConfigSets, "set-1", []byte(`{"id":"set-1","key":"core","current_version":1,"versions":[],"created_at":"not-a-time","updated_at":"2026-01-01T00:00:00Z"}`))
		return nil
	}); err != nil {
		t.Fatalf("seed: %v", err)
	}

	if _, prob := repository.GetConfigSetByID(context.Background(), "set-1"); prob == nil {
		t.Fatal("expected corrupted config set record to fail")
	}
}

func TestRepositoryDeleteConfigSetRemovesIndexes(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	set := mustCompiledSet(t, "set-1", "core", "ver-1")

	if prob := repository.SaveConfigSet(context.Background(), set); prob != nil {
		t.Fatalf("save config set: %v", prob)
	}
	if prob := repository.DeleteConfigSet(context.Background(), "set-1"); prob != nil {
		t.Fatalf("delete config set: %v", prob)
	}

	if _, prob := repository.GetConfigSetByID(context.Background(), "set-1"); prob == nil {
		t.Fatal("expected deleted set to be absent by id")
	}
	if _, prob := repository.GetConfigSetByKey(context.Background(), "core"); prob == nil {
		t.Fatal("expected deleted set to be absent by key")
	}
	if _, prob := repository.GetConfigSetByVersionID(context.Background(), "ver-1"); prob == nil {
		t.Fatal("expected deleted set to be absent by version")
	}
}

func TestRepositoryActivationIndexesAndOrdering(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	set := mustCompiledSet(t, "set-1", "core", "ver-1")
	if prob := set.CreateDraftVersion("ver-2", validSource(), testTime(4)); prob != nil {
		t.Fatalf("create second version: %v", prob)
	}
	if _, prob := set.ValidateVersion("ver-2", testTime(5)); prob != nil {
		t.Fatalf("validate second version: %v", prob)
	}
	artifact, prob := configdomain.NewCompilationArtifact("artifact-2", "runtime/v1", "checksum-2", "memory://artifacts/core/v2", "validator:v1", "compiler:v1", testTime(6))
	if prob != nil {
		t.Fatalf("new artifact: %v", prob)
	}
	if prob := set.CompileVersion("ver-2", artifact, testTime(6)); prob != nil {
		t.Fatalf("compile second version: %v", prob)
	}
	if prob := repository.SaveConfigSet(context.Background(), set); prob != nil {
		t.Fatalf("save config set: %v", prob)
	}

	versionOne, _ := set.VersionByID("ver-1")
	versionTwo, _ := set.VersionByID("ver-2")

	activeOne := mustActivation(t, "act-1", set, versionOne, configdomain.ActivationScope{Kind: "tenant", Key: "br"}, testTime(7))
	activeTwo := mustActivation(t, "act-2", set, versionOne, configdomain.ActivationScope{Kind: "global", Key: "default"}, testTime(8))
	deactivated, prob := activeOne.Deactivate(testTime(9))
	if prob != nil {
		t.Fatalf("deactivate activation: %v", prob)
	}
	activeThree := mustActivation(t, "act-3", set, versionTwo, configdomain.ActivationScope{Kind: "tenant", Key: "br"}, testTime(10))

	if prob := repository.SaveActivation(context.Background(), activeOne); prob != nil {
		t.Fatalf("save activation 1: %v", prob)
	}
	if prob := repository.SaveActivation(context.Background(), activeTwo); prob != nil {
		t.Fatalf("save activation 2: %v", prob)
	}
	if prob := repository.SaveActivation(context.Background(), deactivated); prob != nil {
		t.Fatalf("save deactivated activation: %v", prob)
	}
	if prob := repository.SaveActivation(context.Background(), activeThree); prob != nil {
		t.Fatalf("save activation 3: %v", prob)
	}

	byScope, prob := repository.GetActivationByScope(context.Background(), configdomain.ActivationScope{Kind: "tenant", Key: "br"})
	if prob != nil {
		t.Fatalf("get activation by scope: %v", prob)
	}
	if byScope.ID != "act-3" {
		t.Fatalf("expected active scope to point to act-3, got %q", byScope.ID)
	}

	versionOneActivations, prob := repository.ListActivationsByVersionID(context.Background(), "ver-1")
	if prob != nil {
		t.Fatalf("list activations by version: %v", prob)
	}
	if len(versionOneActivations) != 2 {
		t.Fatalf("expected 2 activations on ver-1, got %d", len(versionOneActivations))
	}
	if versionOneActivations[0].ID != "act-1" || versionOneActivations[1].ID != "act-2" {
		t.Fatalf("expected activation ordering by activation time, got %+v", versionOneActivations)
	}

	if prob := repository.DeleteActivation(context.Background(), "act-3"); prob != nil {
		t.Fatalf("delete activation: %v", prob)
	}
	if _, prob := repository.GetActivationByScope(context.Background(), configdomain.ActivationScope{Kind: "tenant", Key: "br"}); prob == nil {
		t.Fatal("expected scope index to be removed after deleting active activation")
	}
}

func TestRepositoryReportsCorruptedActivationIndex(t *testing.T) {
	t.Parallel()

	db := memdb.New()
	repository := NewRepository(db)

	if err := db.Update(context.Background(), func(tx memdb.WriteTx) error {
		tx.Put(bucketActivationByScope, "tenant:br", []byte("act-missing"))
		return nil
	}); err != nil {
		t.Fatalf("seed: %v", err)
	}

	if _, prob := repository.GetActivationByScope(context.Background(), configdomain.ActivationScope{Kind: "tenant", Key: "br"}); prob == nil {
		t.Fatal("expected corrupted activation index to fail")
	}
}

func TestRepositorySaveAndListIngestionRuntimes(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	runtime := configdomain.IngestionRuntimeProjection{
		Scope:              configdomain.ActivationScope{Kind: "tenant", Key: "br"},
		ConfigSetID:        "set-1",
		ConfigKey:          "core",
		VersionID:          "ver-1",
		Version:            1,
		Artifact:           configdomain.CompilationArtifact{ID: "artifact-1", SchemaVersion: "runtime/v1", Checksum: "checksum-1", StorageRef: "memory://artifacts/core/v1", RuntimeLoader: "validator:v1", CreatedAt: testTime(5)},
		ActivatedAt:        testTime(6),
		Bindings:           []configdomain.Binding{{Name: "orders", Topic: "orders.v1"}},
		Fields:             []configdomain.Field{{Name: "order_id", Type: configdomain.FieldTypeString, Required: true}},
		DefinitionChecksum: "definition-1",
	}

	if prob := repository.SaveIngestionRuntime(context.Background(), runtime); prob != nil {
		t.Fatalf("save ingestion runtime: %v", prob)
	}

	runtime.Bindings[0].Topic = "mutated"
	runtimes, prob := repository.ListIngestionRuntimes(context.Background())
	if prob != nil {
		t.Fatalf("list ingestion runtimes: %v", prob)
	}
	if len(runtimes) != 1 {
		t.Fatalf("expected one ingestion runtime, got %d", len(runtimes))
	}
	if runtimes[0].Bindings[0].Topic != "orders.v1" {
		t.Fatalf("expected persisted ingestion runtime to be isolated, got %+v", runtimes[0])
	}
	if len(runtimes[0].Fields) != 1 || runtimes[0].Fields[0].Name != "order_id" {
		t.Fatalf("expected persisted ingestion fields, got %+v", runtimes[0].Fields)
	}

	if prob := repository.DeleteIngestionRuntimeByScope(context.Background(), configdomain.ActivationScope{Kind: "tenant", Key: "br"}); prob != nil {
		t.Fatalf("delete ingestion runtime: %v", prob)
	}
	runtimes, prob = repository.ListIngestionRuntimes(context.Background())
	if prob != nil {
		t.Fatalf("list ingestion runtimes after delete: %v", prob)
	}
	if len(runtimes) != 0 {
		t.Fatalf("expected no ingestion runtimes after delete, got %+v", runtimes)
	}
}

func TestRepositoryListConfigSetsIsSortedByCreatedAt(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	first := mustCompiledSet(t, "set-1", "core", "ver-1")
	second := mustCompiledSet(t, "set-2", "billing", "ver-2")
	first.CreatedAt = testTime(10)
	first.UpdatedAt = testTime(10)
	first.Versions[0].CreatedAt = testTime(10)
	first.Versions[0].UpdatedAt = testTime(10)
	second.CreatedAt = testTime(20)
	second.UpdatedAt = testTime(20)
	second.Versions[0].CreatedAt = testTime(20)
	second.Versions[0].UpdatedAt = testTime(20)

	if prob := repository.SaveConfigSet(context.Background(), second); prob != nil {
		t.Fatalf("save second: %v", prob)
	}
	if prob := repository.SaveConfigSet(context.Background(), first); prob != nil {
		t.Fatalf("save first: %v", prob)
	}

	sets, prob := repository.ListConfigSets(context.Background())
	if prob != nil {
		t.Fatalf("list config sets: %v", prob)
	}
	if len(sets) != 2 || sets[0].ID != "set-1" || sets[1].ID != "set-2" {
		t.Fatalf("expected created_at ordering, got %+v", sets)
	}
}

func TestRepositorySupportsConcurrentAccess(t *testing.T) {
	t.Parallel()

	repository := NewRepository(nil)
	const workers = 24

	var wg sync.WaitGroup
	for index := range workers {
		wg.Add(1)
		go func(index int) {
			defer wg.Done()

			set := mustCompiledSet(t, "set-"+string(rune('a'+index)), "key-"+string(rune('a'+index)), "ver-"+string(rune('a'+index)))
			if prob := repository.SaveConfigSet(context.Background(), set); prob != nil {
				t.Errorf("save config set %d: %v", index, prob)
				return
			}
			if _, prob := repository.GetConfigSetByVersionID(context.Background(), "ver-"+string(rune('a'+index))); prob != nil {
				t.Errorf("get config set %d: %v", index, prob)
			}
		}(index)
	}
	wg.Wait()

	sets, prob := repository.ListConfigSets(context.Background())
	if prob != nil {
		t.Fatalf("list config sets: %v", prob)
	}
	if len(sets) != workers {
		t.Fatalf("expected %d sets, got %d", workers, len(sets))
	}
}

func mustCompiledSet(t *testing.T, setID, key, versionID string) configdomain.ConfigSet {
	t.Helper()

	set, prob := configdomain.NewConfigSet(setID, key, versionID, validSource(), testTime(1))
	if prob != nil {
		t.Fatalf("new config set: %v", prob)
	}
	if _, prob := set.ValidateVersion(versionID, testTime(2)); prob != nil {
		t.Fatalf("validate version: %v", prob)
	}
	artifact, prob := configdomain.NewCompilationArtifact("artifact-"+versionID, "runtime/v1", "checksum-"+versionID, "memory://artifacts/"+versionID, "validator:v1", "compiler:v1", testTime(3))
	if prob != nil {
		t.Fatalf("new artifact: %v", prob)
	}
	if prob := set.CompileVersion(versionID, artifact, testTime(3)); prob != nil {
		t.Fatalf("compile version: %v", prob)
	}

	return set
}

func mustActivation(t *testing.T, id string, set configdomain.ConfigSet, version configdomain.ConfigVersion, scope configdomain.ActivationScope, at time.Time) configdomain.Activation {
	t.Helper()

	activation, prob := configdomain.NewActivation(id, set, version, scope, at)
	if prob != nil {
		t.Fatalf("new activation: %v", prob)
	}
	return activation
}

func validSource() configdomain.ConfigSource {
	return configdomain.ConfigSource{
		Format: configdomain.FormatJSON,
		Content: `{
			"metadata":{"name":"Core Quality Config","labels":{"team":"quality"}},
			"bindings":[{"name":"orders","topic":"orders.v1"}],
			"fields":[{"name":"order_id","type":"string","required":true}],
			"rules":[{"name":"order_id_required","field":"order_id","operator":"required","severity":"error"}]
		}`,
	}
}

func testTime(offset int) time.Time {
	return time.Unix(int64(offset), 0).UTC()
}
