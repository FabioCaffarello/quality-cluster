package configctl

import "internal/shared/events"

type DomainEvent interface {
	events.Event
}

const (
	EventDraftCreated            events.Name = "config.draft_created"
	EventValidated               events.Name = "config.validated"
	EventCompiled                events.Name = "config.compiled"
	EventActivated               events.Name = "config.activated"
	EventDeactivated             events.Name = "config.deactivated"
	EventIngestionRuntimeChanged events.Name = "config.ingestion_runtime_changed"
	EventArchived                events.Name = "config.archived"
	EventRejected                events.Name = "config.rejected"
)

type IngestionRuntimeChangeType string

const (
	IngestionRuntimeChangeActivated IngestionRuntimeChangeType = "activated"
	IngestionRuntimeChangeCleared   IngestionRuntimeChangeType = "cleared"
)

type DraftCreatedEvent struct {
	Metadata     events.Metadata `json:"metadata"`
	ConfigSetID  string          `json:"config_set_id"`
	ConfigKey    string          `json:"config_key"`
	VersionID    string          `json:"version_id"`
	Version      int             `json:"version"`
	SourceFormat string          `json:"source_format"`
}

type ConfigValidatedEvent struct {
	Metadata           events.Metadata `json:"metadata"`
	ConfigSetID        string          `json:"config_set_id"`
	ConfigKey          string          `json:"config_key"`
	VersionID          string          `json:"version_id"`
	Version            int             `json:"version"`
	DefinitionChecksum string          `json:"definition_checksum"`
}

type ConfigCompiledEvent struct {
	Metadata    events.Metadata     `json:"metadata"`
	ConfigSetID string              `json:"config_set_id"`
	ConfigKey   string              `json:"config_key"`
	VersionID   string              `json:"version_id"`
	Version     int                 `json:"version"`
	Artifact    CompilationArtifact `json:"artifact"`
}

type ConfigActivatedEvent struct {
	Metadata    events.Metadata   `json:"metadata"`
	ConfigSetID string            `json:"config_set_id"`
	ConfigKey   string            `json:"config_key"`
	VersionID   string            `json:"version_id"`
	Version     int               `json:"version"`
	Activation  Activation        `json:"activation"`
	Projection  RuntimeProjection `json:"projection"`
}

type ConfigDeactivatedEvent struct {
	Metadata    events.Metadata `json:"metadata"`
	ConfigSetID string          `json:"config_set_id"`
	ConfigKey   string          `json:"config_key"`
	VersionID   string          `json:"version_id"`
	Version     int             `json:"version"`
	Activation  Activation      `json:"activation"`
	Scope       ActivationScope `json:"scope"`
}

type IngestionRuntimeChangedEvent struct {
	Metadata   events.Metadata             `json:"metadata"`
	ChangeType IngestionRuntimeChangeType  `json:"change_type"`
	Scope      ActivationScope             `json:"scope"`
	Runtime    *IngestionRuntimeProjection `json:"runtime,omitempty"`
}

type ConfigArchivedEvent struct {
	Metadata    events.Metadata `json:"metadata"`
	ConfigSetID string          `json:"config_set_id"`
	ConfigKey   string          `json:"config_key"`
	VersionID   string          `json:"version_id"`
	Version     int             `json:"version"`
}

type ConfigRejectedEvent struct {
	Metadata    events.Metadata `json:"metadata"`
	ConfigSetID string          `json:"config_set_id"`
	ConfigKey   string          `json:"config_key"`
	VersionID   string          `json:"version_id"`
	Version     int             `json:"version"`
	Reason      string          `json:"reason,omitempty"`
}

func (e DraftCreatedEvent) EventName() events.Name                    { return EventDraftCreated }
func (e DraftCreatedEvent) EventMetadata() events.Metadata            { return e.Metadata }
func (e ConfigValidatedEvent) EventName() events.Name                 { return EventValidated }
func (e ConfigValidatedEvent) EventMetadata() events.Metadata         { return e.Metadata }
func (e ConfigCompiledEvent) EventName() events.Name                  { return EventCompiled }
func (e ConfigCompiledEvent) EventMetadata() events.Metadata          { return e.Metadata }
func (e ConfigActivatedEvent) EventName() events.Name                 { return EventActivated }
func (e ConfigActivatedEvent) EventMetadata() events.Metadata         { return e.Metadata }
func (e ConfigDeactivatedEvent) EventName() events.Name               { return EventDeactivated }
func (e ConfigDeactivatedEvent) EventMetadata() events.Metadata       { return e.Metadata }
func (e IngestionRuntimeChangedEvent) EventName() events.Name         { return EventIngestionRuntimeChanged }
func (e IngestionRuntimeChangedEvent) EventMetadata() events.Metadata { return e.Metadata }
func (e ConfigArchivedEvent) EventName() events.Name                  { return EventArchived }
func (e ConfigArchivedEvent) EventMetadata() events.Metadata          { return e.Metadata }
func (e ConfigRejectedEvent) EventName() events.Name                  { return EventRejected }
func (e ConfigRejectedEvent) EventMetadata() events.Metadata          { return e.Metadata }
