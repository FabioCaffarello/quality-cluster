package configctl

import (
	"fmt"
	"strings"
	"time"

	configdomain "internal/domain/configctl"
)

type configSetRecord struct {
	ID             string                `json:"id"`
	Key            string                `json:"key"`
	CurrentVersion int                   `json:"current_version"`
	Versions       []configVersionRecord `json:"versions"`
	CreatedAt      string                `json:"created_at"`
	UpdatedAt      string                `json:"updated_at"`
}

type configVersionRecord struct {
	ID                 string                     `json:"id"`
	Number             int                        `json:"number"`
	Lifecycle          string                     `json:"lifecycle"`
	Source             configSourceRecord         `json:"source"`
	SourceChecksum     string                     `json:"source_checksum"`
	Document           *configDocumentRecord      `json:"document,omitempty"`
	DefinitionChecksum string                     `json:"definition_checksum,omitempty"`
	ValidatedAt        *string                    `json:"validated_at,omitempty"`
	Artifact           *compilationArtifactRecord `json:"artifact,omitempty"`
	CreatedAt          string                     `json:"created_at"`
	UpdatedAt          string                     `json:"updated_at"`
	RejectedReason     string                     `json:"rejected_reason,omitempty"`
}

type configSourceRecord struct {
	Format  string `json:"format"`
	Content string `json:"content"`
}

type configDocumentRecord struct {
	Metadata configMetadataRecord `json:"metadata"`
	Bindings []bindingRecord      `json:"bindings,omitempty"`
	Fields   []fieldRecord        `json:"fields,omitempty"`
	Rules    []ruleRecord         `json:"rules,omitempty"`
}

type configMetadataRecord struct {
	Name        string            `json:"name"`
	Description string            `json:"description,omitempty"`
	Labels      map[string]string `json:"labels,omitempty"`
}

type bindingRecord struct {
	Name  string `json:"name"`
	Topic string `json:"topic"`
}

type fieldRecord struct {
	Name     string `json:"name"`
	Type     string `json:"type"`
	Required bool   `json:"required,omitempty"`
}

type ruleRecord struct {
	Name          string `json:"name"`
	Field         string `json:"field"`
	Operator      string `json:"operator"`
	ExpectedValue string `json:"expected_value,omitempty"`
	Severity      string `json:"severity,omitempty"`
}

type compilationArtifactRecord struct {
	ID              string `json:"id"`
	SchemaVersion   string `json:"schema_version"`
	Checksum        string `json:"checksum"`
	StorageRef      string `json:"storage_ref"`
	RuntimeLoader   string `json:"runtime_loader"`
	Capabilities    []string `json:"capabilities,omitempty"`
	CompilerVersion string `json:"compiler_version,omitempty"`
	CreatedAt       string `json:"created_at"`
}

type activationRecord struct {
	ID            string                `json:"id"`
	ConfigSetID   string                `json:"config_set_id"`
	ConfigKey     string                `json:"config_key"`
	VersionID     string                `json:"version_id"`
	Version       int                   `json:"version"`
	ArtifactID    string                `json:"artifact_id"`
	Scope         activationScopeRecord `json:"scope"`
	ActivatedAt   string                `json:"activated_at"`
	DeactivatedAt *string               `json:"deactivated_at,omitempty"`
}

type activationScopeRecord struct {
	Kind string `json:"kind"`
	Key  string `json:"key"`
}

type ingestionRuntimeRecord struct {
	Scope              activationScopeRecord     `json:"scope"`
	ConfigSetID        string                    `json:"config_set_id"`
	ConfigKey          string                    `json:"config_key"`
	VersionID          string                    `json:"version_id"`
	Version            int                       `json:"version"`
	Artifact           compilationArtifactRecord `json:"artifact"`
	ActivatedAt        string                    `json:"activated_at"`
	Bindings           []bindingRecord           `json:"bindings,omitempty"`
	Fields             []fieldRecord             `json:"fields,omitempty"`
	DefinitionChecksum string                    `json:"definition_checksum"`
}

