package runtimecontracts

import (
	"testing"
	"time"

	configdomain "internal/domain/configctl"
)

func TestRecordFromProjectionFamiliesShareTheSameRuntimeLanguage(t *testing.T) {
	t.Parallel()

	artifact := configdomain.CompilationArtifact{
		ID:            "artifact-1",
		SchemaVersion: "runtime/v1",
		Checksum:      "artifact-checksum",
		StorageRef:    "memory://artifacts/core/v1",
		RuntimeLoader: "validator:v1",
		CreatedAt:     time.Unix(10, 0).UTC(),
	}

	validatorRecord := RecordFromProjection(configdomain.RuntimeProjection{
		Scope:              configdomain.ActivationScope{Kind: "tenant", Key: "br"},
		ConfigSetID:        "set-1",
		ConfigKey:          "core",
		VersionID:          "ver-1",
		Version:            2,
		Artifact:           artifact,
		ActivatedAt:        time.Unix(20, 0).UTC(),
		DefinitionChecksum: "definition-1",
	})
	ingestionRecord := RecordFromIngestionProjection(configdomain.IngestionRuntimeProjection{
		Scope:              configdomain.ActivationScope{Kind: "tenant", Key: "br"},
		ConfigSetID:        "set-1",
		ConfigKey:          "core",
		VersionID:          "ver-1",
		Version:            2,
		Artifact:           artifact,
		ActivatedAt:        time.Unix(20, 0).UTC(),
		DefinitionChecksum: "definition-1",
	})

	if validatorRecord != ingestionRecord {
		t.Fatalf("expected shared runtime language, got validator=%+v ingestion=%+v", validatorRecord, ingestionRecord)
	}
}
