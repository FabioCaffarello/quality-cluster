package runtimecontracts

import (
	"time"

	configdomain "internal/domain/configctl"
)

type ScopeRecord struct {
	Kind string `json:"kind"`
	Key  string `json:"key"`
}

type ConfigRecord struct {
	SetID              string `json:"set_id"`
	Key                string `json:"key"`
	VersionID          string `json:"version_id"`
	Version            int    `json:"version"`
	DefinitionChecksum string `json:"definition_checksum"`
}

type ArtifactRecord struct {
	ID            string `json:"id"`
	SchemaVersion string `json:"schema_version"`
	Checksum      string `json:"checksum"`
	StorageRef    string `json:"storage_ref"`
	RuntimeLoader string `json:"runtime_loader"`
}

type RuntimeRecord struct {
	Scope       ScopeRecord    `json:"scope"`
	Config      ConfigRecord   `json:"config"`
	Artifact    ArtifactRecord `json:"artifact"`
	ActivatedAt time.Time      `json:"activated_at"`
}

func RecordFromProjection(projection configdomain.RuntimeProjection) RuntimeRecord {
	return RuntimeRecord{
		Scope: ScopeRecord{
			Kind: projection.Scope.Kind,
			Key:  projection.Scope.Key,
		},
		Config: ConfigRecord{
			SetID:              projection.ConfigSetID,
			Key:                projection.ConfigKey,
			VersionID:          projection.VersionID,
			Version:            projection.Version,
			DefinitionChecksum: projection.DefinitionChecksum,
		},
		Artifact: ArtifactRecord{
			ID:            projection.Artifact.ID,
			SchemaVersion: projection.Artifact.SchemaVersion,
			Checksum:      projection.Artifact.Checksum,
			StorageRef:    projection.Artifact.StorageRef,
			RuntimeLoader: projection.Artifact.RuntimeLoader,
		},
		ActivatedAt: projection.ActivatedAt,
	}
}

func RecordFromIngestionProjection(projection configdomain.IngestionRuntimeProjection) RuntimeRecord {
	return RuntimeRecord{
		Scope: ScopeRecord{
			Kind: projection.Scope.Kind,
			Key:  projection.Scope.Key,
		},
		Config: ConfigRecord{
			SetID:              projection.ConfigSetID,
			Key:                projection.ConfigKey,
			VersionID:          projection.VersionID,
			Version:            projection.Version,
			DefinitionChecksum: projection.DefinitionChecksum,
		},
		Artifact: ArtifactRecord{
			ID:            projection.Artifact.ID,
			SchemaVersion: projection.Artifact.SchemaVersion,
			Checksum:      projection.Artifact.Checksum,
			StorageRef:    projection.Artifact.StorageRef,
			RuntimeLoader: projection.Artifact.RuntimeLoader,
		},
		ActivatedAt: projection.ActivatedAt,
	}
}