func newConfigSetRecord(set configdomain.ConfigSet) configSetRecord {
	record := configSetRecord{
		ID:             set.ID,
		Key:            set.Key,
		CurrentVersion: set.CurrentVersion,
		Versions:       make([]configVersionRecord, 0, len(set.Versions)),
		CreatedAt:      formatTime(set.CreatedAt),
		UpdatedAt:      formatTime(set.UpdatedAt),
	}

	for _, version := range set.Versions {
		record.Versions = append(record.Versions, newConfigVersionRecord(version))
	}

	return record
}

func (r configSetRecord) toDomain() (configdomain.ConfigSet, error) {
	if strings.TrimSpace(r.ID) == "" {
		return configdomain.ConfigSet{}, invalidRecordError("config set id is required")
	}
	if strings.TrimSpace(r.Key) == "" {
		return configdomain.ConfigSet{}, invalidRecordError("config set key is required")
	}
	createdAt, err := parseRequiredTime(r.CreatedAt, "config set created_at")
	if err != nil {
		return configdomain.ConfigSet{}, err
	}
	updatedAt, err := parseRequiredTime(r.UpdatedAt, "config set updated_at")
	if err != nil {
		return configdomain.ConfigSet{}, err
	}

	set := configdomain.ConfigSet{
		ID:             r.ID,
		Key:            r.Key,
		CurrentVersion: r.CurrentVersion,
		Versions:       make([]configdomain.ConfigVersion, 0, len(r.Versions)),
		CreatedAt:      createdAt,
		UpdatedAt:      updatedAt,
	}

	for _, version := range r.Versions {
		domainVersion, err := version.toDomain()
		if err != nil {
			return configdomain.ConfigSet{}, err
		}
		set.Versions = append(set.Versions, domainVersion)
	}

	return set, nil
}

func newConfigVersionRecord(version configdomain.ConfigVersion) configVersionRecord {
	record := configVersionRecord{
		ID:                 version.ID,
		Number:             version.Number,
		Lifecycle:          string(version.Lifecycle),
		Source:             configSourceRecord{Format: string(version.Source.Format), Content: version.Source.Content},
		SourceChecksum:     version.SourceChecksum,
		DefinitionChecksum: version.DefinitionChecksum,
		ValidatedAt:        formatTimePtr(version.ValidatedAt),
		CreatedAt:          formatTime(version.CreatedAt),
		UpdatedAt:          formatTime(version.UpdatedAt),
		RejectedReason:     version.RejectedReason,
	}

	if version.Document != nil {
		record.Document = newConfigDocumentRecord(*version.Document)
	}
	if version.Artifact != nil {
		record.Artifact = newCompilationArtifactRecord(*version.Artifact)
	}

	return record
}

func (r configVersionRecord) toDomain() (configdomain.ConfigVersion, error) {
	if strings.TrimSpace(r.ID) == "" {
		return configdomain.ConfigVersion{}, invalidRecordError("config version id is required")
	}
	createdAt, err := parseRequiredTime(r.CreatedAt, "config version created_at")
	if err != nil {
		return configdomain.ConfigVersion{}, err
	}
	updatedAt, err := parseRequiredTime(r.UpdatedAt, "config version updated_at")
	if err != nil {
		return configdomain.ConfigVersion{}, err
	}
	validatedAt, err := parseOptionalTime(r.ValidatedAt, "config version validated_at")
	if err != nil {
		return configdomain.ConfigVersion{}, err
	}

	version := configdomain.ConfigVersion{
		ID:                 r.ID,
		Number:             r.Number,
		Lifecycle:          configdomain.VersionLifecycle(r.Lifecycle),
		Source:             configdomain.ConfigSource{Format: configdomain.SourceFormat(r.Source.Format), Content: r.Source.Content},
		SourceChecksum:     r.SourceChecksum,
		DefinitionChecksum: r.DefinitionChecksum,
		ValidatedAt:        validatedAt,
		CreatedAt:          createdAt,
		UpdatedAt:          updatedAt,
		RejectedReason:     r.RejectedReason,
	}

	if r.Document != nil {
		document := r.Document.toDomain()
		version.Document = &document
	}
	if r.Artifact != nil {
		artifact, err := r.Artifact.toDomain()
		if err != nil {
			return configdomain.ConfigVersion{}, err
		}
		version.Artifact = &artifact
	}

	return version, nil
}

