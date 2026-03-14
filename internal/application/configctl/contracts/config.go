package contracts

import (
	"time"

	sharedruntime "internal/application/runtimecontracts"
)

type ConfigVersionSummary struct {
	ID                 string                            `json:"id"`
	ConfigSetID        string                            `json:"config_set_id"`
	ConfigKey          string                            `json:"config_key"`
	Version            int                               `json:"version"`
	Format             string                            `json:"format"`
	Lifecycle          string                            `json:"lifecycle"`
	SourceChecksum     string                            `json:"source_checksum"`
	DefinitionChecksum string                            `json:"definition_checksum,omitempty"`
	Artifact           *CompilationArtifactSummaryRecord `json:"artifact,omitempty"`
	ActiveScopes       []ActivationScopeRecord           `json:"active_scopes,omitempty"`
	CreatedAt          time.Time                         `json:"created_at"`
	UpdatedAt          time.Time                         `json:"updated_at"`
	ValidatedAt        *time.Time                        `json:"validated_at,omitempty"`
}

type ConfigVersionDetail struct {
	ID                 string                     `json:"id"`
	ConfigSetID        string                     `json:"config_set_id"`
	ConfigKey          string                     `json:"config_key"`
	Version            int                        `json:"version"`
	Format             string                     `json:"format"`
	Source             string                     `json:"source"`
	Lifecycle          string                     `json:"lifecycle"`
	SourceChecksum     string                     `json:"source_checksum"`
	DefinitionChecksum string                     `json:"definition_checksum,omitempty"`
	Metadata           *ConfigMetadataRecord      `json:"metadata,omitempty"`
	Bindings           []BindingRecord            `json:"bindings,omitempty"`
	Fields             []FieldRecord              `json:"fields,omitempty"`
	Rules              []RuleRecord               `json:"rules,omitempty"`
	Artifact           *CompilationArtifactRecord `json:"artifact,omitempty"`
	ActiveScopes       []ActivationScopeRecord    `json:"active_scopes,omitempty"`
	RejectedReason     string                     `json:"rejected_reason,omitempty"`
	CreatedAt          time.Time                  `json:"created_at"`
	UpdatedAt          time.Time                  `json:"updated_at"`
	ValidatedAt        *time.Time                 `json:"validated_at,omitempty"`
}

type CompilationArtifactSummaryRecord struct {
	ID            string `json:"id"`
	SchemaVersion string `json:"schema_version"`
	Checksum      string `json:"checksum"`
	StorageRef    string `json:"storage_ref"`
	RuntimeLoader string `json:"runtime_loader"`
}

type ConfigMetadataRecord struct {
	Name        string            `json:"name"`
	Description string            `json:"description,omitempty"`
	Labels      map[string]string `json:"labels,omitempty"`
}

type BindingRecord struct {
	Name  string `json:"name"`
	Topic string `json:"topic"`
}

type FieldRecord struct {
	Name     string `json:"name"`
	Type     string `json:"type"`
	Required bool   `json:"required,omitempty"`
}

type RuleRecord struct {
	Name          string `json:"name"`
	Field         string `json:"field"`
	Operator      string `json:"operator"`
	ExpectedValue string `json:"expected_value,omitempty"`
	Severity      string `json:"severity,omitempty"`
}

type CompilationArtifactRecord struct {
	ID              string    `json:"id"`
	SchemaVersion   string    `json:"schema_version"`
	Checksum        string    `json:"checksum"`
	StorageRef      string    `json:"storage_ref"`
	RuntimeLoader   string    `json:"runtime_loader"`
	CompilerVersion string    `json:"compiler_version,omitempty"`
	CreatedAt       time.Time `json:"created_at"`
}

type ActivationScopeRecord struct {
	Kind string `json:"kind"`
	Key  string `json:"key"`
}

type ActivationRecord struct {
	ID            string                `json:"id"`
	ConfigSetID   string                `json:"config_set_id"`
	ConfigKey     string                `json:"config_key"`
	VersionID     string                `json:"version_id"`
	Version       int                   `json:"version"`
	ArtifactID    string                `json:"artifact_id"`
	Scope         ActivationScopeRecord `json:"scope"`
	ActivatedAt   time.Time             `json:"activated_at"`
	DeactivatedAt *time.Time            `json:"deactivated_at,omitempty"`
}

type RuntimeProjectionRecord struct {
	Scope              ActivationScopeRecord     `json:"scope"`
	ConfigSetID        string                    `json:"config_set_id"`
	ConfigKey          string                    `json:"config_key"`
	VersionID          string                    `json:"version_id"`
	Version            int                       `json:"version"`
	Artifact           CompilationArtifactRecord `json:"artifact"`
	ActivatedAt        time.Time                 `json:"activated_at"`
	Bindings           []BindingRecord           `json:"bindings,omitempty"`
	Fields             []FieldRecord             `json:"fields,omitempty"`
	Rules              []RuleRecord              `json:"rules,omitempty"`
	DefinitionChecksum string                    `json:"definition_checksum"`
}

type ActiveIngestionBindingRecord struct {
	Binding BindingRecord               `json:"binding"`
	Fields  []FieldRecord               `json:"fields,omitempty"`
	Runtime sharedruntime.RuntimeRecord `json:"runtime"`
}
