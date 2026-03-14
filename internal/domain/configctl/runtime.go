package configctl

import (
	"strings"
	"time"

	"internal/shared/problem"
)

type CompilationArtifact struct {
	ID              string
	SchemaVersion   string
	Checksum        string
	StorageRef      string
	RuntimeLoader   string
	CompilerVersion string
	CreatedAt       time.Time
}

type ActivationScope struct {
	Kind string
	Key  string
}

type Activation struct {
	ID            string
	ConfigSetID   string
	ConfigKey     string
	VersionID     string
	Version       int
	ArtifactID    string
	Scope         ActivationScope
	ActivatedAt   time.Time
	DeactivatedAt *time.Time
}

type RuntimeProjection struct {
	Scope              ActivationScope
	ConfigSetID        string
	ConfigKey          string
	VersionID          string
	Version            int
	Artifact           CompilationArtifact
	ActivatedAt        time.Time
	Bindings           []Binding
	Fields             []Field
	Rules              []Rule
	DefinitionChecksum string
}

type IngestionRuntimeProjection struct {
	Scope              ActivationScope
	ConfigSetID        string
	ConfigKey          string
	VersionID          string
	Version            int
	Artifact           CompilationArtifact
	ActivatedAt        time.Time
	Bindings           []Binding
	Fields             []Field
	DefinitionChecksum string
}

func DefaultActivationScope() ActivationScope {
	return ActivationScope{
		Kind: "global",
		Key:  "default",
	}
}

func (s ActivationScope) Normalize() ActivationScope {
	s.Kind = strings.ToLower(strings.TrimSpace(s.Kind))
	s.Key = strings.TrimSpace(s.Key)
	if s.Kind == "" {
		s.Kind = DefaultActivationScope().Kind
	}
	if s.Key == "" {
		s.Key = DefaultActivationScope().Key
	}
	return s
}

func (s ActivationScope) Validate() *problem.Problem {
	s = s.Normalize()
	var issues []problem.ValidationIssue
	if s.Kind == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope.kind", Message: "must not be empty"})
	}
	if s.Key == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope.key", Message: "must not be empty"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "activation scope is invalid", issues...)
}

func (s ActivationScope) String() string {
	s = s.Normalize()
	return s.Kind + ":" + s.Key
}

func NewCompilationArtifact(id, schemaVersion, checksumValue, storageRef, runtimeLoader, compilerVersion string, createdAt time.Time) (CompilationArtifact, *problem.Problem) {
	artifact := CompilationArtifact{
		ID:              strings.TrimSpace(id),
		SchemaVersion:   strings.TrimSpace(schemaVersion),
		Checksum:        strings.TrimSpace(checksumValue),
		StorageRef:      strings.TrimSpace(storageRef),
		RuntimeLoader:   strings.TrimSpace(runtimeLoader),
		CompilerVersion: strings.TrimSpace(compilerVersion),
		CreatedAt:       createdAt.UTC(),
	}

	var issues []problem.ValidationIssue
	if artifact.ID == "" {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.id", Message: "must not be empty"})
	}
	if artifact.SchemaVersion == "" {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.schema_version", Message: "must not be empty"})
	}
	if artifact.Checksum == "" {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.checksum", Message: "must not be empty"})
	}
	if artifact.StorageRef == "" {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.storage_ref", Message: "must not be empty"})
	}
	if artifact.RuntimeLoader == "" {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.runtime_loader", Message: "must not be empty"})
	}
	if artifact.CreatedAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "artifact.created_at", Message: "must not be zero"})
	}
	if len(issues) == 0 {
		return artifact, nil
	}
	return CompilationArtifact{}, problem.Validation(problem.InvalidArgument, "compilation artifact is invalid", issues...)
}