func newConfigDocumentRecord(document configdomain.ConfigDocument) *configDocumentRecord {
	record := &configDocumentRecord{
		Metadata: configMetadataRecord{
			Name:        document.Metadata.Name,
			Description: document.Metadata.Description,
			Labels:      cloneLabels(document.Metadata.Labels),
		},
		Bindings: make([]bindingRecord, 0, len(document.Bindings)),
		Fields:   make([]fieldRecord, 0, len(document.Fields)),
		Rules:    make([]ruleRecord, 0, len(document.Rules)),
	}

	for _, binding := range document.Bindings {
		record.Bindings = append(record.Bindings, bindingRecord{Name: binding.Name, Topic: binding.Topic})
	}
	for _, field := range document.Fields {
		record.Fields = append(record.Fields, fieldRecord{
			Name:     field.Name,
			Type:     string(field.Type),
			Required: field.Required,
		})
	}
	for _, rule := range document.Rules {
		record.Rules = append(record.Rules, ruleRecord{
			Name:          rule.Name,
			Field:         rule.Field,
			Operator:      string(rule.Operator),
			ExpectedValue: rule.ExpectedValue,
			Severity:      string(rule.Severity),
		})
	}

	return record
}

func (r configDocumentRecord) toDomain() configdomain.ConfigDocument {
	document := configdomain.ConfigDocument{
		Metadata: configdomain.ConfigMetadata{
			Name:        r.Metadata.Name,
			Description: r.Metadata.Description,
			Labels:      cloneLabels(r.Metadata.Labels),
		},
		Bindings: make([]configdomain.Binding, 0, len(r.Bindings)),
		Fields:   make([]configdomain.Field, 0, len(r.Fields)),
		Rules:    make([]configdomain.Rule, 0, len(r.Rules)),
	}

	for _, binding := range r.Bindings {
		document.Bindings = append(document.Bindings, configdomain.Binding{Name: binding.Name, Topic: binding.Topic})
	}
	for _, field := range r.Fields {
		document.Fields = append(document.Fields, configdomain.Field{
			Name:     field.Name,
			Type:     configdomain.FieldType(field.Type),
			Required: field.Required,
		})
	}
	for _, rule := range r.Rules {
		document.Rules = append(document.Rules, configdomain.Rule{
			Name:          rule.Name,
			Field:         rule.Field,
			Operator:      configdomain.RuleOperator(rule.Operator),
			ExpectedValue: rule.ExpectedValue,
			Severity:      configdomain.RuleSeverity(rule.Severity),
		})
	}

	return document
}

func newCompilationArtifactRecord(artifact configdomain.CompilationArtifact) *compilationArtifactRecord {
	return &compilationArtifactRecord{
		ID:              artifact.ID,
		SchemaVersion:   artifact.SchemaVersion,
		Checksum:        artifact.Checksum,
		StorageRef:      artifact.StorageRef,
		RuntimeLoader:   artifact.RuntimeLoader,
		Capabilities:    append([]string(nil), artifact.NormalizedCapabilities()...),
		CompilerVersion: artifact.CompilerVersion,
		CreatedAt:       formatTime(artifact.CreatedAt),
	}
}

