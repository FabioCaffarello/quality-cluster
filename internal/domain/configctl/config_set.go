package configctl

import (
	"strings"
	"time"

	"internal/shared/events"
	"internal/shared/problem"
)

type ConfigVersion struct {
	ID                 string
	Number             int
	Lifecycle          VersionLifecycle
	Source             ConfigSource
	SourceChecksum     string
	Document           *ConfigDocument
	DefinitionChecksum string
	ValidatedAt        *time.Time
	Artifact           *CompilationArtifact
	CreatedAt          time.Time
	UpdatedAt          time.Time
	RejectedReason     string
}

type ConfigSet struct {
	ID             string
	Key            string
	CurrentVersion int
	Versions       []ConfigVersion
	CreatedAt      time.Time
	UpdatedAt      time.Time
	pendingEvents  []events.Event
}

func NewConfigSet(setID, key, versionID string, source ConfigSource, createdAt time.Time) (ConfigSet, *problem.Problem) {
	key = strings.TrimSpace(key)
	source = source.Normalize()
	createdAt = createdAt.UTC()

	var issues []problem.ValidationIssue
	if strings.TrimSpace(setID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "config_set_id", Message: "must not be empty"})
	}
	if key == "" {
		issues = append(issues, problem.ValidationIssue{Field: "config_key", Message: "must not be empty"})
	}
	if strings.TrimSpace(versionID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "version_id", Message: "must not be empty"})
	}
	if createdAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "created_at", Message: "must not be zero"})
	}
	if prob := source.ValidateForDraft(); prob != nil {
		issues = append(issues, validationIssues(prob)...)
	}
	if len(issues) > 0 {
		return ConfigSet{}, problem.Validation(problem.InvalidArgument, "config set is invalid", issues...)
	}

	version := ConfigVersion{
		ID:             strings.TrimSpace(versionID),
		Number:         1,
		Lifecycle:      LifecycleDraft,
		Source:         source,
		SourceChecksum: source.Checksum(),
		CreatedAt:      createdAt,
		UpdatedAt:      createdAt,
	}

	set := ConfigSet{
		ID:             strings.TrimSpace(setID),
		Key:            key,
		CurrentVersion: 1,
		Versions:       []ConfigVersion{version},
		CreatedAt:      createdAt,
		UpdatedAt:      createdAt,
	}

	set.recordEvent(DraftCreatedEvent{
		Metadata:     events.NewMetadata().WithOccurredAt(createdAt),
		ConfigSetID:  set.ID,
		ConfigKey:    set.Key,
		VersionID:    version.ID,
		Version:      version.Number,
		SourceFormat: string(version.Source.Format),
	})

	return set, nil
}

func (s *ConfigSet) CreateDraftVersion(versionID string, source ConfigSource, createdAt time.Time) *problem.Problem {
	if s == nil {
		return problem.New(problem.Unavailable, "config set is unavailable")
	}
	if s.hasOpenCandidate() {
		return problem.New(problem.Conflict, "config set already has an open draft")
	}
	if s.hasVersionID(versionID) {
		return problem.New(problem.Conflict, "config version id already exists")
	}
	if prob := source.ValidateForDraft(); prob != nil {
		return prob
	}

	createdAt = createdAt.UTC()
	if createdAt.IsZero() {
		return problem.Validation(problem.InvalidArgument, "draft version is invalid", problem.ValidationIssue{
			Field:   "created_at",
			Message: "must not be zero",
		})
	}

	version := ConfigVersion{
		ID:             strings.TrimSpace(versionID),
		Number:         s.CurrentVersion + 1,
		Lifecycle:      LifecycleDraft,
		Source:         source.Normalize(),
		SourceChecksum: source.Checksum(),
		CreatedAt:      createdAt,
		UpdatedAt:      createdAt,
	}

	s.CurrentVersion = version.Number
	s.Versions = append(s.Versions, version)
	s.UpdatedAt = createdAt
	s.recordEvent(DraftCreatedEvent{
		Metadata:     events.NewMetadata().WithOccurredAt(createdAt),
		ConfigSetID:  s.ID,
		ConfigKey:    s.Key,
		VersionID:    version.ID,
		Version:      version.Number,
		SourceFormat: string(version.Source.Format),
	})

	return nil
}

