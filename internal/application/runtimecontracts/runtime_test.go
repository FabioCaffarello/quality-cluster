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
		Capabilities:  []string{configdomain.RuntimeCapabilityRuleNotEmpty, configdomain.RuntimeCapabilityRuleRequired},
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

	if validatorRecord.Scope != ingestionRecord.Scope || validatorRecord.Config != ingestionRecord.Config || validatorRecord.ActivatedAt != ingestionRecord.ActivatedAt {
		t.Fatalf("expected shared runtime language core, got validator=%+v ingestion=%+v", validatorRecord, ingestionRecord)
	}
	if validatorRecord.Artifact.ID != ingestionRecord.Artifact.ID ||
		validatorRecord.Artifact.SchemaVersion != ingestionRecord.Artifact.SchemaVersion ||
		validatorRecord.Artifact.Checksum != ingestionRecord.Artifact.Checksum ||
		validatorRecord.Artifact.StorageRef != ingestionRecord.Artifact.StorageRef ||
		validatorRecord.Artifact.RuntimeLoader != ingestionRecord.Artifact.RuntimeLoader {
		t.Fatalf("expected shared runtime artifact identity, got validator=%+v ingestion=%+v", validatorRecord.Artifact, ingestionRecord.Artifact)
	}
	if len(validatorRecord.Artifact.Capabilities) != 2 || len(ingestionRecord.Artifact.Capabilities) != 2 {
		t.Fatalf("expected shared runtime language, got validator=%+v ingestion=%+v", validatorRecord, ingestionRecord)
	}
}