func (r compilationArtifactRecord) toDomain() (configdomain.CompilationArtifact, error) {
	if strings.TrimSpace(r.ID) == "" {
		return configdomain.CompilationArtifact{}, invalidRecordError("artifact id is required")
	}
	createdAt, err := parseRequiredTime(r.CreatedAt, "artifact created_at")
	if err != nil {
		return configdomain.CompilationArtifact{}, err
	}

	return configdomain.CompilationArtifact{
		ID:              r.ID,
		SchemaVersion:   r.SchemaVersion,
		Checksum:        r.Checksum,
		StorageRef:      r.StorageRef,
		RuntimeLoader:   r.RuntimeLoader,
		Capabilities:    configdomain.CompilationArtifact{Capabilities: r.Capabilities}.NormalizedCapabilities(),
		CompilerVersion: r.CompilerVersion,
		CreatedAt:       createdAt,
	}, nil
}

func newActivationRecord(activation configdomain.Activation) activationRecord {
	scope := activation.Scope.Normalize()
	return activationRecord{
		ID:          activation.ID,
		ConfigSetID: activation.ConfigSetID,
		ConfigKey:   activation.ConfigKey,
		VersionID:   activation.VersionID,
		Version:     activation.Version,
		ArtifactID:  activation.ArtifactID,
		Scope:       activationScopeRecord{Kind: scope.Kind, Key: scope.Key},
		ActivatedAt: formatTime(activation.ActivatedAt),
		DeactivatedAt: formatTimePtr(
			activation.DeactivatedAt,
		),
	}
}

func (r activationRecord) toDomain() (configdomain.Activation, error) {
	if strings.TrimSpace(r.ID) == "" {
		return configdomain.Activation{}, invalidRecordError("activation id is required")
	}
	activatedAt, err := parseRequiredTime(r.ActivatedAt, "activation activated_at")
	if err != nil {
		return configdomain.Activation{}, err
	}
	deactivatedAt, err := parseOptionalTime(r.DeactivatedAt, "activation deactivated_at")
	if err != nil {
		return configdomain.Activation{}, err
	}

	return configdomain.Activation{
		ID:            r.ID,
		ConfigSetID:   r.ConfigSetID,
		ConfigKey:     r.ConfigKey,
		VersionID:     r.VersionID,
		Version:       r.Version,
		ArtifactID:    r.ArtifactID,
		Scope:         configdomain.ActivationScope{Kind: r.Scope.Kind, Key: r.Scope.Key}.Normalize(),
		ActivatedAt:   activatedAt,
		DeactivatedAt: deactivatedAt,
	}, nil
}

func (r activationRecord) isActive() bool {
	return r.DeactivatedAt == nil
}

func newIngestionRuntimeRecord(runtime configdomain.IngestionRuntimeProjection) ingestionRuntimeRecord {
	scope := runtime.Scope.Normalize()
	record := ingestionRuntimeRecord{
		Scope:       activationScopeRecord{Kind: scope.Kind, Key: scope.Key},
		ConfigSetID: runtime.ConfigSetID,
		ConfigKey:   runtime.ConfigKey,
		VersionID:   runtime.VersionID,
		Version:     runtime.Version,
		Artifact: compilationArtifactRecord{
			ID:              runtime.Artifact.ID,
			SchemaVersion:   runtime.Artifact.SchemaVersion,
			Checksum:        runtime.Artifact.Checksum,
			StorageRef:      runtime.Artifact.StorageRef,
			RuntimeLoader:   runtime.Artifact.RuntimeLoader,
			Capabilities:    append([]string(nil), runtime.Artifact.NormalizedCapabilities()...),
			CompilerVersion: runtime.Artifact.CompilerVersion,
			CreatedAt:       formatTime(runtime.Artifact.CreatedAt),
		},
		ActivatedAt:        formatTime(runtime.ActivatedAt),
		Bindings:           make([]bindingRecord, 0, len(runtime.Bindings)),
		Fields:             make([]fieldRecord, 0, len(runtime.Fields)),
		DefinitionChecksum: runtime.DefinitionChecksum,
	}
	for _, binding := range runtime.Bindings {
		record.Bindings = append(record.Bindings, bindingRecord{Name: binding.Name, Topic: binding.Topic})
	}
	for _, field := range runtime.Fields {
		record.Fields = append(record.Fields, fieldRecord{
			Name:     field.Name,
			Type:     string(field.Type),
			Required: field.Required,
		})
	}
	return record
}