func (s *ConfigSet) ValidateVersion(versionID string, validatedAt time.Time) ([]ValidationDiagnostic, *problem.Problem) {
	version, errProb := s.mutableVersion(versionID)
	if errProb != nil {
		return nil, errProb
	}
	if version.Lifecycle != LifecycleDraft {
		return nil, problem.New(problem.Conflict, "only draft versions can be validated")
	}

	document, diagnostics, prob := InspectDocument(version.Source)
	if prob != nil || len(diagnostics) > 0 {
		return diagnostics, prob
	}

	validatedAt = validatedAt.UTC()
	checksumValue := document.Checksum()
	version.Lifecycle = LifecycleValidated
	version.Document = &document
	version.DefinitionChecksum = checksumValue
	version.ValidatedAt = ptrTime(validatedAt)
	version.UpdatedAt = validatedAt
	s.UpdatedAt = validatedAt

	s.recordEvent(ConfigValidatedEvent{
		Metadata:           events.NewMetadata().WithOccurredAt(validatedAt),
		ConfigSetID:        s.ID,
		ConfigKey:          s.Key,
		VersionID:          version.ID,
		Version:            version.Number,
		DefinitionChecksum: checksumValue,
	})

	return nil, nil
}

func (s *ConfigSet) CompileVersion(versionID string, artifact CompilationArtifact, compiledAt time.Time) *problem.Problem {
	version, prob := s.mutableVersion(versionID)
	if prob != nil {
		return prob
	}
	if version.Lifecycle != LifecycleValidated {
		return problem.New(problem.Conflict, "only validated versions can be compiled")
	}
	if version.Document == nil || version.ValidatedAt == nil || version.DefinitionChecksum == "" {
		return problem.New(problem.Conflict, "validated definition is required before compilation")
	}

	compiledAt = compiledAt.UTC()
	artifact.CreatedAt = compiledAt
	version.Lifecycle = LifecycleCompiled
	version.Artifact = &artifact
	version.UpdatedAt = compiledAt
	s.UpdatedAt = compiledAt

	s.recordEvent(ConfigCompiledEvent{
		Metadata:    events.NewMetadata().WithOccurredAt(compiledAt),
		ConfigSetID: s.ID,
		ConfigKey:   s.Key,
		VersionID:   version.ID,
		Version:     version.Number,
		Artifact:    artifact,
	})

	return nil
}

func (s *ConfigSet) ActivateVersion(versionID string, activation Activation, projection RuntimeProjection) *problem.Problem {
	version, prob := s.mutableVersion(versionID)
	if prob != nil {
		return prob
	}
	if version.Lifecycle != LifecycleCompiled && version.Lifecycle != LifecycleInactive {
		return problem.New(problem.Conflict, "only compiled or inactive versions can be activated")
	}
	if version.Artifact == nil {
		return problem.New(problem.Conflict, "compiled artifact is required before activation")
	}
	if activation.VersionID != version.ID || activation.ConfigSetID != s.ID {
		return problem.New(problem.InvalidArgument, "activation does not match config version")
	}
	if prob := activation.Scope.Validate(); prob != nil {
		return prob
	}

	version.Lifecycle = LifecycleActive
	version.UpdatedAt = activation.ActivatedAt.UTC()
	s.UpdatedAt = activation.ActivatedAt.UTC()
	s.recordEvent(ConfigActivatedEvent{
		Metadata:    events.NewMetadata().WithOccurredAt(activation.ActivatedAt),
		ConfigSetID: s.ID,
		ConfigKey:   s.Key,
		VersionID:   version.ID,
		Version:     version.Number,
		Activation:  activation,
		Projection:  projection,
	})

	return nil
}

func (s *ConfigSet) DeactivateVersion(versionID string, activation Activation, stillActive bool, deactivatedAt time.Time) *problem.Problem {
	version, prob := s.mutableVersion(versionID)
	if prob != nil {
		return prob
	}
	if version.Lifecycle != LifecycleActive {
		return problem.New(problem.Conflict, "only active versions can be deactivated")
	}

	version.Lifecycle = LifecycleInactive
	if stillActive {
		version.Lifecycle = LifecycleActive
	}
	version.UpdatedAt = deactivatedAt.UTC()
	s.UpdatedAt = deactivatedAt.UTC()
	s.recordEvent(ConfigDeactivatedEvent{
		Metadata:    events.NewMetadata().WithOccurredAt(deactivatedAt),
		ConfigSetID: s.ID,
		ConfigKey:   s.Key,
		VersionID:   version.ID,
		Version:     version.Number,
		Activation:  activation,
		Scope:       activation.Scope,
	})

	return nil
}

