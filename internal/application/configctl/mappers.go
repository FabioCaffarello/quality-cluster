package configctl

import (
	"sort"

	"internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	configdomain "internal/domain/configctl"
)

func detailRecordFromDomain(set configdomain.ConfigSet, version configdomain.ConfigVersion, activations []configdomain.Activation) contracts.ConfigVersionDetail {
	record := contracts.ConfigVersionDetail{
		ID:                 version.ID,
		ConfigSetID:        set.ID,
		ConfigKey:          set.Key,
		Version:            version.Number,
		Format:             string(version.Source.Format),
		Source:             version.Source.Content,
		Lifecycle:          string(version.Lifecycle),
		SourceChecksum:     version.SourceChecksum,
		DefinitionChecksum: version.DefinitionChecksum,
		RejectedReason:     version.RejectedReason,
		CreatedAt:          version.CreatedAt,
		UpdatedAt:          version.UpdatedAt,
		ValidatedAt:        version.ValidatedAt,
	}
	record.ActiveScopes = activeScopesFromDomain(activations)

	if version.Document != nil {
		record.Metadata = &contracts.ConfigMetadataRecord{
			Name:        version.Document.Metadata.Name,
			Description: version.Document.Metadata.Description,
			Labels:      version.Document.Metadata.Labels,
		}
		for _, binding := range version.Document.Bindings {
			record.Bindings = append(record.Bindings, contracts.BindingRecord{
				Name:  binding.Name,
				Topic: binding.Topic,
			})
		}
		for _, field := range version.Document.Fields {
			record.Fields = append(record.Fields, contracts.FieldRecord{
				Name:     field.Name,
				Type:     string(field.Type),
				Required: field.Required,
			})
		}
		for _, rule := range version.Document.Rules {
			record.Rules = append(record.Rules, contracts.RuleRecord{
				Name:          rule.Name,
				Field:         rule.Field,
				Operator:      string(rule.Operator),
				ExpectedValue: rule.ExpectedValue,
				Severity:      string(rule.Severity),
			})
		}
	}

	if version.Artifact != nil {
		record.Artifact = &contracts.CompilationArtifactRecord{
			ID:              version.Artifact.ID,
			SchemaVersion:   version.Artifact.SchemaVersion,
			Checksum:        version.Artifact.Checksum,
			StorageRef:      version.Artifact.StorageRef,
			RuntimeLoader:   version.Artifact.RuntimeLoader,
			Capabilities:    append([]string(nil), version.Artifact.NormalizedCapabilities()...),
			CompilerVersion: version.Artifact.CompilerVersion,
			CreatedAt:       version.Artifact.CreatedAt,
		}
	}

	return record
}

func summaryRecordFromDomain(set configdomain.ConfigSet, version configdomain.ConfigVersion, activations []configdomain.Activation) contracts.ConfigVersionSummary {
	record := contracts.ConfigVersionSummary{
		ID:                 version.ID,
		ConfigSetID:        set.ID,
		ConfigKey:          set.Key,
		Version:            version.Number,
		Format:             string(version.Source.Format),
		Lifecycle:          string(version.Lifecycle),
		SourceChecksum:     version.SourceChecksum,
		DefinitionChecksum: version.DefinitionChecksum,
		CreatedAt:          version.CreatedAt,
		UpdatedAt:          version.UpdatedAt,
		ValidatedAt:        version.ValidatedAt,
	}
	record.ActiveScopes = activeScopesFromDomain(activations)

	if version.Artifact != nil {
		record.Artifact = &contracts.CompilationArtifactSummaryRecord{
			ID:            version.Artifact.ID,
			SchemaVersion: version.Artifact.SchemaVersion,
			Checksum:      version.Artifact.Checksum,
			StorageRef:    version.Artifact.StorageRef,
			RuntimeLoader: version.Artifact.RuntimeLoader,
			Capabilities:  append([]string(nil), version.Artifact.NormalizedCapabilities()...),
		}
	}

	return record
}

func activeScopesFromDomain(activations []configdomain.Activation) []contracts.ActivationScopeRecord {
	if len(activations) == 0 {
		return nil
	}
	scopes := make([]contracts.ActivationScopeRecord, 0, len(activations))
	for _, activation := range activations {
		if !activation.IsActive() {
			continue
		}
		scopes = append(scopes, contracts.ActivationScopeRecord{
			Kind: activation.Scope.Kind,
			Key:  activation.Scope.Key,
		})
	}
	return scopes
}

func activationRecordFromDomain(activation configdomain.Activation) contracts.ActivationRecord {
	return contracts.ActivationRecord{
		ID:          activation.ID,
		ConfigSetID: activation.ConfigSetID,
		ConfigKey:   activation.ConfigKey,
		VersionID:   activation.VersionID,
		Version:     activation.Version,
		ArtifactID:  activation.ArtifactID,
		Scope: contracts.ActivationScopeRecord{
			Kind: activation.Scope.Kind,
			Key:  activation.Scope.Key,
		},
		ActivatedAt:   activation.ActivatedAt,
		DeactivatedAt: activation.DeactivatedAt,
	}
}

