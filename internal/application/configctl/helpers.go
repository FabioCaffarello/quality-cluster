package configctl

import (
	"context"
	"time"

	configdomain "internal/domain/configctl"
	"internal/shared/events"
	"internal/shared/problem"
	"internal/shared/requestctx"
)

func publishEvents(ctx context.Context, publisher DomainEventPublisher, pending []events.Event) *problem.Problem {
	if publisher == nil || len(pending) == 0 {
		return nil
	}
	for _, event := range pending {
		if prob := publisher.Publish(ctx, withCorrelation(event, requestctx.CorrelationID(ctx))); prob != nil {
			return prob
		}
	}
	return nil
}

func withCorrelation(event events.Event, correlationID string) events.Event {
	switch typed := event.(type) {
	case configdomain.DraftCreatedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigValidatedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigCompiledEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigActivatedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigDeactivatedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.IngestionRuntimeChangedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigArchivedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	case configdomain.ConfigRejectedEvent:
		typed.Metadata = typed.Metadata.WithCorrelationID(correlationID)
		return typed
	default:
		return event
	}
}

func snapshotConfigSet(set configdomain.ConfigSet) configdomain.ConfigSet {
	snapshot := configdomain.ConfigSet{
		ID:             set.ID,
		Key:            set.Key,
		CurrentVersion: set.CurrentVersion,
		Versions:       make([]configdomain.ConfigVersion, 0, len(set.Versions)),
		CreatedAt:      set.CreatedAt,
		UpdatedAt:      set.UpdatedAt,
	}

	for _, version := range set.Versions {
		snapshot.Versions = append(snapshot.Versions, snapshotConfigVersion(version))
	}

	return snapshot
}

func snapshotConfigVersion(version configdomain.ConfigVersion) configdomain.ConfigVersion {
	snapshot := configdomain.ConfigVersion{
		ID:                 version.ID,
		Number:             version.Number,
		Lifecycle:          version.Lifecycle,
		Source:             version.Source,
		SourceChecksum:     version.SourceChecksum,
		DefinitionChecksum: version.DefinitionChecksum,
		ValidatedAt:        cloneTimePtr(version.ValidatedAt),
		CreatedAt:          version.CreatedAt,
		UpdatedAt:          version.UpdatedAt,
		RejectedReason:     version.RejectedReason,
	}

	if version.Document != nil {
		document := snapshotConfigDocument(*version.Document)
		snapshot.Document = &document
	}
	if version.Artifact != nil {
		artifact := *version.Artifact
		snapshot.Artifact = &artifact
	}

	return snapshot
}

func snapshotConfigDocument(document configdomain.ConfigDocument) configdomain.ConfigDocument {
	snapshot := configdomain.ConfigDocument{
		Metadata: configdomain.ConfigMetadata{
			Name:        document.Metadata.Name,
			Description: document.Metadata.Description,
			Labels:      cloneStringMap(document.Metadata.Labels),
		},
		Bindings: append([]configdomain.Binding(nil), document.Bindings...),
		Fields:   append([]configdomain.Field(nil), document.Fields...),
		Rules:    append([]configdomain.Rule(nil), document.Rules...),
	}

	return snapshot
}

func cloneStringMap(source map[string]string) map[string]string {
	if len(source) == 0 {
		return nil
	}
	cloned := make(map[string]string, len(source))
	for key, value := range source {
		cloned[key] = value
	}
	return cloned
}

func cloneTimePtr(value *time.Time) *time.Time {
	if value == nil {
		return nil
	}
	cloned := value.UTC()
	return &cloned
}

func snapshotIngestionRuntime(runtime configdomain.IngestionRuntimeProjection) configdomain.IngestionRuntimeProjection {
	snapshot := configdomain.IngestionRuntimeProjection{
		Scope: configdomain.ActivationScope{
			Kind: runtime.Scope.Kind,
			Key:  runtime.Scope.Key,
		}.Normalize(),
		ConfigSetID:        runtime.ConfigSetID,
		ConfigKey:          runtime.ConfigKey,
		VersionID:          runtime.VersionID,
		Version:            runtime.Version,
		Artifact:           runtime.Artifact,
		ActivatedAt:        runtime.ActivatedAt,
		Bindings:           append([]configdomain.Binding(nil), runtime.Bindings...),
		Fields:             append([]configdomain.Field(nil), runtime.Fields...),
		DefinitionChecksum: runtime.DefinitionChecksum,
	}

	return snapshot
}