func (s *ConfigSet) ArchiveVersion(versionID string, archivedAt time.Time) *problem.Problem {
	version, prob := s.mutableVersion(versionID)
	if prob != nil {
		return prob
	}
	switch version.Lifecycle {
	case LifecycleDraft, LifecycleValidated, LifecycleCompiled, LifecycleInactive, LifecycleRejected:
	default:
		return problem.New(problem.Conflict, "config version cannot be archived from its current lifecycle")
	}

	version.Lifecycle = LifecycleArchived
	version.UpdatedAt = archivedAt.UTC()
	s.UpdatedAt = archivedAt.UTC()
	s.recordEvent(ConfigArchivedEvent{
		Metadata:    events.NewMetadata().WithOccurredAt(archivedAt),
		ConfigSetID: s.ID,
		ConfigKey:   s.Key,
		VersionID:   version.ID,
		Version:     version.Number,
	})

	return nil
}

func (s *ConfigSet) RejectVersion(versionID, reason string, rejectedAt time.Time) *problem.Problem {
	version, prob := s.mutableVersion(versionID)
	if prob != nil {
		return prob
	}
	if version.Lifecycle != LifecycleDraft && version.Lifecycle != LifecycleValidated {
		return problem.New(problem.Conflict, "only draft or validated versions can be rejected")
	}

	version.Lifecycle = LifecycleRejected
	version.RejectedReason = strings.TrimSpace(reason)
	version.UpdatedAt = rejectedAt.UTC()
	s.UpdatedAt = rejectedAt.UTC()
	s.recordEvent(ConfigRejectedEvent{
		Metadata:    events.NewMetadata().WithOccurredAt(rejectedAt),
		ConfigSetID: s.ID,
		ConfigKey:   s.Key,
		VersionID:   version.ID,
		Version:     version.Number,
		Reason:      version.RejectedReason,
	})

	return nil
}

func (s ConfigSet) VersionByID(versionID string) (ConfigVersion, bool) {
	for _, version := range s.Versions {
		if version.ID == versionID {
			return version, true
		}
	}
	return ConfigVersion{}, false
}

func (s ConfigSet) LatestVersion() (ConfigVersion, bool) {
	if len(s.Versions) == 0 {
		return ConfigVersion{}, false
	}
	return s.Versions[len(s.Versions)-1], true
}

func (s *ConfigSet) PullEvents() []events.Event {
	if s == nil || len(s.pendingEvents) == 0 {
		return nil
	}
	eventsCopy := append([]events.Event(nil), s.pendingEvents...)
	s.pendingEvents = nil
	return eventsCopy
}

func (s *ConfigSet) mutableVersion(versionID string) (*ConfigVersion, *problem.Problem) {
	if s == nil {
		return nil, problem.New(problem.Unavailable, "config set is unavailable")
	}
	versionID = strings.TrimSpace(versionID)
	for index := range s.Versions {
		if s.Versions[index].ID == versionID {
			return &s.Versions[index], nil
		}
	}
	return nil, problem.New(problem.NotFound, "config version not found")
}

func (s ConfigSet) hasOpenCandidate() bool {
	for _, version := range s.Versions {
		if version.Lifecycle == LifecycleDraft || version.Lifecycle == LifecycleValidated {
			return true
		}
	}
	return false
}

func (s ConfigSet) hasVersionID(versionID string) bool {
	for _, version := range s.Versions {
		if version.ID == strings.TrimSpace(versionID) {
			return true
		}
	}
	return false
}

func (s *ConfigSet) recordEvent(event events.Event) {
	if s == nil || event == nil {
		return
	}
	s.pendingEvents = append(s.pendingEvents, event)
}