func projectionRecordFromDomain(projection configdomain.RuntimeProjection) contracts.RuntimeProjectionRecord {
	record := contracts.RuntimeProjectionRecord{
		Scope: contracts.ActivationScopeRecord{
			Kind: projection.Scope.Kind,
			Key:  projection.Scope.Key,
		},
		ConfigSetID: projection.ConfigSetID,
		ConfigKey:   projection.ConfigKey,
		VersionID:   projection.VersionID,
		Version:     projection.Version,
		Artifact: contracts.CompilationArtifactRecord{
			ID:              projection.Artifact.ID,
			SchemaVersion:   projection.Artifact.SchemaVersion,
			Checksum:        projection.Artifact.Checksum,
			StorageRef:      projection.Artifact.StorageRef,
			RuntimeLoader:   projection.Artifact.RuntimeLoader,
			Capabilities:    append([]string(nil), projection.Artifact.NormalizedCapabilities()...),
			CompilerVersion: projection.Artifact.CompilerVersion,
			CreatedAt:       projection.Artifact.CreatedAt,
		},
		ActivatedAt:        projection.ActivatedAt,
		DefinitionChecksum: projection.DefinitionChecksum,
	}

	for _, binding := range projection.Bindings {
		record.Bindings = append(record.Bindings, contracts.BindingRecord{
			Name:  binding.Name,
			Topic: binding.Topic,
		})
	}
	for _, field := range projection.Fields {
		record.Fields = append(record.Fields, contracts.FieldRecord{
			Name:     field.Name,
			Type:     string(field.Type),
			Required: field.Required,
		})
	}
	for _, rule := range projection.Rules {
		record.Rules = append(record.Rules, contracts.RuleRecord{
			Name:          rule.Name,
			Field:         rule.Field,
			Operator:      string(rule.Operator),
			ExpectedValue: rule.ExpectedValue,
			Severity:      string(rule.Severity),
		})
	}

	return record
}

func activeIngestionBindingsFromDomain(runtimes []configdomain.IngestionRuntimeProjection) []contracts.ActiveIngestionBindingRecord {
	if len(runtimes) == 0 {
		return nil
	}

	bindings := make([]contracts.ActiveIngestionBindingRecord, 0)
	for _, runtime := range runtimes {
		for _, binding := range runtime.Bindings {
			record := contracts.ActiveIngestionBindingRecord{
				Binding: contracts.BindingRecord{
					Name:  binding.Name,
					Topic: binding.Topic,
				},
				Runtime: sharedruntime.RecordFromIngestionProjection(runtime),
			}
			for _, field := range runtime.Fields {
				record.Fields = append(record.Fields, contracts.FieldRecord{
					Name:     field.Name,
					Type:     string(field.Type),
					Required: field.Required,
				})
			}
			bindings = append(bindings, record)
		}
	}

	sort.SliceStable(bindings, func(i, j int) bool {
		left := bindings[i]
		right := bindings[j]
		leftScope := left.Runtime.Scope.Kind + ":" + left.Runtime.Scope.Key
		rightScope := right.Runtime.Scope.Kind + ":" + right.Runtime.Scope.Key
		if leftScope != rightScope {
			return leftScope < rightScope
		}
		if left.Binding.Topic != right.Binding.Topic {
			return left.Binding.Topic < right.Binding.Topic
		}
		if left.Binding.Name != right.Binding.Name {
			return left.Binding.Name < right.Binding.Name
		}
		if !left.Runtime.ActivatedAt.Equal(right.Runtime.ActivatedAt) {
			return left.Runtime.ActivatedAt.Before(right.Runtime.ActivatedAt)
		}
		return left.Runtime.Config.VersionID < right.Runtime.Config.VersionID
	})

	return bindings
}

func compactIngestionRuntimesFromDomain(runtimes []configdomain.IngestionRuntimeProjection) []sharedruntime.RuntimeRecord {
	if len(runtimes) == 0 {
		return nil
	}

	records := make([]sharedruntime.RuntimeRecord, 0, len(runtimes))
	for _, runtime := range runtimes {
		records = append(records, sharedruntime.RecordFromIngestionProjection(runtime))
	}

	sort.SliceStable(records, func(i, j int) bool {
		left := records[i]
		right := records[j]
		if left.Scope.Kind != right.Scope.Kind {
			return left.Scope.Kind < right.Scope.Kind
		}
		if left.Scope.Key != right.Scope.Key {
			return left.Scope.Key < right.Scope.Key
		}
		if left.Config.Key != right.Config.Key {
			return left.Config.Key < right.Config.Key
		}
		if left.Config.VersionID != right.Config.VersionID {
			return left.Config.VersionID < right.Config.VersionID
		}
		return left.ActivatedAt.Before(right.ActivatedAt)
	})

	return records
}