func (r ingestionRuntimeRecord) toDomain() (configdomain.IngestionRuntimeProjection, error) {
	if strings.TrimSpace(r.ConfigSetID) == "" {
		return configdomain.IngestionRuntimeProjection{}, invalidRecordError("ingestion runtime config_set_id is required")
	}
	if strings.TrimSpace(r.ConfigKey) == "" {
		return configdomain.IngestionRuntimeProjection{}, invalidRecordError("ingestion runtime config_key is required")
	}
	if strings.TrimSpace(r.VersionID) == "" {
		return configdomain.IngestionRuntimeProjection{}, invalidRecordError("ingestion runtime version_id is required")
	}
	activatedAt, err := parseRequiredTime(r.ActivatedAt, "ingestion runtime activated_at")
	if err != nil {
		return configdomain.IngestionRuntimeProjection{}, err
	}
	artifact, err := r.Artifact.toDomain()
	if err != nil {
		return configdomain.IngestionRuntimeProjection{}, err
	}

	runtime := configdomain.IngestionRuntimeProjection{
		Scope:              configdomain.ActivationScope{Kind: r.Scope.Kind, Key: r.Scope.Key}.Normalize(),
		ConfigSetID:        r.ConfigSetID,
		ConfigKey:          r.ConfigKey,
		VersionID:          r.VersionID,
		Version:            r.Version,
		Artifact:           artifact,
		ActivatedAt:        activatedAt,
		Bindings:           make([]configdomain.Binding, 0, len(r.Bindings)),
		Fields:             make([]configdomain.Field, 0, len(r.Fields)),
		DefinitionChecksum: r.DefinitionChecksum,
	}
	for _, binding := range r.Bindings {
		runtime.Bindings = append(runtime.Bindings, configdomain.Binding{Name: binding.Name, Topic: binding.Topic})
	}
	for _, field := range r.Fields {
		runtime.Fields = append(runtime.Fields, configdomain.Field{
			Name:     field.Name,
			Type:     configdomain.FieldType(field.Type),
			Required: field.Required,
		})
	}

	return runtime, nil
}

func cloneLabels(labels map[string]string) map[string]string {
	if len(labels) == 0 {
		return nil
	}
	cloned := make(map[string]string, len(labels))
	for key, value := range labels {
		cloned[key] = value
	}
	return cloned
}

func formatTime(value time.Time) string {
	if value.IsZero() {
		return ""
	}
	return value.UTC().Format(time.RFC3339Nano)
}

func formatTimePtr(value *time.Time) *string {
	if value == nil || value.IsZero() {
		return nil
	}
	formatted := value.UTC().Format(time.RFC3339Nano)
	return &formatted
}

func parseRequiredTime(raw, field string) (time.Time, error) {
	if strings.TrimSpace(raw) == "" {
		return time.Time{}, invalidRecordError("%s is required", field)
	}
	parsed, err := time.Parse(time.RFC3339Nano, raw)
	if err != nil {
		return time.Time{}, invalidRecordError("%s is invalid: %v", field, err)
	}
	return parsed.UTC(), nil
}

func parseOptionalTime(raw *string, field string) (*time.Time, error) {
	if raw == nil {
		return nil, nil
	}
	parsed, err := parseRequiredTime(*raw, field)
	if err != nil {
		return nil, err
	}
	return &parsed, nil
}

func invalidRecordError(format string, args ...any) error {
	return fmt.Errorf("invalid persisted record: "+format, args...)
}