func (v ConfigVersion) BuildRuntimeProjection(set ConfigSet, scope ActivationScope, activatedAt time.Time) (RuntimeProjection, *problem.Problem) {
	if v.Document == nil || v.Artifact == nil {
		return RuntimeProjection{}, problem.New(problem.Conflict, "validated document and compilation artifact are required")
	}
	scope = scope.Normalize()
	if prob := scope.Validate(); prob != nil {
		return RuntimeProjection{}, prob
	}

	projection := RuntimeProjection{
		Scope:              scope,
		ConfigSetID:        set.ID,
		ConfigKey:          set.Key,
		VersionID:          v.ID,
		Version:            v.Number,
		Artifact:           *v.Artifact,
		ActivatedAt:        activatedAt.UTC(),
		Bindings:           append([]Binding(nil), v.Document.Bindings...),
		Fields:             append([]Field(nil), v.Document.Fields...),
		Rules:              append([]Rule(nil), v.Document.Rules...),
		DefinitionChecksum: v.DefinitionChecksum,
	}

	return projection, nil
}

func (v ConfigVersion) BuildIngestionRuntimeProjection(set ConfigSet, scope ActivationScope, activatedAt time.Time) (IngestionRuntimeProjection, *problem.Problem) {
	if v.Document == nil || v.Artifact == nil {
		return IngestionRuntimeProjection{}, problem.New(problem.Conflict, "validated document and compilation artifact are required")
	}
	scope = scope.Normalize()
	if prob := scope.Validate(); prob != nil {
		return IngestionRuntimeProjection{}, prob
	}

	projection := IngestionRuntimeProjection{
		Scope:              scope,
		ConfigSetID:        set.ID,
		ConfigKey:          set.Key,
		VersionID:          v.ID,
		Version:            v.Number,
		Artifact:           *v.Artifact,
		ActivatedAt:        activatedAt.UTC(),
		Bindings:           append([]Binding(nil), v.Document.Bindings...),
		Fields:             append([]Field(nil), v.Document.Fields...),
		DefinitionChecksum: v.DefinitionChecksum,
	}

	return projection, nil
}

func NewActivation(id string, set ConfigSet, version ConfigVersion, scope ActivationScope, activatedAt time.Time) (Activation, *problem.Problem) {
	scope = scope.Normalize()
	if prob := scope.Validate(); prob != nil {
		return Activation{}, prob
	}
	if version.Artifact == nil {
		return Activation{}, problem.New(problem.Conflict, "compiled artifact is required before activation")
	}

	var issues []problem.ValidationIssue
	if strings.TrimSpace(id) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "activation.id", Message: "must not be empty"})
	}
	if activatedAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "activation.activated_at", Message: "must not be zero"})
	}
	if len(issues) > 0 {
		return Activation{}, problem.Validation(problem.InvalidArgument, "activation is invalid", issues...)
	}

	return Activation{
		ID:          strings.TrimSpace(id),
		ConfigSetID: set.ID,
		ConfigKey:   set.Key,
		VersionID:   version.ID,
		Version:     version.Number,
		ArtifactID:  version.Artifact.ID,
		Scope:       scope,
		ActivatedAt: activatedAt.UTC(),
	}, nil
}

func (a Activation) Deactivate(at time.Time) (Activation, *problem.Problem) {
	if a.DeactivatedAt != nil {
		return Activation{}, problem.New(problem.Conflict, "activation is already deactivated")
	}
	if at.IsZero() {
		return Activation{}, problem.Validation(problem.InvalidArgument, "activation is invalid", problem.ValidationIssue{
			Field:   "deactivated_at",
			Message: "must not be zero",
		})
	}
	a.DeactivatedAt = ptrTime(at.UTC())
	return a, nil
}

func (a Activation) IsActive() bool {
	return a.DeactivatedAt == nil
}

func ptrTime(value time.Time) *time.Time {
	if value.IsZero() {
		return nil
	}
	utc := value.UTC()
	return &utc
}

func validationIssues(prob *problem.Problem) []problem.ValidationIssue {
	if prob == nil || prob.Details == nil {
		return nil
	}
	raw, ok := prob.Details[problem.DetailIssues]
	if !ok {
		return nil
	}
	issues, ok := raw.([]problem.ValidationIssue)
	if !ok {
		return nil
	}
	return issues
}
